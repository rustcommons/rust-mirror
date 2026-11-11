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
