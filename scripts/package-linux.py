#!/usr/bin/env python3
"""Build a local Linux package; retain predecessors until explicit approval."""
import datetime
import hashlib
import json
import shutil
import subprocess
import tarfile
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def publish(stage, dist):
    previous = [p for p in dist.glob('rustrdp-linux*') if p.is_file()]
    if previous:
        archive = dist / 'archive' / datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')
        archive.mkdir(parents=True)
        for path in previous:
            path.rename(archive / path.name)
    for path in stage.iterdir():
        path.rename(dist / path.name)


def main():
    subprocess.run(['cargo', 'build', '--release', '--locked'], cwd=ROOT, check=True)
    dist = ROOT / 'dist'
    dist.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='.package-', dir=dist) as temp:
        stage = Path(temp)
        bundle = stage / 'rustrdp-linux'
        bundle.mkdir()
        shutil.copy2(ROOT / 'target/release/rustrdp', bundle / 'rustrdp')
        for name in ['LICENSE', 'NOTICE', 'THIRD_PARTY_NOTICES.md']:
            shutil.copy2(ROOT / name, bundle / name)
        shutil.copytree(ROOT / 'assets', bundle / 'assets')
        shutil.copytree(ROOT / 'packaging', bundle / 'packaging')
        (bundle / 'README.txt').write_text('Local Linux build. Run ./rustrdp. FreeRDP and Qt/KDE runtime dependencies are installed separately; see docs/BUILDING.md in source. Not a public redistribution compliance bundle.\n')
        provenance = {
            'checkout': str(ROOT),
            'head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
            'working_tree': subprocess.check_output(['git', 'status', '--short'], cwd=ROOT, text=True),
            'binary_sha256': hashlib.sha256((bundle / 'rustrdp').read_bytes()).hexdigest(),
        }
        (bundle / 'build.json').write_text(json.dumps(provenance, indent=2) + '\n')
        artifact = stage / 'rustrdp-linux.tar.gz'
        with tarfile.open(artifact, 'w:gz') as out:
            out.add(bundle, arcname=bundle.name)
        with tarfile.open(artifact) as check:
            binary = check.extractfile('rustrdp-linux/rustrdp')
            assert binary is not None
            assert binary.read() == (bundle / 'rustrdp').read_bytes()
        shutil.rmtree(bundle)
        (stage / 'rustrdp-linux.sha256').write_text(hashlib.sha256(artifact.read_bytes()).hexdigest() + '  ' + artifact.name + '\n')
        publish(stage, dist)
    print(dist / 'rustrdp-linux.tar.gz')


if __name__ == '__main__':
    main()
