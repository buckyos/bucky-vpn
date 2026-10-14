"""Fail-closed custom metadata, scope authorization and runner planning."""
import copy
import json
import shutil
import tempfile
import unittest
from pathlib import Path
from governance_fixture import ROOT, module, write, packet, plan

CONTEXT = module('context')
SCOPE = module('stage-scope-check')
RUN = module('test-run')
SELF = module('harness-self-check')

class UpgradeUnitTests(unittest.TestCase):
    def test_file_fragments_and_missing_unsafe_rejection(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            write(root,'harness/rules/reference.md','# Existing\n')
            write(root,'AGENTS.md','`harness/rules/reference.md#existing`\n')
            SELF.check_markdown_path_references(root)
            for value in ('harness/rules/missing.md#section','harness/../outside.md#section'):
                write(root,'AGENTS.md','`'+value+'`\n')
                with self.assertRaises(SystemExit): SELF.check_markdown_path_references(root)

    def test_missing_local_authorization_cannot_enable_scope_exception(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            path=packet(root,name='039-harness-full-upgrade',stage='implementation')
            self.assertFalse(SCOPE.approved_governance_implementation(root,path,'harness/scripts/context.py'))
            self.assertFalse(SCOPE.approved_governance_sync(root,path,str(path.parent/'testing.md')))
            self.assertFalse((root/'.harness/upgrade-039').exists())

    def test_classification_sidecar_explanation_survives_owner_rewrite(self):
        CHECK=module('harness-check')
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            relative='.harness/evidence/v0.1/stage-scope/039-harness-full-upgrade-implementation.paths'
            write(root,relative,'harness/scripts/context.py\n')
            payload={'schema':1,'stage':'implementation','version':'v0.1','module':'repo-governance','submodule':'039-harness-full-upgrade'}
            for _ in range(2):
                CHECK.write_sidecar(root,relative,dict(payload))
                data=json.loads((root/(relative+'.meta.json')).read_text())
                self.assertEqual(data['classification_amendment'],'D-039-001')
                self.assertIn('Pinned complete manifest',data['classification_reason'])

    def test_custom_known_legacy_and_generated_strict(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            text = 'schema_version: 1\nrules:\n  - id: fixture\n    file: harness/custom-rules/fixture.md\n    activation: always\n    tiers: [standard, high-risk, trivial]\n    modes: [manual, auto-pipeline]\n'
            path = write(root, 'harness/custom-rules/index.yaml', text)
            rules = CONTEXT.parse_index(path, custom_root=root)
            self.assertEqual(rules[0]['tiers'], ['standard', 'high-risk'])
            self.assertNotIn('modes', rules[0])
            for variant in (text.replace('manual', 'unknown'), text.replace('high-risk', 'unknown'), text + '    modes: [manual]\n', text.replace('harness/custom-rules/fixture.md', '../product.txt'), text.replace('    activation: always\n', ''), text.replace('  - id: fixture', '  - id: fixture') + text.split('rules:\n')[1]):
                path.write_text(variant, encoding='utf-8')
                with self.subTest(variant=variant), self.assertRaises(SystemExit):
                    CONTEXT.parse_index(path, custom_root=root)
            path.write_text(text, encoding='utf-8')
            with self.assertRaises(SystemExit): CONTEXT.parse_index(path)
            other = write(root, 'elsewhere/index.yaml', text)
            with self.assertRaises(SystemExit): CONTEXT.parse_index(other, custom_root=root)


    def test_runner_scope_union_dedup_registration_and_workdir(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = packet(root)
            plan(path.parent / 'testplan.yaml', [['python', '-c', 'pass'], ['python', '-c', 'pass ']])
            plans = RUN.discover_testplans(root)
            scope = 'repo-governance/001-fixture'
            entries = RUN.deduplicated_execution_plan(root, [scope], 'all', plans)
            self.assertEqual(len(entries), 2)
            self.assertEqual(len(entries[0]['sources']), 3)
            self.assertTrue(all(s['kind'] == 'task-testplan' for e in entries for s in e['sources']))
            module_entries = RUN.deduplicated_execution_plan(root, ['repo-governance'], 'all', plans)
            self.assertTrue(all(s['kind'] == 'module-suite' for e in module_entries for s in e['sources']))
            self.assertEqual(RUN.command_workdir(root, [{'kind':'module-suite','scope':'vpn_web'}]), root/'vpn_web')
            self.assertEqual(RUN.command_workdir(root, entries[0]['sources']), root)
            with self.assertRaises(SystemExit): RUN.selected_scopes('unknown', plans)
            self.assertEqual(set(RUN.selected_scopes('all', plans)), set(RUN.MODULE_SUITES))


class Task039UnitProvenanceTests(unittest.TestCase):
    """Explicit task checks requiring the approved local 039 evidence."""

    def test_file_fragment_red_green_and_missing_unsafe_rejection(self):
        import zipfile, types
        with zipfile.ZipFile(ROOT/'.harness/upgrade-039/originals.zip') as bundle:
            source=bundle.read('harness/scripts/harness-self-check.py').decode('utf-8')
        old=types.ModuleType('original_selfcheck'); old.__file__=str(ROOT/'harness/scripts/harness-self-check.py')
        exec(compile(source,'original-self-check','exec'),old.__dict__)
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            write(root,'harness/rules/reference.md','# Existing\n')
            write(root,'AGENTS.md','`harness/rules/reference.md#existing`\n')
            with self.assertRaises(SystemExit): old.check_markdown_path_references(root)
            SELF.check_markdown_path_references(root)
            for value in ('harness/rules/missing.md#section','harness/../outside.md#section'):
                write(root,'AGENTS.md','`'+value+'`\n')
                with self.assertRaises(SystemExit): SELF.check_markdown_path_references(root)


    def test_pinned_scope_authorization_and_all_rejection_boundaries(self):
        relative = '.harness/tasks/v0.1/modules/repo-governance/039-harness-full-upgrade'
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path=packet(root,name='039-harness-full-upgrade',stage='implementation')
            text=path.read_text().replace('  - id: CHG-fixture\n    target_module: repo-governance\n    scope_paths: ["sample.txt"]', '\n'.join('  - id: '+change+'\n    target_module: repo-governance\n    scope_paths: ["sample.txt"]' for change in SCOPE.governance_implementation_record()['change_ids']))
            path.write_text(text,encoding='utf-8')
            write(root,relative+'/proposal.md',(ROOT/relative/'proposal.md').read_bytes())
            for name in ('scope-authorization.json', 'governance-implementation-authorization.json'):
                write(root, '.harness/upgrade-039/' + name, (ROOT / '.harness/upgrade-039' / name).read_bytes())
            expected = SCOPE.governance_implementation_record()
            manifest = write(root, expected['changed_paths_file'], (ROOT / expected['changed_paths_file']).read_bytes())
            record = root / '.harness/upgrade-039/governance-implementation-authorization.json'
            target = 'harness/scripts/context.py'
            self.assertTrue(SCOPE.approved_governance_implementation(root, path, target))
            self.assertTrue(SCOPE.approved_governance_sync(root, path, relative + '/testplan.yaml'))
            for key, value in (('changed_paths_count', 278), ('proposal_sha256', '0'*64), ('allowed_stages', ['testing']), ('change_ids', []), ('allowed_artifact_groups', ['all']), ('extra', True)):
                changed = copy.deepcopy(expected); changed[key] = value
                record.write_text(json.dumps(changed), encoding='utf-8')
                self.assertFalse(SCOPE.approved_governance_implementation(root, path, target), key)
            record.write_text(json.dumps(expected), encoding='utf-8')
            data = manifest.read_bytes(); manifest.write_bytes(data.splitlines(keepends=True)[0])
            self.assertFalse(SCOPE.approved_governance_implementation(root, path, target))
            manifest.write_bytes(data)
            self.assertFalse(SCOPE.approved_governance_implementation(root, path, 'vpn-client/src/main.rs'))
            self.assertFalse(SCOPE.approved_governance_implementation(root, packet(root), target))
            for stage in ('proposal', 'design', 'testing', 'acceptance'):
                path.write_text(path.read_text(encoding='utf-8').replace('stage: implementation', 'stage: ' + stage), encoding='utf-8')
                self.assertFalse(SCOPE.approved_governance_implementation(root, path, target))
                if stage in ('testing', 'acceptance'):
                    self.assertFalse(SCOPE.approved_governance_sync(root, path, relative + '/testing.md'))
                path.write_text(path.read_text(encoding='utf-8').replace('stage: ' + stage, 'stage: implementation'), encoding='utf-8')
            record.unlink()
            self.assertFalse(SCOPE.approved_governance_implementation(root, path, target))


    def test_product_registrations_preserved_from_original(self):
        import ast, zipfile
        with zipfile.ZipFile(ROOT / '.harness/upgrade-039/originals.zip') as bundle:
            tree = ast.parse(bundle.read('harness/scripts/test-run.py').decode('utf-8'))
        mapping = next(n.value for n in tree.body if isinstance(n, (ast.Assign, ast.AnnAssign)) and (getattr(n, 'target', None) and isinstance(n.target, ast.Name) and n.target.id == 'MODULE_SUITES' or isinstance(n, ast.Assign) and any(isinstance(t, ast.Name) and t.id == 'MODULE_SUITES' for t in n.targets)))
        old = ast.literal_eval(mapping)
        for name in ('vpn-frame','bucky-vpn','bucky-vpn-server','vpn_web'):
            self.assertEqual(RUN.MODULE_SUITES[name], old[name])


if __name__ == '__main__': unittest.main()
