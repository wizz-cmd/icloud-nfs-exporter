"""Python IPC client for the hydration daemon."""

import json
import socket
import struct
from pathlib import Path

DEFAULT_SOCKET = "/tmp/icloud-nfs-exporter.sock"


class IpcError(Exception):
    pass


class IpcClient:
    """Connects to the hydration daemon over a Unix domain socket."""

    def __init__(self, socket_path: str = DEFAULT_SOCKET, timeout: float = 10.0):
        self.socket_path = socket_path
        self.timeout = timeout

    def send(self, request: dict) -> dict:
        """Send a request and return the response."""
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        sock.settimeout(self.timeout)
        try:
            sock.connect(self.socket_path)
            # Encode: 4-byte BE length + JSON
            payload = json.dumps(request).encode()
            header = struct.pack(">I", len(payload))
            sock.sendall(header + payload)

            # Read response length
            resp_header = _recv_exact(sock, 4)
            (resp_len,) = struct.unpack(">I", resp_header)
            if resp_len > 1_048_576:
                raise IpcError(f"response too large: {resp_len}")

            # Read response payload
            resp_data = _recv_exact(sock, resp_len)
            return json.loads(resp_data)
        except OSError as e:
            raise IpcError(str(e)) from e
        finally:
            sock.close()

    def ping(self) -> bool:
        """Health check — returns True if daemon responds with pong."""
        resp = self.send({"type": "ping"})
        return resp.get("type") == "pong"

    def query_state(self, path: str) -> str:
        """Query the hydration state of a file."""
        resp = self.send({"type": "query_state", "path": path})
        if resp.get("type") == "state":
            return resp["state"]
        raise IpcError(f"unexpected response: {resp}")

    def hydrate(self, path: str) -> bool:
        """Request hydration of a file.  Returns True on success."""
        resp = self.send({"type": "hydrate", "path": path})
        if resp.get("type") == "hydration_result":
            if resp["success"]:
                return True
            raise IpcError(resp.get("error", "hydration failed"))
        raise IpcError(f"unexpected response: {resp}")

    def is_available(self) -> bool:
        """Check if the daemon socket exists and responds."""
        if not Path(self.socket_path).exists():
            return False
        try:
            return self.ping()
        except IpcError:
            return False


def _recv_exact(sock: socket.socket, n: int) -> bytes:
    """Read exactly n bytes from a socket."""
    buf = bytearray()
    while len(buf) < n:
        chunk = sock.recv(n - len(buf))
        if not chunk:
            raise IpcError("connection closed")
        buf.extend(chunk)
    return bytes(buf)
