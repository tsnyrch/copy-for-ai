#!/usr/bin/env python3
"""Verify and unpack the optional official ARM64 ORT probe, never the app bundle."""
import argparse
import hashlib
from pathlib import Path
import tarfile

VERSION = '1.31.0'
ARCHIVE_SHA256 = '1031b36dd3bdaa8d976a6130d9cf2a05e9be320b93ae804f8427b91dc3432afc'
LIBRARY_SHA256 = '2a50a63e7f41f8d4d551d3ee47248df3864f6540109fce8ad520ca3b51af4b70'
parser = argparse.ArgumentParser()
parser.add_argument('--archive', type=Path, required=True)
parser.add_argument('--destination', type=Path, required=True)
args = parser.parse_args()

def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

if digest(args.archive) != ARCHIVE_SHA256:
    raise SystemExit('ORT archive checksum mismatch')
args.destination.mkdir(parents=True, exist_ok=True)
with tarfile.open(args.archive) as archive:
    archive.extractall(args.destination, filter='data')
root = args.destination / f'onnxruntime-osx-arm64-{VERSION}'
library = root / 'lib' / f'libonnxruntime.{VERSION}.dylib'
if (root / 'VERSION_NUMBER').read_text().strip() != VERSION or digest(library) != LIBRARY_SHA256:
    raise SystemExit('ORT library verification failed')
print(library.resolve())
