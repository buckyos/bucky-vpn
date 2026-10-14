"""Regression for the common proposal gate after retirement of auto-pipeline."""
import tempfile
import unittest
from pathlib import Path
from governance_fixture import module, packet, cli
CHECK = module('harness-check')
class CommonProposalTransitionTests(unittest.TestCase):
    def test_proposal_completion_requires_approval(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = packet(root)
            commands = CHECK.build_commands(root, path, dict(CHECK.parse_task(path), changed_paths_file='.harness/evidence/proposal.paths'), 'completion')
            schema = [c for c in commands if any(p.endswith('schema-check.py') for p in c)]
            self.assertEqual(len(schema), 1)
            self.assertIn('--require-approved', schema[0])
    def test_retired_mode_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = packet(root)
            path.write_text(path.read_text() + 'mode: auto-pipeline\n', encoding='utf-8')
            with self.assertRaises(SystemExit): CHECK.parse_task(path)
            self.assertIn('unrecognized arguments', cli(root, 'context', '--mode', 'manual', ok=False).stderr)
if __name__ == '__main__': unittest.main()
