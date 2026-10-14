"""Cross-entrypoint contracts using isolated task plans and real run artifacts."""
import json
import os
import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path
from governance_fixture import ROOT, SCRIPTS, module, packet, plan, write, cli, install, complete_packet
from task_manifest import parse_task_manifest

RUN=module('test-run')
LIFE=module('lifecycle-check')
TRANSITION=module('task-transition')

class NativeShortcutFixture(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if os.name != 'nt':
            return
        # A .cmd uv stub transfers batch control and cannot model an executable.
        # Compile inside a temporary directory; never use the project's .venv.
        framework=Path(os.environ['WINDIR'])/'Microsoft.NET'
        compiler=next((path for path in (
            framework/'Framework64/v4.0.30319/csc.exe',
            framework/'Framework/v4.0.30319/csc.exe') if path.is_file()),None)
        if compiler is None:
            raise RuntimeError('Native Windows shortcut fixture requires .NET Framework csc.exe')
        cls.native_uv_directory=tempfile.TemporaryDirectory(prefix='harness native uv ! space ')
        cls.addClassCleanup(cls.native_uv_directory.cleanup)
        native=Path(cls.native_uv_directory.name)
        source=native/'uv.cs'
        source.write_text(r'''
using System;
using System.IO;
class NativeUvFixture {
    static int Main(string[] args) {
        string branch=args[0];
        File.AppendAllText(Environment.GetEnvironmentVariable("HARNESS_UV_LOG"),
            branch+"|"+Environment.GetEnvironmentVariable("UV_CACHE_DIR")+"|"+
            String.Join(" ",args)+Environment.NewLine);
        if (branch==Environment.GetEnvironmentVariable("HARNESS_UV_FAIL"))
            return Int32.Parse(Environment.GetEnvironmentVariable("HARNESS_UV_CODE"));
        if (branch=="venv") {
            Directory.CreateDirectory(".venv/Scripts");
            File.WriteAllText(".venv/Scripts/python.exe","marker; never executed");
            File.WriteAllText(".venv/Scripts/activate.bat","@echo off\r\nexit /b 0\r\n");
        }
        return 0;
    }
}
''',encoding='utf-8')
        result=subprocess.run([str(compiler),'/nologo','/target:exe',
                               '/out:'+str(native/'uv.exe'),str(source)],
                              capture_output=True,text=True,timeout=30)
        if result.returncode:
            raise RuntimeError('Native uv fixture compilation failed: '+result.stdout+result.stderr)
        cls.native_uv=native

    def shortcut_fixture(self,branch,code,content=None):
        with tempfile.TemporaryDirectory(prefix='harness setup ! space ') as directory:
            root=Path(directory)
            write(root,'test-run.bat',content if content is not None else (ROOT/'test-run.bat').read_bytes())
            if branch != 'venv':
                write(root,'.venv/Scripts/python.exe',b'marker; never executed')
                write(root,'.venv/Scripts/activate.bat','@echo off\nexit /b '+str(code if branch=='activation' else 0)+'\n')
            if branch=='sync': write(root,'pyproject.toml','[project]\nname="fixture"\n')
            if branch=='pip': write(root,'requirements.txt','fixture\n')
            log=root/'uv.log'
            environment=os.environ.copy()
            environment.update(PATH=str(self.native_uv)+os.pathsep+environment['PATH'],
                               HARNESS_UV_LOG=str(log),HARNESS_UV_FAIL=branch,HARNESS_UV_CODE=str(code))
            command='cmd.exe /d /v:off /c call "'+str(root/'test-run.bat')+'"'
            if branch != 'success': command+=' repo-governance/fixture all --dry-run'
            result=subprocess.run(command,cwd=root.parent,env=environment,
                                  capture_output=True,text=True,timeout=15)
            calls=log.read_text().splitlines() if log.exists() else []
            self.assertTrue((root/'.harness/uv-cache').is_dir())
            self.assertTrue(all(line.split('|')[1]==str(root/'.harness/uv-cache') for line in calls),calls)
            return result,calls

    def assert_setup_failure(self,branch,code):
        result,calls=self.shortcut_fixture(branch,code)
        self.assertEqual(result.returncode,code,result.stdout+result.stderr)
        self.assertEqual([line.split('|')[0] for line in calls],[branch],calls)

class UpgradeIntegrationTests(NativeShortcutFixture):
    @unittest.skipUnless(os.name=='nt','Windows batch invocation requires cmd.exe')
    def test_windows_venv_failure_preserves_exit_without_runner(self):
        self.assert_setup_failure('venv',7)

    @unittest.skipUnless(os.name=='nt','Windows batch invocation requires cmd.exe')
    def test_windows_sync_failure_preserves_exit_without_runner(self):
        self.assert_setup_failure('sync',11)

    @unittest.skipUnless(os.name=='nt','Windows batch invocation requires cmd.exe')
    def test_windows_pip_failure_preserves_exit_without_runner(self):
        self.assert_setup_failure('pip',13)


    @unittest.skipUnless(os.name=='nt','Windows batch invocation requires cmd.exe')
    def test_windows_activation_runner_and_default_success_contracts(self):
        for branch,code in (('activation',17),('run',23),('success',0)):
            with self.subTest(branch=branch):
                result,calls=self.shortcut_fixture(branch,code)
                self.assertEqual(result.returncode,code,result.stdout+result.stderr)
                self.assertEqual([line.split('|')[0] for line in calls],[] if branch=='activation' else ['run'])
                if branch=='success': self.assertTrue(calls[0].endswith('harness\\scripts\\test-run.py all all'),calls)

    @unittest.skipUnless(os.name=='nt','Windows batch invocation requires cmd.exe')
    def test_windows_clean_shortcut_creates_venv_and_task_dry_run(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory); install(root); path=complete_packet(root)
            write(root,'test-run.bat',(ROOT/'test-run.bat').read_bytes())
            result=subprocess.run(['cmd','/c',str(root/'test-run.bat'),'repo-governance/001-fixture','all','--dry-run'],cwd=root,capture_output=True,text=True,encoding='utf-8')
            self.assertEqual(result.returncode,0,result.stdout+result.stderr)
            self.assertTrue((root/'.venv/Scripts/python.exe').exists())
            self.assertTrue((root/'.harness/uv-cache').is_dir())
            self.assertFalse((root/'.harness/test-results/test-runs').exists())
            self.assertIn('sample.txt',result.stdout)

    def test_high_risk_full_owning_lifecycle_and_index_closeout(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory); install(root); path=complete_packet(root)
            relative=path.relative_to(root).as_posix()
            cli(root,'task-index','init','--version','v0.1')
            cli(root,'task-index','add','--task',relative)
            for stage in ('proposal','design','implementation','testing'):
                self.assertEqual(parse_task_manifest(path)['stage'],stage)
                cli(root,'risk-profile-check','--task',relative,'--prepare')
                write(root,'.harness/evidence/v0.1/stage-scope/001-fixture.paths','sample.txt\n' if stage=='implementation' else relative+'\n' if stage=='proposal' else (path.parent/(stage+'.md')).relative_to(root).as_posix()+'\n')
                if stage=='testing': artifact=self.run_task(root)
                cli(root,'task-transition','--task',relative,'advance')
            self.assertEqual(parse_task_manifest(path)['stage'],'acceptance')
            self.assertEqual(LIFE.latest_test_artifact(root,path,parse_task_manifest(path)),artifact)
            template=(ROOT/'harness/templates/task-packet/acceptance-report.md').read_text(encoding='utf-8')
            template=template.replace('CHG-example','CHG-fixture').replace('pass / fail / not-applicable','pass').replace('pass / fail / not-present','pass').replace('pass / fail','pass').replace('accepted / rejected / needs changes','accepted')
            template=template.replace('concrete primary sources inspected','proposal.md P-001, design.md Key Flows and sample.txt bytes').replace('concrete production path/symbol/runtime evidence','sample.txt contains delivery').replace('concrete gap/finding or no adequacy defect found','byte assertion detects a mismatched delivery').replace('concrete finding or no defect found','review confirms expected sample bytes').replace('concrete finding or task-specific not-applicable reason','sample has no resource, API or concurrent owner').replace('required behavior or boundary','sample contains delivery').replace('explain why the completed defect search does or does not support acceptance','sample bytes match P-001 and fixture boundary inspection found no defect')
            write(root,str((path.parent/'acceptance-report.md').relative_to(root)),template+'\nRisk profile: ./risk-profile.yaml\n')
            write(root,'.harness/evidence/v0.1/stage-scope/001-fixture.paths',(path.parent/'acceptance-report.md').relative_to(root).as_posix()+'\n')
            cli(root,'task-transition','--task',relative,'complete')
            cli(root,'task-index','remove','--task',relative)
            self.assertEqual(json.loads((root/'.harness/tasks/v0.1/tasks.json').read_text())['tasks'],[])
            self.assertEqual(set(json.loads((path.parent/'lifecycle.json').read_text())['stages']),set(LIFE.STAGES))

    def test_standard_real_baseline_completion_report_and_remove(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory); install(root)
            path=packet(root,tier='standard',stage='general')
            relative=path.relative_to(root).as_posix()
            path.write_text(path.read_text().replace('changes:', '''baseline_manifest: .harness/baselines/v0.1/001-fixture-delivery/manifest.json
changed_paths_file: .harness/evidence/v0.1/stage-scope/001-fixture.paths
change_record: .harness/changes/fixture.md
completion_report: completion-report.md
changes:'''),encoding='utf-8')
            proposal=path.parent/'proposal.md'
            proposal.write_text(proposal.read_text()+'''\n## Proposal Items
| proposal_id | change_id | requirement | success_evidence |
|---|---|---|---|
| P-001 | CHG-fixture | update sample | sample contains delivery |
''',encoding='utf-8')
            write(root,'sample.txt','before')
            for arguments in (['init'],['add','sample.txt','.gitignore'],['-c','user.name=Fixture','-c','user.email=fixture@example.invalid','commit','-m','baseline']):
                result=subprocess.run(['git',*arguments],cwd=root,capture_output=True,text=True)
                self.assertEqual(result.returncode,0,result.stderr)
            cli(root,'task-index','init','--version','v0.1'); cli(root,'task-index','add','--task',relative)
            cli(root,'lower-tier-check','--task',relative,'--profile','pre-edit')
            write(root,'sample.txt','delivery')
            record=f'''# Fixture change
- Status: complete
- Task manifest: {relative}
- Approved proposal: {proposal.relative_to(root).as_posix()}
- Affected paths: sample.txt
## Verification
- Targeted check: read sample.txt and assert delivery
- Result: passed
- Residual risk or follow-up: not-applicable
'''
            write(root,'.harness/changes/fixture.md',record)
            report=(ROOT/'harness/templates/task-packet/completion-report.md').read_text(encoding='utf-8')
            replacements={'<change>':'fixture','CHG-example':'CHG-fixture','summarize the delivered result':'sample.txt contains delivery','concise result and any residual follow-up':'sample bytes verified against requested delivery','proposal requirement or boundary':'update sample','proposal.md section/item':'proposal.md P-001','current implementation, documentation, or concrete no-change evidence':'sample.txt delivery bytes','matches / mismatch explanation':'matches approved sample update','pass / fail / not-applicable':'pass','pass / fail':'pass','concrete current implementation/documentation paths':'sample.txt delivery bytes','concrete input, error, cleanup, or documentation-consumer paths':'sample.txt existing content boundary','concrete callers, interfaces, state, tests, or downstream docs':'sample.txt and baseline changed-path manifest','concrete finding or no defect found':'delivery matches expected bytes','concrete finding or task-specific not-applicable reason':'only intended sample bytes changed','exact command/check / not-run':'read sample.txt and assert delivery','passed / not-run':'passed','not-applicable / concrete reason no executable check is proportionate':'not-applicable','none / low / medium / high':'none','proposal and delivery evidence':'proposal.md and sample.txt','no issue / concrete issue':'no defect found in sample delivery','no / yes':'no','accepted / rejected / needs changes':'accepted','explain why delivery does or does not satisfy the approved proposal':'sample delivery agrees with P-001 and independent boundary inspection'}
            for before,after in replacements.items(): report=report.replace(before,after)
            (path.parent/'completion-report.md').write_text(report,encoding='utf-8')
            cli(root,'lower-tier-check','--task',relative,'--profile','completion')
            self.assertEqual((root/'.harness/evidence/v0.1/stage-scope/001-fixture.paths').read_text().strip(),'sample.txt')
            cli(root,'task-index','remove','--task',relative)
            self.assertEqual(json.loads((root/'.harness/tasks/v0.1/tasks.json').read_text())['tasks'],[])

    def run_task(self,root,level='all',ok=True):
        result=subprocess.run([sys.executable,str(SCRIPTS/'test-run.py'),'repo-governance/001-fixture',level,'--root',str(root)],capture_output=True,text=True,encoding='utf-8')
        self.assertEqual(result.returncode==0,ok,result.stdout+result.stderr)
        paths=list((root/'.harness/test-results/test-runs').glob('*.json'))
        return max(paths,key=lambda p: json.loads(p.read_text())['started_at']) if paths else None

    def test_artifact_dedup_latest_failed_stale_and_membership(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory); taskpath=packet(root); write(root,'sample.txt','stable')
            plan(taskpath.parent/'testplan.yaml')
            artifact=self.run_task(root); task=parse_task_manifest(taskpath)
            data=json.loads(artifact.read_text())
            self.assertEqual(len(data['steps']),1)
            self.assertEqual(len(data['steps'][0]['sources']),3)
            self.assertTrue(data['inputs_unchanged'])
            self.assertEqual(LIFE.latest_test_artifact(root,taskpath,task),artifact)
            plan(taskpath.parent/'testplan.yaml',[[sys.executable,'-c','raise SystemExit(7)']])
            self.run_task(root,'unit',ok=False)
            with self.assertRaises(SystemExit): LIFE.latest_test_artifact(root,taskpath,task)
            plan(taskpath.parent/'testplan.yaml'); artifact=self.run_task(root)
            write(root,'sample.txt','changed')
            with self.assertRaises(SystemExit): LIFE.latest_test_artifact(root,taskpath,task)
            write(root,'sample.txt','stable'); write(root,'inputtree/a.txt','a')
            plan(taskpath.parent/'testplan.yaml',inputs=['inputtree'])
            self.run_task(root); write(root,'inputtree/b.txt','b')
            with self.assertRaises(SystemExit): LIFE.latest_test_artifact(root,taskpath,task)

    def test_input_mutation_writes_failed_artifact_and_manual_empty_contract(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory); taskpath=packet(root); write(root,'sample.txt','stable')
            plan(taskpath.parent/'testplan.yaml',[[sys.executable,'-c',"from pathlib import Path; Path('sample.txt').write_text('mutated')"]])
            artifact=self.run_task(root,ok=False)
            self.assertFalse(json.loads(artifact.read_text())['inputs_unchanged'])
            plan(taskpath.parent/'testplan.yaml',mode='manual')
            data=json.loads(self.run_task(root).read_text())
            self.assertEqual(data['steps'],[]); self.assertEqual(len(data['non_executed_levels']),3)
            self.assertTrue(LIFE.successful_test_run(data,'repo-governance/001-fixture',data['testplans'][0],{'CHG-fixture'}))
            p=taskpath.parent/'testplan.yaml'
            p.write_text(p.read_text().replace('mode: manual','mode: enabled'),encoding='utf-8')
            self.run_task(root,ok=False)

    def test_index_owner_registration_rejects_legacy_and_premature_remove(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory); path=packet(root)
            cli(root,'task-index','init','--version','v0.1')
            for _ in range(2): cli(root,'task-index','add','--task',path.relative_to(root).as_posix())
            data=json.loads((root/'.harness/tasks/v0.1/tasks.json').read_text())
            self.assertEqual(len(data['tasks']),1)
            cli(root,'task-index','remove','--task',path.relative_to(root).as_posix(),ok=False)
            cli(root,'task-index','contains','--task',path.relative_to(root).as_posix())
            cli(root,'task-index','add','--task','docs/versions/v0.1/modules/repo-governance/001-fixture/task.yaml',ok=False)
            cli(root,'task-transition','--task',str(path),'complete',ok=False)
            cli(root,'task-transition','--task',str(path),'return','--to','testing',ok=False)

    def test_high_risk_receipt_sequence_recovery_and_return_invalidation(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory); path=packet(root); write(root,'sample.txt','stable'); plan(path.parent/'testplan.yaml')
            task=parse_task_manifest(path)
            for stage in ('proposal','design','implementation','testing','acceptance'):
                TRANSITION.write_stage(path,stage); task=parse_task_manifest(path)
                if stage=='testing': self.run_task(root)
                if stage=='acceptance': write(root,str((path.parent/'acceptance-report.md').relative_to(root)),'accepted fixture')
                LIFE.record_stage(root,path,task,stage)
                LIFE.verify_receipts(root,path,task,tuple(LIFE.STAGES[:LIFE.STAGES.index(stage)+1]))
            cli(root,'task-transition','--task',str(path),'return','--to','design')
            state=json.loads((path.parent/'lifecycle.json').read_text())
            self.assertEqual(set(state['stages']),{'proposal'})
            self.assertIn('status: draft',(path.parent/'design.md').read_text())
            self.assertIn('status: draft',(path.parent/'testing.md').read_text())
            cli(root,'task-transition','--task',str(path),'advance',ok=False)
            self.assertEqual(parse_task_manifest(path)['stage'],'design')

    def test_clean_durable_templates_and_cli_consumers_compile(self):
        import py_compile
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory); install(root)
            import shutil
            shutil.copytree(root/'harness/templates/task-packet',root/'.harness/tasks/v0.1/modules/_template')
            write(root,'.harness/changes/_template.md',(root/'harness/templates/change-record.md').read_bytes())
            cli(root,'task-index','init','--version','v0.1')
            self.assertFalse((root/'docs/versions').exists())
            for path in (root/'harness/scripts').glob('*.py'):
                py_compile.compile(str(path),doraise=True)
            # Execute documented current CLI forms against the clean local index.
            cli(root,'task-index','list','--version','v0.1')
            cli(root,'context','--validate-index')
            cli(root,'task-seq','next','--version','v0.1')

