#!/usr/bin/env python3
"""Package a compiled zed-cc release binary for GitHub Actions."""
import argparse
from pathlib import Path
import tarfile
import zipfile


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--target', required=True)
    parser.add_argument('--output-dir', default='dist')
    args = parser.parse_args()

    is_windows = 'windows' in args.target
    binary_name = 'zed-cc.exe' if is_windows else 'zed-cc'
    binary = Path('target') / args.target / 'release' / binary_name
    if not binary.is_file():
        parser.error(f'Missing release executable: {binary}')
    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    basename = f'zed-cc-{args.target}'
    if is_windows:
        artifact = output_dir / f'{basename}.zip'
        with zipfile.ZipFile(artifact, 'w', zipfile.ZIP_DEFLATED) as archive:
            archive.write(binary, binary_name)
    else:
        artifact = output_dir / f'{basename}.tar.gz'
        with tarfile.open(artifact, 'w:gz') as archive:
            archive.add(binary, arcname=binary_name)
    print(artifact)


if __name__ == '__main__':
    main()
