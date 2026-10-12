#!/usr/bin/env python3
"""Install pinned public model assets; never sends user text."""
import argparse
import hashlib
from pathlib import Path
import urllib.request

REVISION = '6ddaeb9413b0e71ad8457da1aab378a165b24058'
ASSETS = [
    ('onnx/model.onnx', 1157129714, '57af7f3d8d72a3ca4e5619c247344b4b5ee362a5ebcf307c0634d67b65e721c0'),
    ('tokenizer.json', 16331948, '914bd3c8fb7b525af9e23b60d0ec7b1248ddb2b99014efd9c02ebeb022f8cab7'),
]
DESTINATION = Path(__file__).resolve().parent.parent / 'models' / 'gliner-multi'
arguments = argparse.ArgumentParser()
arguments.add_argument('--verify-only', action='store_true')
arguments.add_argument('--int8', action='store_true', help='Optional evaluation model; application continues using FP32.')
arguments.add_argument('--q4', action='store_true', help='Optional weight-only evaluation model; application continues using FP32.')
options = arguments.parse_args()
DESTINATION.mkdir(parents=True, exist_ok=True)
if options.int8:
    ASSETS.append(('onnx/model_int8.onnx', 349120924, '995058c82c5f570601dd8a0ba74ee60a392f764268bc5f628455e44dd3b476ec'))
if options.q4:
    ASSETS.append(('onnx/model_q4.onnx', 893933650, '3c54cf1332d491f7f4057504df4ac78a78712874aca1c5731330430e0c55259b'))

for remote, size, expected in ASSETS:
    local_name = {'onnx/model_int8.onnx': 'model-int8.onnx', 'onnx/model_q4.onnx': 'model-q4.onnx'}.get(remote, Path(remote).name)
    target = DESTINATION / local_name
    if target.exists() and target.stat().st_size == size:
        with target.open('rb') as source:
            if hashlib.file_digest(source, 'sha256').hexdigest() == expected:
                print(f'Verified {target.name}', flush=True)
                continue
    if options.verify_only:
        raise SystemExit(f'Missing or invalid model asset: {target.name}')
    partial = target.with_suffix(target.suffix + '.partial')
    url = f'https://huggingface.co/onnx-community/gliner_multi-v2.1/resolve/{REVISION}/{remote}'
    print(f'Downloading {target.name} ({size} bytes)', flush=True)
    digest = hashlib.sha256()
    total = 0
    with urllib.request.urlopen(url, timeout=90) as source, partial.open('wb') as output:
        while block := source.read(1024 * 1024):
            output.write(block)
            digest.update(block)
            total += len(block)
    if total != size or digest.hexdigest() != expected:
        partial.unlink(missing_ok=True)
        raise SystemExit(f'Checksum or size mismatch: {target.name}')
    partial.replace(target)
    print(f'Verified {target.name}', flush=True)