class FreshCloneRegressionTests(unittest.TestCase):
    """Execute installed durable suites without inheriting this task's history.

    Selected by the task plan only: the nested module run selects the portable
    classes explicitly, so this end-to-end regression cannot invoke itself.
    """
    def fresh_fixture(self,root):
        import shutil
        install(root)
        for name in ('AGENTS.md','test-run.bat','test-run.sh'):
            write(root,name,(ROOT/name).read_bytes())
        for name in ('architecture','modules'):
            shutil.copytree(ROOT/'docs'/name,root/'docs'/name)
        (root/'.harness').mkdir()
        self.assertEqual(list((root/'.harness').iterdir()),[])
        shutil.copytree(root/'harness/templates/task-packet',root/'.harness/tasks/v0.1/modules/_template')
        write(root,'.harness/changes/_template.md',(root/'harness/templates/change-record.md').read_bytes())
        cli(root,'task-index','init','--version','v0.1')

    def run_installed(self,root,scope,level,*args):
        return subprocess.run([sys.executable,str(root/'harness/scripts/test-run.py'),scope,level,*args],
                              cwd=root,capture_output=True,text=True,encoding='utf-8',timeout=180)

    def test_clean_module_all_and_task_selection_without_private_history(self):
        with tempfile.TemporaryDirectory(prefix='harness clean module ') as directory:
            root=Path(directory); self.fresh_fixture(root)
            artifacts=root/'.harness/test-results/test-runs'
            for scope,level in (('repo-governance/039-harness-full-upgrade','all'),('unknown','all'),('repo-governance','unknown')):
                failure=self.run_installed(root,scope,level)
                self.assertNotEqual(failure.returncode,0,failure.stdout+failure.stderr)
                self.assertFalse(artifacts.exists(),'invalid selection must not claim an executed run')
            result=self.run_installed(root,'repo-governance','all')
            self.assertEqual(result.returncode,0,result.stdout+result.stderr)
            runs=list(artifacts.glob('*.json')); self.assertEqual(len(runs),1)
            data=json.loads(runs[0].read_text())
            self.assertEqual(data['requested_module'],'repo-governance')
            self.assertEqual(data['requested_level'],'all')
            self.assertTrue(data['inputs_unchanged'])
            self.assertEqual(data['testplans'],[])
            self.assertEqual(len(data['steps']),10)
            self.assertTrue(all(step['exit_code']==0 for step in data['steps']))
            self.assertTrue(all(source['kind']=='module-suite' and source['scope']=='repo-governance'
                                for step in data['steps'] for source in step['sources']))
            for script,selection in (('unit','UpgradeUnitTests'),('migration','MigrationTests'),('integration','UpgradeIntegrationTests')):
                self.assertTrue(any(step['command'][-2:]==['./harness/tests/test_harness_upgrade_'+script+'.py',selection]
                                    for step in data['steps']))
            self.assertFalse((root/'.harness/upgrade-039').exists())
            self.assertFalse((root/'.harness/archives').exists())
            self.assertEqual(json.loads((root/'.harness/tasks/v0.1/tasks.json').read_text())['tasks'],[])
            # Even an available task plan cannot leak into the module selection.
            write(root,'sample.txt','fixture input')
            path=packet(root); plan(path.parent/'testplan.yaml',[[sys.executable,'-c','raise SystemExit(19)']])
            dry=self.run_installed(root,'repo-governance','all','--dry-run')
            self.assertEqual(dry.returncode,0,dry.stdout+dry.stderr)
            self.assertNotIn('raise SystemExit(19)',dry.stdout)
            task=self.run_installed(root,'repo-governance/001-fixture','all')
            self.assertNotEqual(task.returncode,0,task.stdout+task.stderr)
            latest=max(artifacts.glob('*.json'),key=lambda p:json.loads(p.read_text())['started_at'])
            taskdata=json.loads(latest.read_text())
            self.assertEqual(len(taskdata['steps']),1)
            self.assertEqual(taskdata['steps'][0]['exit_code'],19)
            self.assertTrue(all(source['kind']=='task-testplan' for source in taskdata['steps'][0]['sources']))
            print('fresh durable copy: repo-governance all passed 10 real commands; private history absent; invalid selections emitted no artifact; task isolation preserved')

    def test_clean_module_unit_rejects_unfiltered_provenance_registration(self):
        with tempfile.TemporaryDirectory(prefix='harness unfiltered red ') as directory:
            root=Path(directory); self.fresh_fixture(root)
            runner=root/'harness/scripts/test-run.py'
            current='["python3", "./harness/tests/test_harness_upgrade_unit.py", "UpgradeUnitTests"]'
            text=runner.read_text(encoding='utf-8'); self.assertEqual(text.count(current),1)
            runner.write_text(text.replace(current,'["python3", "./harness/tests/test_harness_upgrade_unit.py"]'),encoding='utf-8')
            result=self.run_installed(root,'repo-governance','unit')
            self.assertNotEqual(result.returncode,0,result.stdout+result.stderr)
            self.assertIn('FileNotFoundError',result.stderr)
            self.assertIn('originals.zip',result.stderr)
            data=json.loads(next((root/'.harness/test-results/test-runs').glob('*.json')).read_text())
            self.assertTrue(any(step['exit_code']!=0 for step in data['steps']))
            self.assertFalse((root/'.harness/upgrade-039').exists())
            print('fresh durable copy red: unfiltered old module registration fails on absent local originals.zip')

class Task039ShortcutProvenanceTests(NativeShortcutFixture):
    """Archived pre-fix Windows behavior remains required for task 039."""

    @unittest.skipUnless(os.name=='nt','Windows batch invocation requires cmd.exe')
    def test_windows_setup_archive_red_green(self):
        with zipfile.ZipFile(ROOT/'.harness/upgrade-039/originals.zip') as archive:
            original=archive.read('test-run.bat')
        for branch,code in (('venv',7),('sync',11),('pip',13)):
            with self.subTest(branch=branch):
                before,before_calls=self.shortcut_fixture(branch,code,original)
                after,after_calls=self.shortcut_fixture(branch,code)
                self.assertEqual(before.returncode,0,before.stdout+before.stderr)
                self.assertEqual(after.returncode,code,after.stdout+after.stderr)
                self.assertEqual([line.split('|')[0] for line in before_calls],[branch])
                self.assertEqual([line.split('|')[0] for line in after_calls],[branch])
                print('native Windows red-green: '+branch+' archive='+str(before.returncode)+' fixed='+str(after.returncode)+' runner=False')


if __name__ == '__main__': unittest.main()
