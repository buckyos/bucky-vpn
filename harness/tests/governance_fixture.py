"""Isolated real-filesystem fixtures for governance regressions."""
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
SCRIPTS = ROOT / 'harness/scripts'
sys.path.insert(0, str(SCRIPTS))

def module(name):
    spec = importlib.util.spec_from_file_location(name.replace('-', '_'), SCRIPTS / (name + '.py'))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result

def write(root, path, data):
    target = root / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(data if isinstance(data, bytes) else data.encode('utf-8'))
    return target

def cli(root, script, *args, ok=True):
    result = subprocess.run([sys.executable, str(SCRIPTS / (script + '.py')), '--root', str(root), *args], capture_output=True, text=True, encoding='utf-8')
    if ok and result.returncode:
        raise AssertionError(result.stdout + result.stderr)
    if not ok and not result.returncode:
        raise AssertionError('unexpected success: ' + result.stdout)
    return result

def install(root):
    shutil.copytree(ROOT / 'harness', root / 'harness', ignore=shutil.ignore_patterns('__pycache__'))
    write(root, '.gitignore', '.harness/\n__pycache__/\n')

def packet(root, name='001-fixture', tier='high-risk', stage='proposal'):
    relative = '.harness/tasks/v0.1/modules/repo-governance/' + name
    fields = f'''schema_version: 1
workflow_tier: {tier}
version: v0.1
packet_module: repo-governance
task_name: {name}
stage: {stage}
proposal: proposal.md
design: design.md
testing: testing.md
testplan: testplan.yaml
risk_profile: risk-profile.yaml
acceptance_report: acceptance-report.md
lifecycle_state: lifecycle.json
changes:
  - id: CHG-fixture
    target_module: repo-governance
    scope_paths: ["sample.txt"]
'''
    path = write(root, relative + '/task.yaml', fields)
    for doc in ('proposal.md', 'design.md', 'testing.md'):
        write(root, relative + '/' + doc, '---\ntask_manifest: task.yaml\nstatus: approved\n---\n\n# Fixture\n\n- Final tier: ' + tier + '\n')
    write(root, relative + '/risk-profile.yaml', 'schema_version: 1\ntask_manifest: task.yaml\n')
    return path

def plan(path, commands=None, mode='enabled', inputs=None):
    commands = commands or [[sys.executable, '-c', 'pass']]
    text = 'schema_version: 1\ntask_manifest: task.yaml\nevidence_inputs: ' + json.dumps(inputs or ['sample.txt']) + '\nlevels:\n'
    for level in ('unit', 'dv', 'integration'):
        text += f'  {level}:\n    mode: {mode}\n    change_ids: ["CHG-fixture"]\n'
        if mode != 'enabled':
            text += '    reason: "isolated manual environment"\n    steps: []\n'
        else:
            text += '    steps:\n'
            for index, command in enumerate(commands):
                text += f'      - id: {level}-{index}\n        name: fixture\n        change_ids: ["CHG-fixture"]\n        run: ' + json.dumps(command) + '\n'
    path.write_text(text, encoding='utf-8')

