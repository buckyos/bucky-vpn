#!/usr/bin/env python3
"""Preserve legacy Harness materials and rebuild unfinished identities safely.

The migration journal owns copy/verification/cleanup state. Only task-index.py
writes the unfinished index; old lifecycle and run evidence remain archives.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import zipfile


class MigrationError(ValueError):
    pass


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


class LegacyTaskMigration:
    def __init__(self, root: Path, journal: Path):
        self.root = root.resolve()
        self.journal = self.safe(journal if journal.is_absolute() else self.root / journal)
        if not self.journal.is_relative_to(self.root / '.harness'):
            raise MigrationError('journal must stay under .harness')
        self.archive = self.safe(self.root / '.harness/archives/legacy-upgrade-039')
        self.sources = ['docs/versions', 'docs/changes', 'test-results']
        self.state = json.loads(self.journal.read_text(encoding='utf-8')) if self.journal.exists() else None
        if self.state is not None and (self.state.get('schema_version') != 1 or self.state.get('root') != str(self.root)):
            raise MigrationError('journal schema/root mismatch')

    def safe(self, path: Path) -> Path:
        # Reject symlinks at every existing component, including the leaf, before
        # resolving. This also prevents a journal/copy parent from redirecting IO.
        candidate = path if path.is_absolute() else self.root / path
        try:
            candidate.relative_to(self.root)
        except ValueError as error:
            raise MigrationError(f'path outside repository: {candidate}') from error
        for component in (candidate, *candidate.parents):
            if component.is_symlink() or component.is_junction():
                raise MigrationError(f'link/junction is not a migration path: {component}')
            if component == self.root:
                break
        resolved = candidate.resolve()
        if not resolved.is_relative_to(self.root):
            raise MigrationError(f'path escapes repository: {candidate}')
        return resolved

    def path(self, relative: str) -> Path:
        return self.safe(self.root / relative)

    def save(self) -> None:
        self.safe(self.journal)
        self.journal.parent.mkdir(parents=True, exist_ok=True)
        temporary = self.safe(self.journal.with_suffix('.tmp'))
        temporary.write_text(json.dumps(self.state, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
        temporary.replace(self.journal)

    def index(self, *arguments: str) -> dict | None:
        result = subprocess.run([sys.executable, str(self.root / 'harness/scripts/task-index.py'),
                                 '--root', str(self.root), *arguments], capture_output=True, text=True, encoding='utf-8')
        if result.returncode:
            raise MigrationError(result.stderr.strip() or result.stdout.strip())
        return json.loads(result.stdout) if '--json' in arguments else None

    def originals_verified(self) -> None:
        manifest_path = self.root / '.harness/upgrade-039/originals-manifest.json'
        if not manifest_path.exists():
            raise MigrationError('original backup manifest is required before migration')
        manifest = json.loads(manifest_path.read_text(encoding='utf-8'))
        archive = self.path(manifest['archive'])
        if sha(archive.read_bytes()) != manifest['archive_sha256']:
            raise MigrationError('original backup archive hash mismatch')
        with zipfile.ZipFile(archive) as bundle:
            for item in manifest['files']:
                if sha(bundle.read(item['path'])) != item['sha256']:
                    raise MigrationError(f'original backup member mismatch: {item["path"]}')

    def inventory(self) -> dict:
        self.originals_verified()
        if self.state is not None:
            self.preflight()
            return self.state
        old_list = self.path('.harness/upgrade-039/legacy-unfinished-list.json')
        indexed = {item['task_id'] for item in json.loads(old_list.read_text(encoding='utf-8'))['tasks']}
        packets = []
        active = set(indexed)
        for packet in sorted(self.path('docs/versions').glob('*/modules/*/*')):
            if not packet.is_dir() or not re.fullmatch(r'\d{3,}-[a-z0-9][a-z0-9_.-]*', packet.name):
                continue
            self.safe(packet)
            manifest = packet / 'task.yaml'
            text = manifest.read_text(encoding='utf-8') if manifest.is_file() else ''
            def field(key):
                match = re.search(rf'(?m)^{key}:\s*([^\n]*)$', text)
                return match.group(1).strip() if match else None
            reports = [p for p in (packet / 'completion-report.md', packet / 'acceptance-report.md') if p.is_file()]
            decisions = [re.findall(r'(?mi)^\s*-\s*Accepted / rejected / needs changes:\s*(accepted|rejected|needs changes)\s*$', p.read_text(encoding='utf-8')) for p in reports]
            accepted = bool(decisions) and all(values and values[-1] == 'accepted' for values in decisions)
            # Index membership always wins. An implementation stage or absent
            # explicit accepted result keeps directory-discovered work unfinished.
            unfinished = packet.name in indexed or field('stage') == 'implementation' or not accepted
            if unfinished:
                active.add(packet.name)
                if not manifest.is_file():
                    raise MigrationError(f'unfinished packet has no manifest: {packet}')
                if field('workflow_tier') not in {'standard', 'high-risk'}:
                    raise MigrationError(f'unfinished legacy tier needs a user decision: {packet}')
            packets.append({'task_id': packet.name, 'source': packet.relative_to(self.root).as_posix(),
                            'version': packet.parts[-4], 'packet_module': packet.parent.name,
                            'stage': field('stage'), 'tier': field('workflow_tier'), 'has_manifest': manifest.is_file(),
                            'unfinished': unfinished, 'status_provenance': 'old-owning-index' if packet.name in indexed else
                            ('directory-active-stage-or-missing-accepted-result' if unfinished else 'unindexed historical accepted report; no new closeout'),
                            'accepted_report': accepted})
        present = {packet['task_id'] for packet in packets}
        if indexed - present:
            raise MigrationError(f'indexed identities have no packet: {sorted(indexed - present)}')
        files = []
        for source_tree in self.sources:
            tree = self.path(source_tree)
            if not tree.exists():
                continue
            for source in sorted(tree.rglob('*')):
                self.safe(source)
                if not source.is_file():
                    continue
                relative = source.relative_to(self.root).as_posix()
                archive_relative = (self.archive / relative).relative_to(self.root).as_posix()
                destination = archive_relative
                transform = False
                if relative.startswith('docs/changes/'):
                    destination = relative.replace('docs/changes/', '.harness/changes/', 1)
                elif relative.startswith('docs/versions/'):
                    packet = next((p for p in packets if relative.startswith(p['source'] + '/')), None)
                    if packet:
                        suffix = relative[len(packet['source']) + 1:]
                        if packet['unfinished'] and suffix != 'lifecycle.json' and not suffix.startswith('pipeline/'):
                            destination = relative.replace('docs/versions/', '.harness/tasks/', 1)
                            transform = source.suffix in {'.yaml', '.md', '.json', '.txt'}
                        elif not packet['unfinished']:
                            destination = relative.replace('docs/versions/', '.harness/tasks/', 1).replace('/modules/', '/history/modules/', 1)
                    else:
                        # Module-wide proposal/design/testing files are old task
                        # materials, never silently promoted to enduring knowledge.
                        destination = relative.replace('docs/versions/', '.harness/tasks/', 1).replace('/modules/', '/history/modules/', 1)
                        if '/evidence/' in destination:
                            destination = archive_relative
                data = source.read_bytes()
                item = {'source': relative, 'archive': archive_relative, 'destination': destination,
                        'source_sha256': sha(data), 'bytes': len(data), 'transform': transform}
                item['destination_sha256'] = sha(self.normalized(item, data))
                files.append(item)
        self.state = {'schema_version': 1, 'root': str(self.root), 'status': 'inventory', 'files': files,
                      'packets': packets, 'active_tasks': sorted(active), 'completed_copies': [],
                      'recovery_requirements': [{'task_id': p['task_id'], 'status': 'blocked-for-resumption',
                          'reason': 'Old receipt/run fingerprints remain original archives and are invalid at the new path. Reconcile current schema and evidence through the owning task workflow; no product tests or new approvals performed.'}
                          for p in packets if p['unfinished']]}
        self.preflight()
        self.save()
        return self.state

    def normalized(self, item: dict, data: bytes) -> bytes:
        if not item['transform']:
            return data
        text = data.decode('utf-8').replace('docs/versions/', '.harness/tasks/').replace('docs/changes/', '.harness/changes/')
        if item['source'].endswith('/task.yaml'):
            text = re.sub(r'(?m)^(?:mode|auto_pipeline_start_stage|pipeline_plan):[^\n]*\n?', '', text)
        return text.encode('utf-8')

    def frozen_mapping(self) -> tuple[dict, dict]:
        """Reconstruct classification from hash-bound originals, never journal targets."""
        self.originals_verified()
        manifest = json.loads(self.path('.harness/upgrade-039/originals-manifest.json').read_text(encoding='utf-8'))
        original_paths = {item['path'] for item in manifest['files']}
        with zipfile.ZipFile(self.path(manifest['archive'])) as bundle:
            files = {name: bundle.read(name) for name in original_paths if any(name.startswith(tree + '/') for tree in self.sources)}
        # Root run artifacts were outside the initial tracked-doc backup. Their
        # fixed archive retains original bytes; before copying, read the old tree.
        run_archive = self.path('.harness/archives/legacy-upgrade-039/test-results')
        run_source = self.path('test-results')
        for tree in (run_source, run_archive):
            for path in tree.rglob('*') if tree.exists() else []:
                self.safe(path)
                if path.is_file():
                    source = 'test-results/' + path.relative_to(tree).as_posix()
                    data = path.read_bytes()
                    if source in files and files[source] != data:
                        raise MigrationError(f'root run archive differs from original: {source}')
                    files[source] = data
        indexed = set()
        for name, data in files.items():
            if re.fullmatch(r'docs/versions/[^/]+/modules/tasks.json', name):
                indexed.update(item['task_id'] for item in json.loads(data)['tasks'])
        packet_sources = sorted({match.group(1) for name in files if (match := re.match(r'(docs/versions/[^/]+/modules/[^/]+/\d{3,}-[a-z0-9][a-z0-9_.-]*)/', name))})
        packets = {}
        for source in packet_sources:
            text = files.get(source + '/task.yaml', b'').decode('utf-8')
            def field(key):
                match = re.search(rf'(?m)^{key}:\s*([^\n]*)$', text)
                return match.group(1).strip() if match else None
            decisions = [re.findall(r'(?mi)^\s*-\s*Accepted / rejected / needs changes:\s*(accepted|rejected|needs changes)\s*$', files[name].decode('utf-8')) for name in (source + '/completion-report.md', source + '/acceptance-report.md') if name in files]
            accepted = bool(decisions) and all(values and values[-1] == 'accepted' for values in decisions)
            task_id = source.rsplit('/', 1)[-1]
            unfinished = task_id in indexed or field('stage') == 'implementation' or not accepted
            packets[source] = {'task_id': task_id, 'source': source, 'version': source.split('/')[2],
                               'packet_module': source.split('/')[4], 'stage': field('stage'), 'tier': field('workflow_tier'),
                               'has_manifest': source + '/task.yaml' in files, 'unfinished': unfinished,
                               'accepted_report': accepted,
                               'status_provenance': 'old-owning-index' if task_id in indexed else
                               ('directory-active-stage-or-missing-accepted-result' if unfinished else 'unindexed historical accepted report; no new closeout')}
        mappings = {}
        for source, data in files.items():
            archive = '.harness/archives/legacy-upgrade-039/' + source
            destination, transform = archive, False
            if source.startswith('docs/changes/'):
                destination = source.replace('docs/changes/', '.harness/changes/', 1)
            elif source.startswith('docs/versions/'):
                packet = next((packet for prefix, packet in packets.items() if source.startswith(prefix + '/')), None)
                if packet:
                    suffix = source[len(packet['source']) + 1:]
                    if packet['unfinished'] and suffix != 'lifecycle.json' and not suffix.startswith('pipeline/'):
                        destination = source.replace('docs/versions/', '.harness/tasks/', 1)
                        transform = Path(source).suffix in {'.yaml', '.md', '.json', '.txt'}
                    elif not packet['unfinished']:
                        destination = source.replace('docs/versions/', '.harness/tasks/', 1).replace('/modules/', '/history/modules/', 1)
                else:
                    destination = source.replace('docs/versions/', '.harness/tasks/', 1).replace('/modules/', '/history/modules/', 1)
                    if '/evidence/' in destination:
                        destination = archive
            item = {'source': source, 'archive': archive, 'destination': destination,
                    'source_sha256': sha(data), 'bytes': len(data), 'transform': transform}
            item['destination_sha256'] = sha(self.normalized(item, data))
            mappings[source] = item
        return mappings, packets

    def validate_mapping_contract(self) -> None:
        mappings, packets = self.frozen_mapping()
        records = self.state['files']
        if not isinstance(records, list) or len(records) != len(mappings):
            raise MigrationError('journal does not cover the complete frozen source inventory')
        seen, targets = set(), set()
        for item in records:
            source = item['source']
            if source in seen or source not in mappings or item != mappings[source] or type(item['transform']) is not bool or type(item['bytes']) is not int:
                raise MigrationError(f'journal mapping differs from frozen originals: {source}')
            seen.add(source)
            for key in ('source', 'archive', 'destination'):
                relative = item[key]
                if Path(relative).as_posix() != relative or '..' in Path(relative).parts or Path(relative).is_absolute():
                    raise MigrationError(f'noncanonical mapping path: {relative}')
                self.path(relative)
            if item['destination'] in targets:
                raise MigrationError(f'duplicate migration destination: {item["destination"]}')
            targets.add(item['destination'])
        actual_packets = self.state['packets']
        if not isinstance(actual_packets, list) or len(actual_packets) != len(packets) or {p['source'] for p in actual_packets} != set(packets):
            raise MigrationError('journal packet inventory differs from frozen originals')
        if any(packet != packets[packet['source']] for packet in actual_packets):
            raise MigrationError('journal packet classification differs from frozen originals')
        if self.state['active_tasks'] != sorted(p['task_id'] for p in packets.values() if p['unfinished']):
            raise MigrationError('journal active identities differ from frozen originals')

    def preflight(self) -> None:
        if self.state is None:
            return
        self.validate_mapping_contract()
        for item in self.state['files']:
            source = self.path(item['source'])
            if source.exists() and sha(source.read_bytes()) != item['source_sha256']:
                raise MigrationError(f'legacy source changed: {item["source"]}')
            for key, expected in [('archive', 'source_sha256'), ('destination', 'destination_sha256')]:
                target = self.path(item[key])
                if target.exists() and (not target.is_file() or sha(target.read_bytes()) != item[expected]):
                    raise MigrationError(f'conflicting migration target: {item[key]}')
            if not source.is_file() and not self.path(item['archive']).is_file():
                raise MigrationError(f'neither original nor archive exists: {item["source"]}')
        # Detect additions after inventory before deleting a whole legacy tree.
        known = {item['source'] for item in self.state['files']}
        for source_tree in self.sources:
            tree = self.path(source_tree)
            for source in tree.rglob('*') if tree.exists() else []:
                self.safe(source)
                if source.is_file() and source.relative_to(self.root).as_posix() not in known:
                    raise MigrationError(f'new uninventoried source: {source}')

    def apply(self) -> None:
        self.inventory()
        if self.state['status'] == 'cleaned':
            self.verify()
            return
        self.preflight()  # All target collisions are rejected before the first copy.
        for item in self.state['files']:
            source = self.path(item['source'])
            archive = self.path(item['archive'])
            data = source.read_bytes() if source.exists() else archive.read_bytes()
            if sha(data) != item['source_sha256']:
                raise MigrationError(f'source changed during copy: {source}')
            for target, payload in [(archive, data), (self.path(item['destination']), self.normalized(item, data))]:
                if target.exists():
                    if target.read_bytes() != payload:
                        raise MigrationError(f'copy collision: {target}')
                else:
                    self.safe(target)
                    target.parent.mkdir(parents=True, exist_ok=True)
                    # Exclusive creation prevents a concurrent writer being overwritten.
                    with target.open('xb') as handle:
                        handle.write(payload)
            if item['source'] not in self.state['completed_copies']:
                self.state['completed_copies'].append(item['source'])
            self.save()
        self.state['status'] = 'copied'
        self.save()
        self.verify(indexes=False)
        versions = sorted({p['version'] for p in self.state['packets']})
        for version in versions:
            self.index('init', '--version', version)  # Existing 039 remains selected.
        for packet in self.state['packets']:
            if packet['unfinished']:
                task = packet['source'].replace('docs/versions/', '.harness/tasks/', 1) + '/task.yaml'
                self.index('add', '--task', task)
        self.state['status'] = 'indexed'
        self.save()
        self.verify()

    def verify(self, *, indexes: bool = True) -> None:
        if self.state is None:
            raise MigrationError('inventory has not been created')
        self.originals_verified()
        self.preflight()
        for item in self.state['files']:
            for key, expected in [('archive', 'source_sha256'), ('destination', 'destination_sha256')]:
                target = self.path(item[key])
                if not target.is_file() or sha(target.read_bytes()) != item[expected]:
                    raise MigrationError(f'missing or invalid verified copy: {target}')
        protected = self.path('.harness/upgrade-039/protected-files.json')
        for item in json.loads(protected.read_text(encoding='utf-8'))['files']:
            if sha(self.path(item['path']).read_bytes()) != item['sha256']:
                raise MigrationError(f'protected project bytes changed: {item["path"]}')
        if indexes:
            for packet in self.state['packets']:
                if packet['unfinished']:
                    self.index('contains', '--task', packet['source'].replace('docs/versions/', '.harness/tasks/', 1) + '/task.yaml')
        if self.state['status'] not in {'indexed', 'cleaned'}:
            self.state['status'] = 'verified'
        if self.state['status'] != 'cleaned':
            self.save()

    def cleanup(self) -> None:
        self.verify()
        if self.state['status'] not in {'indexed', 'cleaned'}:
            raise MigrationError('cleanup requires indexed verified migration')
        for relative in self.sources:
            target = self.path(relative)
            expected = (self.root / relative).resolve()
            if target != expected or target == self.root or not target.is_relative_to(self.root):
                raise MigrationError(f'unsafe recursive cleanup target: {target}')
            if target.exists():
                self.verify()  # Reconcile all remaining sources immediately before deletion.
                shutil.rmtree(target)
        self.state['status'] = 'cleaned'
        self.save()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['inventory', 'apply', 'verify', 'cleanup'])
    parser.add_argument('--root', type=Path, default=Path('.'))
    parser.add_argument('--journal', type=Path, default=Path('.harness/upgrade-039/migration-journal.json'))
    args = parser.parse_args()
    try:
        migration = LegacyTaskMigration(args.root, args.journal)
        result = getattr(migration, args.command)()
        state = result or migration.state
        print(f'legacy-task-migrate: {args.command} passed; {len(state["files"])} files, {len(state["packets"])} packets, {len(state["active_tasks"])} unfinished identities; status={state["status"]}')
        return 0
    except (MigrationError, OSError, ValueError, KeyError, zipfile.BadZipFile) as error:
        print(f'legacy-task-migrate: {error}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
