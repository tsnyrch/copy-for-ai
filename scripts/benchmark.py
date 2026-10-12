#!/usr/bin/env python3
"""Isolated synthetic benchmark. Reports sizes, counts, timings and process RSS only."""
import argparse
import os
from pathlib import Path
import subprocess
import sys

parser = argparse.ArgumentParser()
parser.add_argument('--profiles', nargs='+', default=['debug', 'release'])
parser.add_argument('--sizes', nargs='+', type=int, default=[1024, 10240, 102400, 262144, 1048576])
parser.add_argument('--varied', action='store_true')
parser.add_argument('--output', type=Path, default=Path('docs/benchmarks'))
parser.add_argument('--binary', type=Path)
parser.add_argument('--tag', default='after')
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=True)
for profile in args.profiles:
    binary = args.binary or Path(f'target/{profile}/examples/benchmark')
    if not args.binary:
        subprocess.run(['cargo', 'build', '-p', 'copy-for-ai-ner', '--example', 'benchmark', '--locked', '--offline'] + (['--release'] if profile == 'release' else []), check=True)
    for size in args.sizes:
        target = args.output / f'{args.tag}-{profile}-{"varied" if args.varied else "repeated"}-{size}.txt'
        with target.open('w') as output:
            process = subprocess.Popen([str(binary.resolve()), str(size), '256', '1', '8', 'fp32'] + (['varied'] if args.varied else []), stdout=output, stderr=output)
            _, status, usage = os.wait4(process.pid, 0)
            process.returncode = os.waitstatus_to_exitcode(status)
            # macOS reports bytes; Linux reports KiB.
            peak_bytes = int(usage.ru_maxrss) * (1 if sys.platform == 'darwin' else 1024)
            output.write(f'peak_rss_bytes={peak_bytes} exit_code={process.returncode}\n')
        print(target, flush=True)
        if process.returncode:
            raise SystemExit(process.returncode)
