"""Tests for icne CLI library modules."""

import json
import os
import socket
import struct
import sys
import tempfile
import threading
import unittest
from pathlib import Path
from unittest.mock import patch

# Add scripts/ to path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "scripts"))

from icne_lib import config, ipc, nfs


class TestConfig(unittest.TestCase):
    def setUp(self):
        self.tmpdir = tempfile.mkdtemp()
        self.config_dir = Path(self.tmpdir) / "config"
        self.config_file = self.config_dir / "config.toml"

    def tearDown(self):
        import shutil
        shutil.rmtree(self.tmpdir)

    def test_default_config(self):
        c = config.default_config()
        self.assertIn("general", c)
        self.assertIn("nfs", c)
        self.assertIn("folders", c)
        self.assertEqual(c["folders"], [])

    def test_save_and_load(self):
        with patch.object(config, "CONFIG_DIR", self.config_dir), \
             patch.object(config, "CONFIG_FILE", self.config_file):
            c = config.default_config()
            c["folders"].append({"source": "/tmp/test", "label": "Test"})
            config.save_config(c)

            loaded = config.load_config()
            self.assertEqual(len(loaded["folders"]), 1)
            self.assertEqual(loaded["folders"][0]["source"], "/tmp/test")
            self.assertEqual(loaded["folders"][0]["label"], "Test")

    def test_load_missing_returns_default(self):
        with patch.object(config, "CONFIG_FILE", Path("/nonexistent/config.toml")):
            c = config.load_config()
            self.assertEqual(c, config.default_config())

    def test_mount_point_for(self):
        c = config.default_config()
        mp = config.mount_point_for("/Users/test/Library/Mobile Documents/CloudDocs", c)
        self.assertEqual(mp, Path(config.DEFAULT_MOUNT_BASE) / "CloudDocs")

    def test_add_folder_nonexistent(self):
        with patch.object(config, "CONFIG_DIR", self.config_dir), \
             patch.object(config, "CONFIG_FILE", self.config_file):
            with self.assertRaises(FileNotFoundError):
                config.add_folder("/nonexistent/path/12345")

    def test_add_folder_existing_dir(self):
        test_dir = Path(self.tmpdir) / "icloud"
        test_dir.mkdir()
        with patch.object(config, "CONFIG_DIR", self.config_dir), \
             patch.object(config, "CONFIG_FILE", self.config_file):
            c = config.add_folder(str(test_dir), label="Test iCloud")
            self.assertEqual(len(c["folders"]), 1)
            self.assertEqual(c["folders"][0]["label"], "Test iCloud")

    def test_add_duplicate_folder(self):
        test_dir = Path(self.tmpdir) / "icloud"
        test_dir.mkdir()
        with patch.object(config, "CONFIG_DIR", self.config_dir), \
             patch.object(config, "CONFIG_FILE", self.config_file):
            config.add_folder(str(test_dir))
            with self.assertRaises(ValueError):
                config.add_folder(str(test_dir))

    def test_remove_folder(self):
        test_dir = Path(self.tmpdir) / "icloud"
        test_dir.mkdir()
        with patch.object(config, "CONFIG_DIR", self.config_dir), \
             patch.object(config, "CONFIG_FILE", self.config_file):
            config.add_folder(str(test_dir))
            c = config.remove_folder(str(test_dir))
            self.assertEqual(len(c["folders"]), 0)


