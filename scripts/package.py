"""Package the verified Linux x64 driver without development files or credentials."""
import os, platform, zipfile
from pathlib import Path

root=Path(__file__).resolve().parents[1]
if platform.system()!='Linux' or platform.machine() not in ('x86_64','AMD64'):
    raise SystemExit('This local packager targets Linux x64 only')
archive=root/'meilisearch-linux-x64.zip'
stage=archive.with_suffix('.zip.new')
try:
    with zipfile.ZipFile(stage,'w',zipfile.ZIP_DEFLATED) as z:
        z.write(root/'target/release/meilisearch','meilisearch')
        for name in ['.tabularium','README.md','TESTING.md','LICENSE']:
            z.write(root/name,name)
        for folder in ['assets','screenshots']:
            for path in sorted((root/folder).rglob('*')):
                if path.is_file():z.write(path,path.relative_to(root))
    os.replace(stage,archive)
finally:
    if stage.exists():stage.unlink()
print(archive)
