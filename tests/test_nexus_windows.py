"""Safety boundaries for the destructive disposable Windows runner."""
import json
from pathlib import Path
import stat
import sys
import tempfile
import unittest
from unittest.mock import patch
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
import run_nexus_windows as runner
import prove_nexus_windows_crates as client


class NativeNexusOwnership(unittest.TestCase):
    def test_archive_traversal_drive_and_symlink_refuse_before_extract(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name in ("../outside", "/outside", "C:/outside", "dir\\outside"):
                archive = root / "input.zip"
                with zipfile.ZipFile(archive, "w") as output:
                    member = zipfile.ZipInfo("placeholder")
                    member.filename = member.orig_filename = name
                    output.writestr(member, b"untrusted")
                with self.assertRaises(ValueError):
                    runner.extract_bundle(archive, root / "extracted")
                self.assertFalse((root / "extracted").exists())
            with zipfile.ZipFile(archive, "w") as output:
                member = zipfile.ZipInfo("link")
                member.create_system = 3
                member.external_attr = (stat.S_IFLNK | 0o777) << 16
                output.writestr(member, "target")
            with self.assertRaises(ValueError):
                runner.extract_bundle(archive, root / "extracted")

    def test_certificate_cleanup_rejects_untrusted_shell_text_before_running(self):
        with patch.object(client, "command") as command:
            for value in ("", "A"*39, "a"*40, "ABC; Remove-Item C:"):
                with self.assertRaises(ValueError):
                    client.remove_native_trust(value)
            command.assert_not_called()

    def test_cleanup_preserves_unmarked_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "foreign"
            root.mkdir()
            (root / "precious").write_text("preserve")
            with self.assertRaises(ValueError):
                runner.cleanup(root)
            self.assertEqual((root / "precious").read_text(), "preserve")

    def test_cleanup_preserves_mismatched_marker(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "ownership.json").write_text(json.dumps({"kind":"rust-mirror-native-nexus", "root":str(root / "other")}))
            with self.assertRaises(ValueError):
                runner.cleanup(root)
            self.assertTrue(root.exists())

    def test_reused_pid_is_never_killed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "ownership.json").write_text(json.dumps({"kind":"rust-mirror-native-nexus", "root":str(root), "pid":123}))
            with patch.object(runner.subprocess, "run") as run:
                run.return_value.returncode = 0
                run.return_value.stdout = str(root / "unrelated.exe")
                with self.assertRaisesRegex(RuntimeError, "reused PID"):
                    runner.cleanup(root)
                self.assertEqual(run.call_count, 1)
                self.assertNotIn("taskkill", run.call_args.args[0])
            self.assertTrue(root.exists())

    def test_completed_owned_instance_is_removed_without_global_process_actions(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "owned"
            root.mkdir()
            (root / "ownership.json").write_text(json.dumps({"kind":"rust-mirror-native-nexus", "root":str(root)}))
            with patch.object(runner.subprocess, "run") as run:
                result = runner.cleanup(root)
                run.assert_not_called()
            self.assertEqual(result, {"instance_removed":True, "private_keys_destroyed":True})
            self.assertFalse(root.exists())


if __name__ == "__main__":
    unittest.main()
