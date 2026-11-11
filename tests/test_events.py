import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / 'target' / 'debug' / ('rust-mirror.exe' if os.name == 'nt' else 'rust-mirror')

class EventContract(unittest.TestCase):
    def invoke(self, *args):
        result = subprocess.run([str(BINARY), '--events', 'jsonl', *args], capture_output=True, timeout=20)
        events = [json.loads(line) for line in result.stdout.decode('utf-8').splitlines()]
        self.assertEqual(events[0]['type'], 'hello')
        self.assertEqual(events[0]['protocol'], 1)
        self.assertEqual([e['seq'] for e in events], list(range(len(events))))
        self.assertEqual(events[-1]['type'], 'result')
        self.assertEqual(sum(e['type'] == 'result' for e in events), 1)
        return result, events

    def test_cheap_discovery_and_nullable_fields(self):
        result, events = self.invoke('--version')
        self.assertEqual(result.returncode, 0)
        self.assertEqual(events[-1]['report']['protocol'], 1)
        self.assertIsNone(events[-1]['error'])
        self.assertIsNone(next(e for e in events if e['type'] == 'stage')['total'])
        self.assertEqual(events[-1]['report']['tool_version'], events[0]['tool_version'])

    def test_unicode_path_and_output_exists_are_machine_readable(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'mirror-\u00e9-\u2603.toml'
            result, events = self.invoke('init', '--config', str(path))
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(events[-1]['report']['path'], str(path))
            result, events = self.invoke('init', '--config', str(path))
            self.assertEqual(result.returncode, 1)
            self.assertEqual(events[-1]['error']['kind'], 'output_exists')
            self.assertIsNone(events[-1]['report'])
            self.assertIsNone(events[-1]['error']['report'])

    def test_unknown_command_has_one_typed_failure(self):
        result, events = self.invoke('unknown-command')
        self.assertEqual(result.returncode, 1)
        self.assertEqual(events[-1]['error']['kind'], 'invalid')
