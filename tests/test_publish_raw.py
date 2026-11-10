import hashlib
from pathlib import Path
import sys
import tempfile
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
import publish_raw


class RawPreflight(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        (self.root / "dist").mkdir()
        (self.root / "dist/file").write_bytes(b"data")
        self.inventory = '\n'.join(['schema = 1', 'release = "1.90.0"', 'host = "x86_64-unknown-linux-gnu"', 'rustup_version = "1.28.2"', 'profile = "minimal"', '[[files]]', 'path = "dist/file"', f'sha256 = "{hashlib.sha256(b"data").hexdigest()}"', 'bytes = 4', ''])
        self.write_inventory(self.inventory)

    def tearDown(self):
        self.temp.cleanup()

    def write_inventory(self, text):
        (self.root / "snapshot.toml").write_text(text, encoding="utf-8")

    def test_valid_inventory_orders_marker_last(self):
        document, files = publish_raw.local_files(self.root)
        self.assertEqual(document["release"], "1.90.0")
        self.assertEqual([f[0] for f in files], ["dist/file", "snapshot.toml"])

    def test_tampered_file_refuses(self):
        (self.root / "dist/file").write_bytes(b"evil")
        with self.assertRaises(ValueError):
            publish_raw.local_files(self.root)

    def test_extra_file_refuses(self):
        (self.root / "extra").write_bytes(b"extra")
        with self.assertRaises(ValueError):
            publish_raw.local_files(self.root)

    def test_parent_encoding_backslash_absolute_and_duplicate_refuse(self):
        for path in ("../file", "/file", "dist/../file", "dist\\file", "dist/%2e/file", "snapshot.toml"):
            self.write_inventory(self.inventory.replace('path = "dist/file"', f'path = "{path}"'))
            with self.assertRaises((ValueError, FileNotFoundError)):
                publish_raw.local_files(self.root)
        self.write_inventory(self.inventory + self.inventory[self.inventory.index("[[files]]"):])
        with self.assertRaises(ValueError):
            publish_raw.local_files(self.root)

    def test_schema_and_size_refuse(self):
        for text in (self.inventory.replace("schema = 1", "schema = 2"), self.inventory.replace("bytes = 4", "bytes = 3")):
            self.write_inventory(text)
            with self.assertRaises(ValueError):
                publish_raw.local_files(self.root)

    def test_unsafe_destination_refuses_before_network(self):
        for base in ("http://example.invalid/repository/raw", "https://user:secret@example.invalid/raw", "https://example.invalid/raw?token=test", "https://example.invalid/../raw", "https://example.invalid/%2e/raw"):
            with self.assertRaises(ValueError):
                publish_raw.publish(self.root, base)

    def test_symlink_refuses(self):
        try:
            (self.root / "extra").symlink_to(self.root / "dist/file")
        except OSError:
            self.skipTest("symlink creation unavailable to this account")
        with self.assertRaises(ValueError):
            publish_raw.local_files(self.root)


if __name__ == "__main__":
    unittest.main()