class TestNfs(unittest.TestCase):
    def test_cidr_to_network_mask(self):
        net, mask = nfs.cidr_to_network_mask("192.168.0.0/24")
        self.assertEqual(net, "192.168.0.0")
        self.assertEqual(mask, "255.255.255.0")

        net, mask = nfs.cidr_to_network_mask("10.0.0.0/8")
        self.assertEqual(net, "10.0.0.0")
        self.assertEqual(mask, "255.0.0.0")

    def test_generate_exports_entry(self):
        entry = nfs.generate_exports_entry("/tmp/mnt", "192.168.1.0/24")
        self.assertEqual(
            entry,
            "/tmp/mnt -network 192.168.1.0 -mask 255.255.255.0",
        )

    def test_update_exports_empty(self):
        with patch.object(nfs, "read_exports", return_value=""):
            result = nfs.update_exports(["/tmp/mnt -network 192.168.0.0 -mask 255.255.255.0"])
        self.assertIn("# BEGIN icloud-nfs-exporter", result)
        self.assertIn("/tmp/mnt", result)
        self.assertIn("# END icloud-nfs-exporter", result)

    def test_update_exports_preserves_existing(self):
        existing = "/other/export -alldirs\n"
        with patch.object(nfs, "read_exports", return_value=existing):
            result = nfs.update_exports(["/tmp/mnt -network 192.168.0.0 -mask 255.255.255.0"])
        self.assertIn("/other/export -alldirs", result)
        self.assertIn("/tmp/mnt", result)

    def test_update_exports_replaces_block(self):
        existing = (
            "/other/export -alldirs\n"
            "# BEGIN icloud-nfs-exporter\n"
            "/old/path -network 10.0.0.0 -mask 255.0.0.0\n"
            "# END icloud-nfs-exporter\n"
        )
        with patch.object(nfs, "read_exports", return_value=existing):
            result = nfs.update_exports(["/new/path -network 192.168.0.0 -mask 255.255.255.0"])
        self.assertNotIn("/old/path", result)
        self.assertIn("/new/path", result)
        self.assertIn("/other/export", result)

    def test_update_exports_no_entries(self):
        with patch.object(nfs, "read_exports", return_value=""):
            result = nfs.update_exports([])
        self.assertNotIn("BEGIN", result)


class TestIpc(unittest.TestCase):
    def test_client_creation(self):
        client = ipc.IpcClient("/tmp/test.sock", timeout=5.0)
        self.assertEqual(client.socket_path, "/tmp/test.sock")

    def test_is_available_missing_socket(self):
        client = ipc.IpcClient("/tmp/nonexistent-12345.sock")
        self.assertFalse(client.is_available())

    def test_send_receive_with_mock_server(self):
        """Test full IPC round-trip with a mock server."""
        sock_path = os.path.join(tempfile.mkdtemp(), "test.sock")

        def mock_server():
            srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            srv.bind(sock_path)
            srv.listen(1)
            conn, _ = srv.accept()
            # Read request
            hdr = conn.recv(4)
            length = struct.unpack(">I", hdr)[0]
            data = conn.recv(length)
            req = json.loads(data)
            # Send pong response
            resp = json.dumps({"type": "pong"}).encode()
            conn.sendall(struct.pack(">I", len(resp)) + resp)
            conn.close()
            srv.close()

        t = threading.Thread(target=mock_server)
        t.start()

        # Give server time to bind
        import time
        time.sleep(0.1)

        client = ipc.IpcClient(sock_path)
        self.assertTrue(client.ping())

        t.join(timeout=2)
        os.unlink(sock_path)

    def test_send_receive_query_state(self):
        """Test query_state with a mock server."""
        sock_path = os.path.join(tempfile.mkdtemp(), "test.sock")

        def mock_server():
            srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            srv.bind(sock_path)
            srv.listen(1)
            conn, _ = srv.accept()
            hdr = conn.recv(4)
            length = struct.unpack(">I", hdr)[0]
            conn.recv(length)
            resp = json.dumps({
                "type": "state", "path": "/test", "state": "evicted"
            }).encode()
            conn.sendall(struct.pack(">I", len(resp)) + resp)
            conn.close()
            srv.close()

        t = threading.Thread(target=mock_server)
        t.start()

        import time
        time.sleep(0.1)

        client = ipc.IpcClient(sock_path)
        state = client.query_state("/test")
        self.assertEqual(state, "evicted")

        t.join(timeout=2)
        os.unlink(sock_path)


if __name__ == "__main__":
    unittest.main()
