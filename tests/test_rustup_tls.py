import http.server
import http.client
from pathlib import Path
import ssl
import sys
import tempfile
import threading
import unittest
from unittest.mock import patch
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
import prove_rustup_lifecycle

class RustupTLSFixture(unittest.TestCase):
    def test_native_trust_refuses_workstations_and_self_hosted_before_tools(self):
        for system in ("Windows", "Darwin"):
            with patch.object(prove_rustup_lifecycle.platform,"system",return_value=system), patch.dict(prove_rustup_lifecycle.os.environ,{"GITHUB_ACTIONS":"true","RUNNER_ENVIRONMENT":"self-hosted"},clear=True), patch.object(prove_rustup_lifecycle.subprocess,"run") as run:
                with self.assertRaisesRegex(RuntimeError,"fresh GitHub-hosted"):
                    prove_rustup_lifecycle.fixture_tls(Path("unused"))
                run.assert_not_called()

    def test_separate_ca_and_leaf_verify_and_untrusted_clients_refuse(self):
        with tempfile.TemporaryDirectory() as root:
            # Unit tests never import temporary roots into a workstation trust store.
            with patch.object(prove_rustup_lifecycle.platform, "system", return_value="Linux"):
                server_context, ca, cleanup = prove_rustup_lifecycle.fixture_tls(Path(root))
            class Handler(http.server.BaseHTTPRequestHandler):
                def do_GET(self):
                    self.send_response(200); self.end_headers(); self.wfile.write(b"verified")
                def log_message(self, *args): pass
            server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
            server.socket = server_context.wrap_socket(server.socket, server_side=True)
            thread = threading.Thread(target=server.serve_forever, daemon=True); thread.start()
            try:
                connection=http.client.HTTPSConnection("127.0.0.1",server.server_port,context=ssl.create_default_context(cafile=ca),timeout=5)
                connection.request("GET","/")
                self.assertEqual(connection.getresponse().read(),b"verified");connection.close()
                with self.assertRaises(ssl.SSLCertVerificationError):
                    connection=http.client.HTTPSConnection("127.0.0.1",server.server_port,context=ssl.create_default_context(),timeout=5)
                    try: connection.request("GET","/")
                    finally: connection.close()

            finally:
                server.shutdown(); server.server_close(); thread.join(timeout=5); cleanup()

class NativeProofSelector(unittest.TestCase):
    def test_dated_selector_is_not_the_compiler_version(self):
        import prove_nexus
        record={"schema":2,"release":"1.90.0","date":"2025-09-18","pinned_toolchain":"stable-2025-09-18","selection":{"hosts":["x86_64-unknown-linux-gnu"]}}
        self.assertEqual(prove_nexus.native_selection(record),("1.90.0","x86_64-unknown-linux-gnu","stable-2025-09-18"))
        self.assertEqual(prove_nexus.native_selection({"schema":3,"toolchains":[record]}),("1.90.0","x86_64-unknown-linux-gnu","stable-2025-09-18"))
        self.assertEqual(prove_nexus.native_selection({"schema":1,"release":"1.90.0","host":"x86_64-unknown-linux-gnu"}),("1.90.0","x86_64-unknown-linux-gnu","1.90.0"))

class DefaultProfileSelection(unittest.TestCase):
    def test_default_profile_uses_recorded_host_not_shared_archive_filename(self):
        document={"schema":3,"toolchains":[{"release":"1.90.0","selection":{"profile":"default","hosts":["x86_64-apple-darwin"]}}],"files":[{"path":"dist/2025-09-18/rust-docs-1.90.0-aarch64-apple-darwin.tar.xz"}]}
        self.assertTrue(prove_rustup_lifecycle.default_profile_selected(document,"1.90.0","x86_64-apple-darwin"))
        self.assertFalse(prove_rustup_lifecycle.default_profile_selected(document,"1.90.0","aarch64-apple-darwin"))
        self.assertFalse(prove_rustup_lifecycle.default_profile_selected(document,"1.89.0","x86_64-apple-darwin"))
        document["toolchains"][0]["selection"]["profile"]="minimal"
        self.assertFalse(prove_rustup_lifecycle.default_profile_selected(document,"1.90.0","x86_64-apple-darwin"))
