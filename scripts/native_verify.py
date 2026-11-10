"""Start a disposable native Meilisearch and verify built and packaged drivers."""
import argparse
import hashlib
import json
import os
import platform
import socket
import subprocess
import sys
import tempfile
import time
import urllib.request
import zipfile
from pathlib import Path

root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--server', required=True, type=Path)
parser.add_argument('--binary', required=True, type=Path)
parser.add_argument('--archive', required=True, type=Path)
parser.add_argument('--port', type=int, default=17700)
args = parser.parse_args()
server_path = args.server.resolve()
binary = args.binary.resolve()
archive = args.archive.resolve()
output = root / '.ci-meili'
output.mkdir(exist_ok=True)
label = archive.stem.removeprefix('meilisearch-')
report = {
    'platform': label,
    'host': {'system': platform.system(), 'machine': platform.machine()},
    'meilisearch': 'v1.56.2',
    'desktopGuiTested': False,
    'archive': archive.name,
}
server_path.chmod(0o755)
env = {**os.environ,
       'MEILI_HTTP_ADDR': f'127.0.0.1:{args.port}',
       'MEILI_TEST_URL': f'http://127.0.0.1:{args.port}',
       'MEILI_ENV': 'development',
       'MEILI_MASTER_KEY': 'tabularis-meili-local-demo-key',
       'MEILI_NO_ANALYTICS': 'true',
       'PYTHONUTF8': '1'}
with socket.socket() as probe:
    probe.bind(('127.0.0.1', args.port))
with tempfile.TemporaryDirectory(prefix='meili-native-') as work:
    env['MEILI_DB_PATH'] = str(Path(work) / 'data.ms')
    with (output / f'{label}-server.log').open('w', encoding='utf-8') as log:
        server = subprocess.Popen([str(server_path)], cwd=work, env=env,
                                  stdout=log, stderr=subprocess.STDOUT)
        try:
            for attempt in range(120):
                if server.poll() is not None:
                    raise RuntimeError(f'Meilisearch exited with {server.returncode}; inspect server log')
                try:
                    with urllib.request.urlopen(env['MEILI_TEST_URL'] + '/health', timeout=1):
                        break
                except OSError:
                    if attempt == 119:
                        raise TimeoutError('Native Meilisearch did not become ready')
                    time.sleep(.25)
            subprocess.run([sys.executable, 'tests/seed.py'], cwd=root, env=env, check=True)
            env['MEILI_TEST_BINARY'] = str(binary)
            subprocess.run([sys.executable, 'tests/live.py'], cwd=root, env=env, check=True)
            report['builtBinaryLiveTests'] = 8
            with tempfile.TemporaryDirectory(prefix='meili-extracted-') as extracted:
                with zipfile.ZipFile(archive) as z:
                    expected = {'meilisearch.exe' if os.name == 'nt' else 'meilisearch',
                                '.tabularium', 'README.md', 'TESTING.md', 'LICENSE',
                                'assets/icon.svg', 'screenshots/browse.png', 'screenshots/search.png'}
                    names = set(z.namelist())
                    if names != expected:
                        raise AssertionError(f'Archive whitelist mismatch: {names ^ expected}')
                    manifest = json.loads(z.read('.tabularium'))
                    report['version'] = manifest['version']
                    z.extractall(extracted)
                exe = Path(extracted) / ('meilisearch.exe' if os.name == 'nt' else 'meilisearch')
                exe.chmod(0o755)
                env['MEILI_TEST_BINARY'] = str(exe)
                subprocess.run([sys.executable, 'tests/live.py'], cwd=root, env=env, check=True)
                report['extractedArchiveLiveTests'] = 8
            report['sha256'] = hashlib.sha256(archive.read_bytes()).hexdigest()
        finally:
            server.terminate()
            try:
                server.wait(timeout=10)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait(timeout=10)
with (output / f'{label}-verification.json').open('w', encoding='utf-8') as f:
    json.dump(report, f, indent=2)
print(json.dumps(report, indent=2))