def complete_packet(root):
    """A substantive toy delivery, completed only through owning entrypoints."""
    path = packet(root)
    rel = path.parent.relative_to(root).as_posix()
    path.write_text(path.read_text().replace('changes:', 'changed_paths_file: .harness/evidence/v0.1/stage-scope/001-fixture.paths\nchanges:'), encoding='utf-8')
    write(root, 'sample.txt', 'delivery')
    front = '---\ntask_manifest: task.yaml\nstatus: approved\n---\n\n# Fixture\n\nRisk profile: ./risk-profile.yaml\n'
    write(root, rel+'/proposal.md', front + '''
- Final tier: high-risk
## Background and Goal
Deliver sample bytes through the governed lifecycle.
## Scope
sample.txt only; no product changes.
## Requirement Review
The requested sample content is unambiguous.
## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|---|---|---|---|---|---|---|
| P-001 | CHG-fixture | sample contains delivery | local file | minimal byte update | compare sample bytes | product changes |
## Success Criteria
sample.txt contains delivery.
## Risks
Byte mismatch is rejected by verification.
''')
    write(root, rel+'/design.md', front.replace('status: approved','status: draft') + '''
## Module Dependencies
| level | parent | node | depends_on |
|---|---|---|---|
| submodule | repo-governance | sample | none |
## File-Level Interfaces
not-applicable: sample.txt is plain data without exported code.
## API and Build Surface Impact
- Public API impact: none
- Crate-root export change: no
- Build-surface change: no
- Documentation examples affected: no
## Key Flows
Write delivery to sample.txt; consumers read the complete bytes.
## State and Ownership
- Owner: fixture sample writer
- Access path: sample.txt is written once and read by verification.
- Transitions and invariants: before becomes delivery; complete bytes remain delivery.
## Directly Mapped Change Items
| change_id | target_module | proposal_id | Design Coverage | Scope Paths |
|---|---|---|---|---|
| CHG-fixture | repo-governance | P-001 | sample byte replacement | sample.txt |
## File-Level Implementation Sequence
| sequence | file_level_module | action | depends_on | change_id |
|---|---|---|---|---|
| 1 | sample.txt | modify | none | CHG-fixture |
''')
    write(root, rel+'/testing.md', front.replace('status: approved','status: draft')+'\nThe plan checks the actual delivered bytes.\n')
    risk='schema_version: 1\ntask_manifest: task.yaml\nchange_ids: ["CHG-fixture"]\nsource_bindings:\n  proposal: proposal.md\n  design_source:\nrisks:\n'
    for category in ('contract','data','security','runtime','build','ui','harness'):
        risk+=f'  {category}:\n    applies: false\n    evidence: Isolated plain sample bytes introduce no {category} boundary or production impact.\n    required_checks: []\n'
    write(root, rel+'/risk-profile.yaml',risk)
    plan(path.parent/'testplan.yaml',[[sys.executable,'-c',"from pathlib import Path; assert Path('sample.txt').read_text() == 'delivery'"]])
    p=path.parent/'testplan.yaml'
    text=p.read_text().replace('task_manifest: task.yaml','task_manifest: task.yaml\nrisk_profile: risk-profile.yaml\napi_impact:\n  public_api: none\n  crate_root_export_change: false\n  build_surface_change: false\n  documentation_examples_affected: false')
    text+='coverage:\n'
    text=text.replace('levels:\n','contract_checks:\n  mode: disabled\n  reason: "Plain sample bytes have no API, compiler or documentation example surface."\n  steps: []\nlevels:\n')
    for level in ('unit','dv','integration'):
        text+=f'''  - id: fixture-{level}
    change_ids: ["CHG-fixture"]
    design_source: "design.md Key Flows and State and Ownership"
    target: "sample byte consumer {level}"
    case_types: ["normal"]
    element_types: ["invariant"]
    level: {level}
    status: covered
    test: "inline fixture byte assertion"
    assertion: "sample contains requested complete bytes"
    step_id: {level}-0
'''
    text+='''  - id: fixture-inapplicable
    change_ids: ["CHG-fixture"]
    design_source: "design.md File-Level Interfaces and State and Ownership"
    target: "plain sample delivery has no executable API, branch, fallible runtime or concurrency boundary"
    case_types: ["boundary", "negative", "error", "compatibility", "lifecycle", "cross-module"]
    element_types: ["parameter-domain", "state-transition", "failure-path", "error-handling", "concurrency"]
    level: dv
    status: not-applicable
    reason: "No source API/runtime is delivered in this fixture; its only production behavior is fixed local sample bytes. Stage transition behavior is asserted by the enclosing governance integration test."
'''
    p.write_text(text,encoding='utf-8')
    return path
