"""Bounded metadata-only transport for the corpus CLI and CI Cargo oracle."""
import contextlib
import hashlib
import http.server
import json
from pathlib import Path
import re
import ssl
import threading
import urllib.error
import urllib.request
from urllib.parse import urlsplit
from prove_crates_https import certificates

LIMIT = 32 * 1024 * 1024
CONTACT = "https://github.com/rustcommons/rust-mirror/issues"


def shard(name):
    if not re.fullmatch(r"[A-Za-z][A-Za-z0-9_-]{0,63}", name):
        raise ValueError("invalid crate name")
    n = name.lower()
    return "1/" + n if len(n) == 1 else "2/" + n if len(n) == 2 else "3/" + n[0] + "/" + n if len(n) == 3 else n[:2] + "/" + n[2:4] + "/" + n


def valid_path(path):
    try:
        return path == shard(path.rsplit("/", 1)[-1])
    except ValueError:
        return False


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args):
        return None


class IndexCache:
    def __init__(self, directory, user_agent):
        self.directory = Path(directory)
        self.directory.mkdir(parents=True, exist_ok=True)
        self.user_agent = user_agent
        self.files = {}
        self.lock = threading.RLock()
        self.requests = []
        self.opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())

    def get(self, path):
        if not valid_path(path):
            raise ValueError("not a canonical sparse path")
        with self.lock:
            if path in self.files:
                return self.files[path]
            # Hash-derived filenames cannot escape an explicitly selected private cache.
            file = self.directory / (hashlib.sha256(path.encode()).hexdigest() + ".json")
            if file.is_symlink():
                raise ValueError("metadata cache entry is a symlink")
            saved = json.loads(file.read_text()) if file.is_file() else {}
            headers = {"User-Agent": self.user_agent, "Accept-Encoding": "identity"}
            if saved.get("path") == path:
                if saved.get("etag"):
                    headers["If-None-Match"] = saved["etag"]
                if saved.get("last_modified"):
                    headers["If-Modified-Since"] = saved["last_modified"]
            request = urllib.request.Request("https://index.crates.io/" + path, headers=headers)
            try:
                with self.opener.open(request, timeout=60) as response:
                    if response.status != 200:
                        raise ValueError("sparse response is not HTTP 200")
                    data = response.read(LIMIT + 1)
                    saved = dict(path=path, etag=response.headers.get("ETag"),
                                 last_modified=response.headers.get("Last-Modified"), data=data.decode())
                    status = 200
            except urllib.error.HTTPError as error:
                if error.code != 304 or saved.get("path") != path:
                    raise
                data = saved["data"].encode()
                status = 304
            if len(data) > LIMIT or sum(map(len, self.files.values())) + len(data) > 512 * 1024 * 1024:
                raise ValueError("corpus metadata budget exceeded")
            for line in data.splitlines():
                json.loads(line)
            file.write_text(json.dumps(saved))
            self.files[path] = data
            self.requests.append(dict(path=path, status=status, sha256=hashlib.sha256(data).hexdigest()))
            return data


class BoundedServer(http.server.ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self, *args):
        self.slots = threading.BoundedSemaphore(4)
        super().__init__(*args)

    def process_request(self, request, address):
        self.slots.acquire()
        try:
            super().process_request(request, address)
        except BaseException:
            self.slots.release()
            raise

    def process_request_thread(self, *args):
        try:
            super().process_request_thread(*args)
        finally:
            self.slots.release()


class MetadataFixture:
    def __init__(self, cache):
        self.cache = cache
        self.override = None
        self.filtered = {}
        self.observed = {}
        self.archive_requests = []
        self.errors = []

    def full(self, path):
        if self.override is not None:
            if path not in self.override:
                raise FileNotFoundError("frozen/synthetic sparse cache miss: " + path)
            data = self.override[path]
        else:
            data = self.cache.get(path)
        self.observed[path] = hashlib.sha256(data).hexdigest()
        return data

    @contextlib.contextmanager
    def serve(self):
        with certificates(["index.crates.io"]) as pki:
            self.ca = pki / "ca.pem"
            context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
            context.minimum_version = ssl.TLSVersion.TLSv1_2
            context.load_cert_chain(pki / "server.pem", pki / "server.key")
            fixture = self

            class Origin(http.server.BaseHTTPRequestHandler):
                def log_message(self, *args):
                    pass

                def do_HEAD(self):
                    fixture.archive_requests.append(dict(method="HEAD", path=self.path))
                    self.send_error(403, "archive estimates disabled: sparse metadata only")

                def do_GET(self):
                    path = urlsplit(self.path).path.lstrip("/")
                    if path.startswith("cargo/"):
                        _, mode, path = path.split("/", 2)
                    else:
                        mode = "full"
                    try:
                        if path == "config.json":
                            data = json.dumps({"dl": fixture.base + "/archive/{crate}/{version}"}).encode()
                        elif path.startswith("archive/") or path.endswith(".crate"):
                            fixture.archive_requests.append(dict(method="GET", path=self.path))
                            self.send_error(403, "archive downloads forbidden")
                            return
                        elif not valid_path(path):
                            self.send_error(400)
                            return
                        elif mode == "filtered":
                            if path not in fixture.filtered:
                                self.send_error(404)
                                return
                            data = fixture.filtered[path]
                        elif mode == "full":
                            data = fixture.full(path)
                        else:
                            self.send_error(400)
                            return
                    except FileNotFoundError:
                        self.send_error(404)
                        return
                    except Exception as error:
                        fixture.errors.append(type(error).__name__ + ": " + str(error))
                        self.send_error(502)
                        return
                    self.send_response(200)
                    self.send_header("Content-Type", "application/json")
                    self.send_header("Content-Length", str(len(data)))
                    self.end_headers()
                    self.wfile.write(data)

            class Proxy(Origin):
                def do_CONNECT(self):
                    if self.path != "index.crates.io:443":
                        fixture.archive_requests.append(dict(method="CONNECT", path=self.path))
                        self.send_error(403)
                        return
                    self.send_response(200)
                    self.end_headers()
                    try:
                        with context.wrap_socket(self.connection, server_side=True) as connection:
                            Origin(connection, self.client_address, self.server)
                    except (OSError, ssl.SSLError):
                        pass
                    self.close_connection = True

            server = BoundedServer(("127.0.0.1", 0), Proxy)
            self.base = f"http://127.0.0.1:{server.server_port}"
            self.network = ["--proxy", self.base, "--extra-ca-file", str(self.ca)]
            thread = threading.Thread(target=lambda: server.serve_forever(poll_interval=0.05), daemon=True)
            thread.start()
            try:
                yield self
            finally:
                server.shutdown()
                server.server_close()
                thread.join(timeout=10)
