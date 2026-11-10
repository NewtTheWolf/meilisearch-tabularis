"""Validate all native verification reports before publishing their exact archives."""
import argparse
import hashlib
import json
import subprocess
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--artifacts', required=True, type=Path)
parser.add_argument('--tag', required=True)
args = parser.parse_args()
manifest = Path('.tabularium')
version = json.loads(manifest.read_text(encoding='utf-8'))['version']
if args.tag != 'v' + version:
    raise SystemExit('Tag and manifest version differ')
platforms = ['linux-x64', 'linux-arm64', 'darwin-x64', 'darwin-arm64', 'win-x64']
files = []
reports = []
for label in platforms:
    archive = args.artifacts / f'meilisearch-{label}.zip'
    report_file = args.artifacts / '.ci-meili' / f'{label}-verification.json'
    report = json.loads(report_file.read_text(encoding='utf-8'))
    expected_hosts = {
        'linux-x64': ('Linux', 'x86_64'), 'linux-arm64': ('Linux', 'aarch64'),
        'darwin-x64': ('Darwin', 'x86_64'), 'darwin-arm64': ('Darwin', 'arm64'),
        'win-x64': ('Windows', 'AMD64'),
    }
    if (report['host']['system'], report['host']['machine']) != expected_hosts[label]:
        raise SystemExit(f'Wrong native host for {label}')
    if report['platform'] != label or report['version'] != version:
        raise SystemExit(f'Wrong platform/version in {report_file}')
    if report['builtBinaryLiveTests'] != 8 or report['extractedArchiveLiveTests'] != 8:
        raise SystemExit(f'Incomplete live tests for {label}')
    if hashlib.sha256(archive.read_bytes()).hexdigest() != report['sha256']:
        raise SystemExit(f'Archive hash mismatch for {label}')
    files.append(archive)
    reports.append(report)
standalone = Path('default.tabularium')
standalone.write_bytes(manifest.read_bytes())
files.append(standalone)
checksums = Path('SHA256SUMS')
checksums.write_text(''.join(hashlib.sha256(p.read_bytes()).hexdigest() + '  ' + p.name + '\n' for p in files), encoding='utf-8')
files.append(checksums)
notes = ('Native builds and unit tests passed on all five matching runners. Each built binary and extracted ZIP passed eight live JSON-RPC tests against Meilisearch v1.56.2. '
         'The Apple Silicon driver runs natively; the local x64 Meilisearch test server uses Rosetta. '
         'Desktop GUI E2E was verified on Linux only. See TESTING.md and the verification records below for evidence and limits.')
notes += '\n\n## Native verification records\n\n```json\n' + json.dumps(reports, indent=2) + '\n```\n'
subprocess.run(['gh', 'release', 'create', args.tag, '--verify-tag', '--title', 'Meilisearch ' + version + ' native platform packages', '--notes', notes, *map(str, files)], check=True)
