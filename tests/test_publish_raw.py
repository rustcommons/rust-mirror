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

    def test_ca_file_requires_https(self):
        with self.assertRaisesRegex(ValueError, "custom CA file requires HTTPS"):
            publish_raw.publish(self.root, "http://127.0.0.1:9/raw", allow_loopback_http=True, ca_file=self.root / "not-a-ca.pem")

    def test_missing_ca_refuses_before_network(self):
        with self.assertRaises(FileNotFoundError):
            publish_raw.publish(self.root, "https://127.0.0.1:9/raw", ca_file=self.root / "missing-ca.pem")

    def test_selection_inventory_accepts_default_and_multiple_hosts(self):
        selection = self.inventory.replace('schema = 1', 'schema = 2').replace('profile = "minimal"\n', '')
        selection += '\n[selection]\nprofile = "default"\nhosts = ["x86_64-unknown-linux-gnu", "x86_64-pc-windows-msvc"]\n'
        self.write_inventory(selection)
        document, files = publish_raw.local_files(self.root)
        self.assertEqual(document["schema"], 2)
        self.assertEqual(len(document["selection"]["hosts"]), 2)
        self.assertEqual(files[-1][0], "snapshot.toml")

    def test_dated_metadata_is_ordered_after_archives(self):
        archive = self.root / "dist/2025-09-18/z-archive.tar.xz"
        archive.parent.mkdir()
        archive.write_bytes(b"data")
        metadata = archive.with_name("channel-rust-stable.toml")
        metadata.write_bytes(b"data")
        extra = ''
        for name in [archive, metadata]:
            extra += '\n[[files]]\npath = "' + name.relative_to(self.root).as_posix() + '"\nsha256 = "' + hashlib.sha256(b"data").hexdigest() + '"\nbytes = 4\n'
        self.write_inventory(self.inventory + extra)
        _, files = publish_raw.local_files(self.root)
        names = [f[0] for f in files]
        self.assertLess(names.index(archive.relative_to(self.root).as_posix()), names.index(metadata.relative_to(self.root).as_posix()))

    def test_collection_inventory_accepts_records_and_keeps_inventory_last(self):
        text = self.inventory.replace('schema = 1', 'schema = 3')
        text += '\n[[toolchains]]\nrelease = "1.90.0"\n'
        self.write_inventory(text)
        document, files = publish_raw.local_files(self.root)
        self.assertEqual(document["schema"], 3)
        self.assertEqual(files[-1][0], "snapshot.toml")
        self.write_inventory(text.replace('[[toolchains]]\nrelease = "1.90.0"', 'toolchains = []'))
        with self.assertRaises(ValueError):
            publish_raw.local_files(self.root)

    def test_symlink_refuses(self):
        try:
            (self.root / "extra").symlink_to(self.root / "dist/file")
        except OSError:
            self.skipTest("symlink creation unavailable to this account")
        with self.assertRaises(ValueError):
            publish_raw.local_files(self.root)


if __name__ == "__main__":
    unittest.main()
