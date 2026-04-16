"""Python IPC client for the hydration daemon."""

import json
import socket
import struct
from pathlib import Path

DEFAULT_SOCKET = "/tmp/icloud-nfs-exporter.sock"


class IpcError(Exception):
    """Raise when IPC communication with the hydration daemon fails."""


class IpcClient:
    """Connect to the hydration daemon over a Unix domain socket.

    Each public method opens a fresh connection, sends a single
    length-prefixed JSON request, reads the response, and closes the
    socket.

    Args:
        socket_path: Filesystem path to the daemon's Unix socket.
        timeout: Socket timeout in seconds for connect/read/write.
    """

    def __init__(self, socket_path: str = DEFAULT_SOCKET, timeout: float = 10.0) -> None:
        self.socket_path = socket_path
        self.timeout = timeout

    def send(self, request: dict[str, object]) -> dict[str, object]:
        """Send a JSON request to the daemon and return the response.

        Open a new Unix-socket connection, transmit a 4-byte
        big-endian length header followed by the JSON payload, then
        read the response in the same format.

        Args:
            request: Serialisable dict to send (must contain a
                ``"type"`` key).

        Returns:
            The parsed JSON response as a dict.

        Raises:
            IpcError: On connection failure, protocol violation, or an
                oversized response (>1 MiB).
        """
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
        """Perform a health-check against the daemon.

        Returns:
            ``True`` if the daemon responds with a ``"pong"`` message,
            ``False`` otherwise.

        Raises:
            IpcError: On connection or protocol errors.
        """
        resp = self.send({"type": "ping"})
        return resp.get("type") == "pong"

    def query_state(self, path: str) -> str:
        """Query the hydration state of a file.

        Args:
            path: Absolute path to the iCloud file to inspect.

        Returns:
            A string describing the hydration state (e.g. ``"local"``,
            ``"evicted"``, ``"downloading"``).

        Raises:
            IpcError: If the daemon returns an unexpected response type.
        """
        resp = self.send({"type": "query_state", "path": path})
        if resp.get("type") == "state":
            return resp["state"]
        raise IpcError(f"unexpected response: {resp}")

    def hydrate(self, path: str) -> bool:
        """Request on-demand hydration (download) of an evicted file.

        Args:
            path: Absolute path to the iCloud file to hydrate.

        Returns:
            ``True`` when the file has been fully hydrated.

        Raises:
            IpcError: If hydration fails or the daemon returns an
                unexpected response type.
        """
        resp = self.send({"type": "hydrate", "path": path})
        if resp.get("type") == "hydration_result":
            if resp["success"]:
                return True
            raise IpcError(resp.get("error", "hydration failed"))
        raise IpcError(f"unexpected response: {resp}")

    def is_available(self) -> bool:
        """Check whether the daemon socket exists and responds.

        Return ``False`` without raising if the socket is missing or
        the daemon does not answer a ping.

        Returns:
            ``True`` if the daemon is reachable and healthy.
        """
        if not Path(self.socket_path).exists():
            return False
        try:
            return self.ping()
        except IpcError:
            return False


def _recv_exact(sock: socket.socket, n: int) -> bytes:
    """Read exactly *n* bytes from a socket.

    Args:
        sock: An open, connected socket.
        n: Number of bytes to read.

    Returns:
        A ``bytes`` object of length *n*.

    Raises:
        IpcError: If the connection closes before *n* bytes have been
            received.
    """
    buf = bytearray()
    while len(buf) < n:
        chunk = sock.recv(n - len(buf))
        if not chunk:
            raise IpcError("connection closed")
        buf.extend(chunk)
    return bytes(buf)
