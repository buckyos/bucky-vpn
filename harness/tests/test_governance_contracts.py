"""Executable Python CLI consumer and documentation contracts."""
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from governance_fixture import SCRIPTS, module, packet, write, install

COVERAGE=module('testing-coverage-check')

def documentation_examples():
    with tempfile.TemporaryDirectory() as directory:
        root=Path(directory); install(root)
        import shutil
        shutil.copytree(root/'harness/templates/task-packet',root/'.harness/tasks/v0.1/modules/_template')
        write(root,'.harness/changes/_template.md',(root/'harness/templates/change-record.md').read_bytes())
        for script,args in [('task-index',['init','--version','v0.1']),('task-index',['list','--version','v0.1']),('task-seq',['next','--version','v0.1']),('context',['--validate-index'])]:
            subprocess.run([sys.executable,str(SCRIPTS/(script+'.py')),'--root',str(root),*args],check=True,capture_output=True)

def external_positive():
    with tempfile.TemporaryDirectory() as directory:
        root=Path(directory); path=packet(root)
        # Separate process imports the exported parser, then exercises index owner CLI.
        subprocess.run([sys.executable,'-c',"import sys; sys.path.insert(0,sys.argv[1]); from task_manifest import parse_task_manifest; assert parse_task_manifest(__import__('pathlib').Path(sys.argv[2]))['workflow_tier']=='high-risk'",str(SCRIPTS),str(path)],check=True,capture_output=True)
        for args in [['init','--version','v0.1'],['add','--task',path.relative_to(root).as_posix()],['contains','--task',path.relative_to(root).as_posix()]]:
            subprocess.run([sys.executable,str(SCRIPTS/'task-index.py'),'--root',str(root),*args],check=True,capture_output=True)

class PythonClosureTests(unittest.TestCase):
    def test_current_language_root_cargo_is_passive_and_rust_mixed_unknown_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory); write(root,'Cargo.toml','unrelated workspace'); write(root,'api.py','x=1')
            impact={'crate_root_export_change':False}
            self.assertEqual(COVERAGE.contract_language(root,impact,{'api.py'}),'python')
            self.assertEqual(COVERAGE.contract_language(root,impact,{'api.rs','Cargo.toml'}),'rust')
            for inputs in ({'api.py','api.rs'},{'api.ts'},set()):
                with self.subTest(inputs=inputs),self.assertRaises(SystemExit): COVERAGE.contract_language(root,impact,inputs)
            with self.assertRaises(SystemExit): COVERAGE.contract_language(root,{'crate_root_export_change':True},{'api.py'})

    def test_python_compile_and_documentation_commands_fail_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory); write(root,'api.py','x=1'); write(root,'other.py','x=2')
            wrapper=write(root,'docs.py',"import subprocess\nimport sys\nif '--documentation-examples' in sys.argv: subprocess.run([sys.executable,'-c','raise SystemExit(9)'],check=True)\n")
            check=COVERAGE.check_language_command
            check(root,'repository-compile-closure',['python','-m','compileall','-q','api.py'],'python',{'api.py'},['api.py'])
            check(root,'documentation-examples',['python','docs.py','--documentation-examples'],'python',{'api.py'},['docs.py'])
            result=subprocess.run([sys.executable,str(wrapper),'--documentation-examples'],capture_output=True)
            self.assertNotEqual(result.returncode,0,'wrapper swallowed example failure')
            for command in (['python','-c','pass'],['python','-m','compileall','other.py'],['python','-m','compileall','-x','api.py']):
                with self.subTest(command=command),self.assertRaises(SystemExit): check(root,'repository-compile-closure',command,'python',{'api.py'},['api.py'])
            for command,inputs in ((['python','docs.py'],['docs.py']),(['python','docs.py','--documentation-examples'],['api.py'])):
                with self.assertRaises(SystemExit): check(root,'documentation-examples',command,'python',{'api.py'},inputs)
            wrapper.write_text("MODE='--documentation-examples'\n",encoding='utf-8')
            with self.assertRaises(SystemExit): check(root,'documentation-examples',['python','docs.py','--documentation-examples'],'python',{'api.py'},['docs.py'])

    def test_affected_rust_preserves_all_targets_and_doctest_contract(self):
        check=COVERAGE.check_language_command; root=Path('.')
        check(root,'repository-compile-closure',['cargo','test','--no-run','--all-targets'],'rust',set(),[])
        check(root,'documentation-examples',['cargo','test','--doc'],'rust',set(),[])
        for kind,command in [('repository-compile-closure',['cargo','test','--no-run']),('repository-compile-closure',['python','-m','compileall']),('documentation-examples',['cargo','test','--doc','--no-run'])]:
            with self.assertRaises(SystemExit): check(root,kind,command,'rust',set(),[])

    def test_executable_external_and_documentation_consumers(self):
        external_positive(); documentation_examples()

if __name__=='__main__':
    if '--documentation-examples' in sys.argv: documentation_examples()
    elif '--external-positive' in sys.argv: external_positive()
    else: unittest.main()
