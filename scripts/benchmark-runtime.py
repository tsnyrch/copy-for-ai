#!/usr/bin/env python3
"""Run serial, alternating ORT comparisons on synthetic inputs only."""
import argparse
import json
import os
from pathlib import Path
import statistics
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('--baseline', type=Path, required=True)
parser.add_argument('--candidate', type=Path, required=True)
parser.add_argument('--library', type=Path, required=True)
parser.add_argument('--model-dir', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=True)
rows = []
for size, pairs in [(1024, 1), (10240, 1), (102400, 3)]:
    for pair in range(pairs):
        order = [('baseline', args.baseline), ('candidate', args.candidate)]
        if pair % 2:
            order.reverse()
        for label, executable in order:
            env = dict(os.environ, COPY_FOR_AI_MODEL_DIR=str(args.model_dir.resolve()))
            env.pop('ORT_DYLIB_PATH', None)
            if label == 'candidate':
                env['ORT_DYLIB_PATH'] = str(args.library.resolve())
            path = args.output / f'{label}-{size}-{pair}.txt'
            with path.open('w') as log:
                process = subprocess.Popen([str(executable.resolve()), str(size), '256', '1', '8', 'fp32', 'varied'], stdout=log, stderr=log, env=env)
                _, status, usage = os.wait4(process.pid, 0)
                process.returncode = os.waitstatus_to_exitcode(status)
            content = path.read_text()
            warm = next((line for line in content.splitlines() if line.startswith('run=1 ')), '')
            values = dict(part.split('=', 1) for part in warm.split() if '=' in part)
            row = dict(label=label, bytes=size, pair=pair, exit_code=process.returncode, peak_rss=usage.ru_maxrss, **{k: v for k, v in values.items() if k != 'bytes'})
            rows.append(row)
            (args.output / 'summary.json').write_text(json.dumps(rows, indent=2))
            print(label, size, pair, values.get('detect_ms'), 'ms', values.get('status'), flush=True)
            if process.returncode or values.get('status') != 'ok' or values.get('reused') != '0':
                raise SystemExit('comparison failed; inspect metadata logs')
for label in ['baseline', 'candidate']:
    samples = [int(row['detect_ms']) for row in rows if row['label'] == label and row['bytes'] == 102400]
    print(label, '100KiB median_ms', statistics.median(samples), flush=True)
