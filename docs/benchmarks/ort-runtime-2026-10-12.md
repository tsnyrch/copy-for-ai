# ORT 1.31.0, progress and cache — 2026-10-12

## Result

Implemented scalar diagnostics, nonblocking mailbox progress, an exact cache of
the last successful detection, explicit clipboard processing and binary marker
checks. Cancelled work without an accepted worker report shows diagnostics as
unavailable, not measured zeroes. Production stays ORT 1.20.0 / FP32; ORT 1.31.0
is an isolated verified experiment. The production .app/process was neither
changed nor restarted.

Machine: Apple M5, 10 CPU cores, 32 GiB RAM, macOS 27.0.1. FP32 model/tokenizer,
eight labels, threshold 0.4, 256 subwords, batch one, eight threads. Both used
gline-rs 1.1.0 and identical Rust ORT rc.9 bindings. The new library served their
API successfully without mixing two runtime types. Exact hashes/build versions:
[manifest](ort-1.31-2026-10-12/manifest.json).

## Alternating performance runs

Fresh processes, two passes per input; table shows warm run one. 100 KiB order
A/B, B/A, A/B. Varied input, **reuse=0**. No other owned builds/inference ran during
this suite. User processes/filesystem cache were uncontrolled. Raw phases/load/RSS:
[summary](ort-1.31-2026-10-12/summary.json).

| Input | ORT 1.20.0 warm ms | ORT 1.31.0 warm ms |
| --- | ---: | ---: |
| 1 KiB, one pair | 174 | 64 |
| 10 KiB, one pair | 2098 | 887 |
| 100 KiB, pair 1 | 19,285 | 8904 |
| 100 KiB, pair 2 | 19,902 | 8979 |
| 100 KiB, pair 3 | 20,447 | 8872 |
| 100 KiB median | **19,902** | **8904** |

Median speedup: 2.235×, 55.3% less time. Three samples are not p95 or an app latency
guarantee. 100 KiB process RSS: baseline 3,315,466,240–3,492,954,112 bytes; candidate
3,331,244,032–3,334,406,144. No memory benefit is inferred. Both replaced spans and
restored byte-for-byte.

## Quality gate and 1 MiB

The same 21 existing synthetic gold expectations at four positions yield 84
CS/EN/DE/FR/RU/JA/ZH cases. Check exact detection and coverage by any span per case;
restoration was exact.

| Result | Baseline | Candidate |
| --- | ---: | ---: |
| Missing exact gold span/kind | 13 | 13 |
| Uncovered gold span | 11 | 11 |
| New exact detection losses | — | 0 |
| New coverage losses | — | 0 |

[Baseline](ort-1.31-2026-10-12/baseline-quality.txt),
[candidate](ort-1.31-2026-10-12/candidate-quality.txt),
[case comparison](ort-1.31-2026-10-12/quality-summary.json).
This is not full precision/recall or complete anonymization. Figures apply only
to this suite.

After passing, one full varied 1 MiB candidate run completed:

- Detection 93,356 ms; inference 93,185 ms; all 1143 windows inferred, reuse zero;
  26,471 model spans.
- Core 31 ms; restoration 5 ms, byte-for-byte.
- File verification 2021 ms and load 602 ms separately. A short model warmup
  preceded the document; OS filesystem cache was not empty.
- Peak process RSS 3,337,584,640 bytes (~3.11 GiB); subsequent cancellation probe
  returned Cancelled after 315 ms.

[Raw 1 MiB](ort-1.31-2026-10-12/candidate-mebibyte.txt). A core-example compilation
and Clippy overlapped part of this run, so it is not a controlled paired 1 MiB
comparison. Historical ORT 1.20.0 308,388 ms is a different sample. The alternating
100 KiB suite is stronger speed evidence. Foundation reported inability to open
its cache database in the isolated sandbox; the operation still completed
status=ok, exit zero. This was not clipboard data.

## Algorithms and cache

Marker checks use first/last `partition_point` intersections, O(log p) per
candidate. Larger Secret spans may fully enclose multiple markers; exact marker
or partial overlaps are rejected. Overlap selection remains O(c log c), with
replacement/restoration appending to one buffer.

Adversarial cases: incomplete email domain, repeated PEM BEGIN without END,
credentials without value and a long credential. Three samples at 64/256/1024 KiB
all restored exactly: [raw](ort-1.31-2026-10-12/regex-adversarial.txt). At 1 MiB,
medians were 7.56 / 7.66 / 5.49 / 62.33 ms. These cases did not justify rewriting
regexes or establish a universal worst-case guarantee. Some overlapped owned CPU
work. Patterns and semantics remain unchanged.

The cache owns one zeroizing input plus model spans. Match input bytes, verified
artifact/config identity and invalidation epoch exactly. The 8 MiB bound includes
buffer capacities and entry overhead; oversize results simply skip caching.
Replacements use current rules/session. Cancellation immediately invalidates the
epoch; Forget everything also frees worker cache. Old cleanup cannot delete a
newer entry. Model-independent restoration invalidates old cache without waiting
for the model lock. ORT kernels are synchronous; zeroizing one buffer does not
erase every copy.

Actual FP32 service test: repeating input after a new session and changed rules
took 355 µs, inferred zero/reused one, restored exactly. A one-byte change missed
cache. Forgetting during real inference returned Cancelled and the next operation
had no cache hit. [Raw](ort-1.31-2026-10-12/real-cache-test.txt). 355 µs is a small
synthetic service result, not UI or 1 MiB latency.

## Reproduction

Download only the official ARM64 artifact from the manifest URL; verify archive
and library using install-runtime-probe.py. Runtime processing downloads no model.
The probe feature is opt-in.

```sh
python3 scripts/install-runtime-probe.py --archive /private/tmp/copy-ai-ort-1.31.0.tgz --destination /private/tmp/copy-ai-ort-probe
cargo build -p copy-for-ai-ner --release --examples --offline
cp target/release/examples/benchmark /private/tmp/copy-ai-baseline-benchmark
cargo build -p copy-for-ai-ner --release --examples --features runtime-probe --offline --target-dir /private/tmp/copy-ai-probe-target
python3 scripts/benchmark-runtime.py --baseline /private/tmp/copy-ai-baseline-benchmark --candidate /private/tmp/copy-ai-probe-target/release/examples/benchmark --library /private/tmp/copy-ai-ort-probe/onnxruntime-osx-arm64-1.31.0/lib/libonnxruntime.1.31.0.dylib --model-dir models/gliner-multi --output /private/tmp/copy-ai-runtime-reproduction
```

`quality_compare runtime gate` snapshots one FP32 variant on existing gold cases.
Set ORT_DYLIB_PATH to the verified library for the candidate executable. Do not run
model/long-input experiments alongside other owned builds or benchmarks.

## Native evidence boundary

An isolated debug preview in /private/tmp used temporary preferences and synthetic
data only. CUA inspected Workspace, the new action, history diagnostics and a
completed 97-byte real-model preview. Only the owned test process was then closed.
Production app, System Settings and the user's clipboard were not controlled.

Headless GPUI verifies clipboard-only completion without target/Accessibility,
clipboard/revision rejection and protected-paste checks. After native failure,
clear only the unchanged result owned by this operation, never a newer clipboard.
Do not accept the vault or restore original text as fallback.

**Not verified:** actual system paste, installed-bundle TCC/Accessibility and long
native visual progress. Real-inference cancellation/progress was verified at
service level, not system paste. ORT 1.31.0 is not deployed in the production
bundle; distribution/signing remain separate work.
