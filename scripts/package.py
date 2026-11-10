"""Package a native driver binary with an explicit platform and a fixed whitelist."""
import argparse
import os
import platform
import zipfile
from pathlib import Path

root = Path(__file__).resolve().parents[1]
labels = {
    ('Linux', 'x86_64'): 'linux-x64',
    ('Linux', 'aarch64'): 'linux-arm64',
    ('Darwin', 'x86_64'): 'darwin-x64',
    ('Darwin', 'arm64'): 'darwin-arm64',
    ('Windows', 'AMD64'): 'win-x64',
}
native = labels.get((platform.system(), platform.machine()))
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--platform', default=native, choices=sorted(set(labels.values())))
parser.add_argument('--binary', type=Path)
args = parser.parse_args()
if not native or args.platform != native:
    parser.error(f'Package on the matching native host: detected {native}, requested {args.platform}')
name = 'meilisearch.exe' if args.platform == 'win-x64' else 'meilisearch'
binary = args.binary or root / 'target' / 'release' / name
archive = root / f'meilisearch-{args.platform}.zip'
stage = archive.with_suffix('.zip.new')
try:
    with zipfile.ZipFile(stage, 'w', zipfile.ZIP_DEFLATED) as z:
        z.write(binary, name)
        for name in ['.tabularium', 'README.md', 'TESTING.md', 'LICENSE']:
            z.write(root / name, name)
        for folder in ['assets', 'screenshots']:
            for path in sorted((root / folder).rglob('*')):
                if path.is_file():
                    z.write(path, path.relative_to(root))
    os.replace(stage, archive)
finally:
    if stage.exists():
        stage.unlink()
print(archive)
