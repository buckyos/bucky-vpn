"""Real migration copy, collision, recovery, index and cleanup behavior."""
import copy
import json
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest import mock
from governance_fixture import ROOT, module, install, packet, write, cli

M = module('legacy-task-migrate')

class MigrationTests(unittest.TestCase):
    def fixture(self, root):
        install(root)
        path = packet(root)
        legacy = 'docs/versions/v0.1/modules/repo-governance/001-fixture/'
        for name in ('task.yaml', 'proposal.md'):
            data = (path.parent/name).read_bytes()
            if name == 'task.yaml': data += b'mode: manual\n'
            write(root, legacy+name, data)
        write(root, legacy+'lifecycle.json', b'{"old":"receipt-original"}')
        write(root, legacy+'pipeline/plan.md', b'old launch original')
        write(root, 'docs/versions/v0.1/modules/tasks.json', json.dumps({'tasks':[{'task_id':'001-fixture'}]}))
        write(root, 'docs/changes/old.md', b'old change')
        write(root, 'test-results/old.json', b'{"old":"run-original"}')
        write(root, 'product.txt', b'protected')
        # Destination absent at migration start, canonical packet recreated by owner.
        import shutil
        shutil.rmtree(path.parent)
        paths = [p for tree in ('docs/versions','docs/changes','test-results') for p in (root/tree).rglob('*') if p.is_file()]
        upgrade = root/'.harness/upgrade-039'; upgrade.mkdir(parents=True, exist_ok=True)
        archive = upgrade/'originals.zip'
        with zipfile.ZipFile(archive, 'w') as bundle:
            for p in paths: bundle.write(p, p.relative_to(root).as_posix())
        write(root, '.harness/upgrade-039/originals-manifest.json', json.dumps({'archive':'.harness/upgrade-039/originals.zip','archive_sha256':M.sha(archive.read_bytes()),'files':[{'path':p.relative_to(root).as_posix(),'sha256':M.sha(p.read_bytes())} for p in paths]}))
        write(root, '.harness/upgrade-039/protected-files.json', json.dumps({'files':[{'path':'product.txt','sha256':M.sha(b'protected')}]}))
        write(root, '.harness/upgrade-039/legacy-unfinished-list.json', json.dumps({'tasks':[{'task_id':'001-fixture'}]}))
        return M.LegacyTaskMigration(root, Path('.harness/upgrade-039/migration-journal.json'))

    def test_normal_idempotent_copy_index_cleanup_and_original_receipts(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory); migration=self.fixture(root)
            migration.apply(); migration.apply()
            self.assertEqual(migration.state['status'],'indexed')
            cli(root,'task-index','contains','--task','.harness/tasks/v0.1/modules/repo-governance/001-fixture/task.yaml')
            self.assertFalse((root/'.harness/tasks/v0.1/modules/repo-governance/001-fixture/lifecycle.json').exists())
            self.assertEqual((root/'.harness/archives/legacy-upgrade-039/docs/versions/v0.1/modules/repo-governance/001-fixture/lifecycle.json').read_bytes(), b'{"old":"receipt-original"}')
            migration.cleanup(); migration.cleanup(); migration.apply()
            self.assertEqual(migration.state['status'],'cleaned')
            self.assertFalse((root/'docs/versions').exists())
            self.assertEqual((root/'product.txt').read_bytes(), b'protected')

    def test_all_conflicts_preflight_without_partial_copy(self):
        for category in ('target','source-modified','source-added','archive-hash','journal-product'):
            with self.subTest(category=category), tempfile.TemporaryDirectory() as directory:
                root=Path(directory); migration=self.fixture(root); migration.inventory()
                item=migration.state['files'][-1]
                if category=='target': write(root,item['destination'],b'conflicting')
                if category=='source-modified': write(root,item['source'],b'changed')
                if category=='source-added': write(root,'docs/changes/new.md',b'new')
                if category=='archive-hash': write(root,'.harness/upgrade-039/originals.zip',b'broken')
                if category=='journal-product': migration.state['files'][0]['destination']='product.txt'
                journal=migration.journal.read_bytes()
                with self.assertRaises(M.MigrationError): migration.apply()
                self.assertEqual(migration.journal.read_bytes(),journal)
                self.assertEqual((root/'product.txt').read_bytes(),b'protected')
                self.assertFalse((root/migration.state['files'][0]['archive']).exists())

    def test_interrupted_copy_and_cleanup_resume(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory); migration=self.fixture(root); migration.inventory()
            save=migration.save; count=0
            def interrupted():
                nonlocal count
                save(); count+=1
                if count==2: raise OSError('simulated interruption')
            with mock.patch.object(migration,'save',side_effect=interrupted), self.assertRaises(OSError): migration.apply()
            resumed=M.LegacyTaskMigration(root,migration.journal); resumed.apply()
            import shutil
            remove=shutil.rmtree; count=0
            def stop_after_first(path):
                nonlocal count
                remove(path); count+=1
                if count==1: raise OSError('cleanup interrupted')
            with mock.patch.object(M.shutil,'rmtree',side_effect=stop_after_first), self.assertRaises(OSError): resumed.cleanup()
            M.LegacyTaskMigration(root,migration.journal).cleanup()
            self.assertFalse((root/'docs/changes').exists())

    def test_links_and_outside_paths_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory); migration=self.fixture(root)
            with self.assertRaises(M.MigrationError): migration.safe(root.parent/'outside')
            link=root/'docs/changes/link'
            try: link.symlink_to(root/'product.txt')
            except OSError as error: self.skipTest('OS denies creating symlink: '+str(error))
            with self.assertRaises(M.MigrationError): migration.inventory()


class Task039MigrationProvenanceTests(unittest.TestCase):
    """Actual local migration preservation, separate from synthesized fixtures."""

    def test_current_actual_inventory_protected_and_archive_hashes(self):
        manifest=json.loads((ROOT/'.harness/upgrade-039/originals-manifest.json').read_text())
        archive=ROOT/manifest['archive']
        self.assertEqual(M.sha(archive.read_bytes()),manifest['archive_sha256'])
        with zipfile.ZipFile(archive) as bundle:
            for item in manifest['files']:
                self.assertEqual(M.sha(bundle.read(item['path'])),item['sha256'],item['path'])
        journal=json.loads((ROOT/'.harness/upgrade-039/migration-journal.json').read_text())
        self.assertEqual(journal['status'],'cleaned'); self.assertEqual(len(journal['packets']),38)
        # Current destination/archive-tree verification belongs to the migration
        # owner's separate snapshot check; formal task evidence binds immutable zip.
        protected=json.loads((ROOT/'.harness/upgrade-039/protected-files.json').read_text())
        for item in protected['files']:
            self.assertEqual(M.sha((ROOT/item['path']).read_bytes()),item['sha256'],item['path'])


if __name__ == '__main__': unittest.main()
