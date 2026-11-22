import functools
import http.client
import http.server
from pathlib import Path
import sys
import tempfile
import threading
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
import prove_offline


class OfflineUploadFixture(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.served = self.root / "served"
        self.served.mkdir()
        self.prefix = self.served / prove_offline.PREFIX.lstrip("/")
        self.prefix.mkdir(parents=True)
        handler = prove_offline.upload_handler(self.served, [])
        self.server = http.server.ThreadingHTTPServer(
            ("127.0.0.1", 0), functools.partial(handler, directory=str(self.served)))
        self.thread = threading.Thread(
            target=lambda: self.server.serve_forever(poll_interval=0.01), daemon=True)
        self.thread.start()

    def tearDown(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=5)
        self.temp.cleanup()

    def put(self, suffix):
        connection = http.client.HTTPConnection("127.0.0.1", self.server.server_port, timeout=5)
        try:
            connection.request("PUT", prove_offline.PREFIX + "/" + suffix, body=b"payload")
            response = connection.getresponse()
            response.read()
            return response.status
        finally:
            connection.close()

    def symlink(self, link, target):
        try:
            link.symlink_to(target, target_is_directory=True)
        except (OSError, NotImplementedError):
            self.skipTest("symlink creation unavailable to this account")

    def test_nested_upload_succeeds_and_does_not_overwrite(self):
        self.assertEqual(self.put("nested/file"), 201)
        self.assertEqual((self.prefix / "nested/file").read_bytes(), b"payload")
        self.assertEqual(self.put("nested/file"), 412)

    def test_parent_traversal_is_refused(self):
        self.assertEqual(self.put("../../../../escaped"), 400)
        self.assertFalse((self.root / "escaped").exists())

    def test_symlinked_parent_cannot_escape_served_directory(self):
        outside = self.root / "outside"
        outside.mkdir()
        self.symlink(self.prefix / "redirect", outside)
        self.assertEqual(self.put("redirect/escaped"), 400)
        self.assertFalse((outside / "escaped").exists())

    def test_symlinked_parent_is_refused_even_inside_served_directory(self):
        target = self.served / "target"
        target.mkdir()
        self.symlink(self.prefix / "redirect", target)
        self.assertEqual(self.put("redirect/file"), 400)
        self.assertFalse((target / "file").exists())
