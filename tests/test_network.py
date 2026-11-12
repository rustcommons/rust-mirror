import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / 'target' / 'debug' / ('rust-mirror.exe' if os.name == 'nt' else 'rust-mirror')

class NetworkProtocol(unittest.TestCase):
    def invoke(self, args, *, input=None, env=None):
        result = subprocess.run([str(BINARY), '--events', 'jsonl', *args], capture_output=True, input=input, env=env, timeout=15)
        events = [json.loads(line) for line in result.stdout.decode().splitlines()]
        self.assertEqual(events[0]['type'], 'hello')
        self.assertEqual([e['seq'] for e in events], list(range(len(events))))
        self.assertEqual(events[-1]['type'], 'result')
        self.assertEqual(sum(e['type'] == 'result' for e in events), 1)
        return result, events

    def test_credentials_in_invalid_proxy_never_escape(self):
        result, events = self.invoke(['--proxy', 'http://user:never-echo-this@localhost:9', 'check-network'])
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(events[0]['command'], 'check-network')
        self.assertEqual(events[-1]['error']['kind'], 'invalid')
        self.assertNotIn(b'never-echo-this', result.stdout + result.stderr)

    def test_invalid_config_omits_the_raw_input(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'config.toml'
            path.write_text('password="never-echo-this"\n')
            result, events = self.invoke(['--network-config', str(path), 'check-network'])
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(events[-1]['error']['kind'], 'invalid')
            self.assertNotIn(b'never-echo-this', result.stdout + result.stderr)

    def test_password_stdin_is_consumed_before_cancel_input(self):
        env = os.environ.copy()
        env['ROM_PROXY_USER'] = 'test-user'
        # A deterministic invalid timeout fails validation after reading the password.
        result, events = self.invoke(['check-network', '--proxy', 'http://127.0.0.1:9', '--proxy-username-env', 'ROM_PROXY_USER', '--proxy-password-stdin', '--read-timeout', '0', '--cancel-stdin'], input=b'never-echo-this\n{"type":"cancel"}\n', env=env)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(events[-1]['error']['kind'], 'invalid')
        self.assertNotIn(b'never-echo-this', result.stdout + result.stderr)

    def test_offline_discovery_does_not_create_state(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'absent'
            result, events = self.invoke(['releases', '--cache', str(path), '--offline'])
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse(path.exists())
            self.assertEqual(events[-1]['error']['kind'], 'invalid')

    def test_malformed_equals_proxy_flags_do_not_echo_userinfo(self):
        for args in [ ['--proxy=http://user:never-echo-this@localhost:9', 'check-network'], ['plan', '--proxy=http://user:never-echo-this@localhost:9'] ]:
            result, events = self.invoke(args)
            self.assertNotEqual(result.returncode, 0)
            self.assertNotIn(b'never-echo-this', result.stdout + result.stderr)
            self.assertEqual(events[-1]['error']['kind'], 'invalid')
