"""Exercise the installed reader in a PTY without touching user configuration.

Usage: python3 tests/terminal_smoke.py /absolute/path/to/leaf
"""

import base64
import fcntl
import http.server
import os
from pathlib import Path
import pty
import select
import struct
import subprocess
import sys
import tempfile
import termios
import threading
import time


PNG = base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8"
    "/x8AAwMCAO+jRZkAAAAASUVORK5CYII="
)


class Images(http.server.BaseHTTPRequestHandler):
    requests = 0

    def do_GET(self):
        type(self).requests += 1
        self.send_response(200)
        self.send_header("Content-Type", "image/png")
        self.send_header("Content-Length", str(len(PNG)))
        self.end_headers()
        self.wfile.write(PNG)

    def log_message(self, *args):
        pass


def read_document(binary, root, target, remote_media=False):
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 960, 800))
    env = {
        key: value for key, value in os.environ.items()
        if not key.startswith(("REX_", "LEAF_"))
    }
    env.update({
        "HOME": str(root),
        "XDG_DATA_HOME": str(root / "data"),
        "XDG_CONFIG_HOME": str(root / "config"),
        "XDG_CACHE_HOME": str(root / "cache"),
        "TERM": "xterm-256color",
        "MallocLargeCache": "0",
    })
    if remote_media:
        env["LEAF_REMOTE_MEDIA"] = "1"
    process = subprocess.Popen(
        [str(binary), "open", str(target)],
        stdin=slave, stdout=slave, stderr=slave, env=env, cwd=root,
    )
    os.close(slave)
    output = bytearray()
    started = time.monotonic()
    quitting = False
    try:
        while time.monotonic() - started < 12:
            if select.select([master], [], [], 0.1)[0]:
                try:
                    chunk = os.read(master, 65536)
                except OSError:
                    break
                if not chunk:
                    break
                output.extend(chunk)
                if b"\x1b[6n" in chunk:
                    os.write(master, b"\x1b[1;1R")
            if time.monotonic() - started > 3 and not quitting:
                os.write(master, b"q")
                quitting = True
            if process.poll() is not None:
                break
        process.wait(timeout=2)
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()
        os.close(master)
    assert process.returncode == 0, (process.returncode, repr(bytes(output)[-500:]))
    assert b"Smoke test body" in output, "document text was not rendered"
    assert b"\x1b]52;" not in output, "document title injected an OSC clipboard command"
    assert not (root / "data/leaf/ask.json").exists(), "selection context was saved"


def main():
    binary = Path(sys.argv[1]).resolve()
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Images)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        with tempfile.TemporaryDirectory(prefix="leaf-smoke-") as folder:
            root = Path(folder)
            # A filename is a real input path to Doc.title, then SetTitle.
            target = root / "Paper\x07\x1b]52;c;YXR0YWNr\x07.md"
            target.write_text(
                "Smoke test body\n\n"
                f"![tracking](http://127.0.0.1:{server.server_port}/pixel.png)\n"
            )
            read_document(binary, root, target)
            assert Images.requests == 0, "remote image fetched without opt-in"
            read_document(binary, root, target, remote_media=True)
            assert Images.requests > 0, "explicit media opt-in did not fetch the image"
            print("PASS: reader opens and quits, title injection is blocked, media opt-in works")
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=2)


if __name__ == "__main__":
    main()
