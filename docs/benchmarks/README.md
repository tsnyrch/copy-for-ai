# Local anonymization — before/after measurements

## Reports

- [Window parallelism and completed 50 MiB runs](window-parallel-2026-10-13.md):
  multiple CoreML processes slowed 1 MiB. Single-model B16 improved the matched
  1 MiB model test by 16.47%. Actual 50 MiB took 271.51 / 261.53 s with B8 / B16
  and identical spans. Student quality and core acceptance still fail.
- [Direct CoreML, MPS, batches and token student selection](apple-backends-2026-10-13.md).
- [Four new speed experiments](speed-techniques-2026-10-13.md): graph/QKV/FP16 had
  no useful speed gain; Nym lost gold coverage.
- [Progress/cache and isolated ORT 1.31.0](ort-runtime-2026-10-12.md): verified
  experiment, without switching production runtime.
- [Additional model/runtime quality gates](runtime-survey-2026-10-12.md): Q4 and
  GLiNER-X-small rejected; FP32 remains default.
- [Trained MiniLM student](student-minilm-2026-10-13.md): 4.02 s model-only varied
  1 MiB, but nine coverage losses/eight benign core additions and failed long core
  processing. The full 10-second anonymization goal remains unmet.

## Original measurement environment

2026-10-12: Apple M5, 10 CPU cores, 32 GiB RAM, arm64, macOS 27.0.1 (26A434),
Rust 1.99.0 Homebrew, Xcode at /Applications/Xcode.app. GLiNER multi-v2.1 FP32,
pinned SHA-256, threshold 0.4 and all eight labels unchanged. The user's app was
not restarted and no user documents, clipboard or mappings were used.

## Method and reproduction

`scripts/benchmark.py` runs each sample in a separate process. It measures complete
asset verification, ORT session creation, window planning, tokenization, tensor
preparation, inference, decoding, core replacement and restoration. `os.wait4`
records peak process RSS including load and cancellation probes. Diagnostics
contain sizes/counts/times only.

```sh
python3 scripts/benchmark.py --output /private/tmp/copy-ai-reproduction
python3 scripts/benchmark.py --varied --output /private/tmp/copy-ai-varied
cargo run -p copy-for-ai-ner --release --example benchmark --offline -- 102400 256 1 8 fp32 varied
```

Defaults: debug/release, 1/10/100 KiB, 256 KiB and 1 MiB; 256 tokens, batch one,
eight threads. Install the model first with install-model.py or point
COPY_FOR_AI_MODEL_DIR at verified local assets. The load line separates checksums
and loading. Run zero is first inference after load; run one is warm. Exact window
cache is rebuilt per call, not shared across documents in this original experiment.
Varied 1 MiB has one complete sample after a short warmup. N=1/2 is not p50/p95 or
production UI latency. Cold means new process, not empty filesystem cache.

[before-source.tar.gz](before-source.tar.gz) retains baseline source and instrumented
harness:

```sh
mkdir -p /private/tmp/copy-ai-baseline
tar -xzf docs/benchmarks/before-source.tar.gz -C /private/tmp/copy-ai-baseline
cd /private/tmp/copy-ai-baseline
export COPY_FOR_AI_MODEL_DIR=/path/to/copy-for-ai/models/gliner-multi
cargo run -p copy-for-ai-ner --example baseline --offline -- 102400
cargo run -p copy-for-ai-ner --release --example baseline --offline -- 102400
```

Baseline differs only in measurement hooks and portable model paths. Corrected
baseline harness reports restoration failure properly: authoritative 256 KiB files
are `before-fixed-*-repeated-262144.txt`. Older `before-*-262144.txt` measured
detection but misreported rejected restoration.

## Identical repeated corpus

Warm detection (run one), milliseconds; synthetic English paragraph with Czech tail:

| Input | Before debug | After debug | Before release | After release | After windows / inferred / reused |
| --- | ---: | ---: | ---: | ---: | --- |
| 1 KiB | 195 | 466 | 192 | 317 | 1 / 1 / 0 |
| 10 KiB | 2283 | 1510 | 2032 | 1306 | 11 / 4 / 7 |
| 100 KiB | 24,103 | 1205 | 22,530 | 705 | 110 / 4 / 106 |
| 256 KiB | 71,625; restoration rejected | 1376 | 76,665; restoration rejected | 734 | 280 / 4 / 276 |
| 1 MiB | Input rejected | 1476 | Input rejected | 723 | 1121 / 4 / 1117 |

100 KiB release detection improved 31.96×, primarily through exact window reuse
within one document. Every window counts and spans shift to original offsets.
A changed byte prevents reuse. Complete output/restoration passed after the change.
Original 256 KiB fit input but marker expansion exceeded restoration's former
262,144-byte limit. Original 1 MiB failed InputTooLarge before inference. Rejection
never counts as partially successful processing. Short input regressed: 1 KiB
192 → 317 ms due to larger per-inference windows. There is no universal 32× gain.

## Cold startup

| 1 KiB, fresh process | SHA-256 | Model load | First detection | Sum |
| --- | ---: | ---: | ---: | ---: |
| Before debug | 38,110 ms | 1660 ms | 198 ms | 39,968 ms |
| After debug | 2979 ms | 2303 ms | 410 ms | 5692 ms |

Optimizing only sha2 in the dev profile preserves full verification: checksum
12.79× faster, debug startup 7.02×. Release was already optimized: checksum 2038 ms
before versus about 2.1–2.7 s after. No meaningful release cold-start gain shown.

## Varied text, no reuse

Distinct paragraph references make every window's bytes different. Do not mix
these results with repeated corpus figures.

| Input | Debug | Release |
| --- | --- | --- |
| 100 KiB | 21,265 / 23,450 ms | 32,019 / 19,620 ms |
| 256 KiB | Warm 61,505 ms, 281 inferences | No separate varied sample |
| 1 MiB | 325,366 ms | 308,388 ms |

The first release 100 KiB overlapped other synthetic CPU work; variation is not a
controlled percentile. Debug 1 MiB has one completed exact round trip; a redundant
second sample was manually interrupted in its isolated process (exit_code=-2),
not counted as partial success or failure of the completed sample.

Release 1 MiB: [raw](after-release-varied-1048576.txt).

| Phase | Time |
| --- | ---: |
| File verification / loading before operation | 2096 / 757 ms |
| Window creation | 11 ms |
| Tokenization | 32 ms |
| Tensor preparation | 30 ms |
| Inference | 308,167 ms |
| Decoding | 130 ms |
| Total detection | 308,388 ms |
| Core replacement / exact restoration | 30 / 3 ms |

All 1143 windows inferred, reuse zero; 26,471 model spans; 3400 bytes/s. Byte-for-byte
restoration passed. CLI independently processed 1,048,576 bytes to 1,401,477 output
bytes, exit zero, with no expected synthetic name left (`cli-mebibyte.json`). The
model remains slow on large varied input; reliability is not a promise of fast
interactive 1 MiB inference.

## Batching, threads, GPU and INT8

Warm varied 10 KiB CPU tuning (`tune-*.txt`):

| Tokens / batch / threads | ms |
| --- | ---: |
| 128 / 1 / 4 | 2891 |
| 256 / 1 / 1 | 6501 |
| 256 / 1 / 4 | 2122 |
| 256 / 1 / 8 | 1915 |
| 256 / 2 / 4 | 2275 |
| 480 / 1 / 4 | 2178 |
| 480 / 2 / 4 | 2666 |

256/1/8 was best in this sample. Batch two is bounded and has a real model regression
test but did not help here. Production uses `available_parallelism().min(8)`,
batch one, without multiplying model copies.

CoreML was available but slower: legacy NN warm 10 KiB 6303 ms; MLProgram explicit
CPUAndGPU 6140 ms versus about 2 s CPU. Profile: 165 CoreML partitions, 1340 CPU
kernel nodes; over 22 windows, summed CPU kernels 948,167 µs and CoreML 11,262,077 µs.
This establishes partition overhead, not direct GPU utilization.

Static dimensions were tested. After fixing text_lengths, CPU produced 265 spans
and restored exactly; GPU reproducibly aborted (SIGABRT 134), MPSGraph mps.matmul,
contraction dimensions 2 versus 1024. Static export is not production. The corrected
reproducer is [gpu-experiment.tar.gz](gpu-experiment.tar.gz); main source retains
only an opt-in CoreML probe:

```sh
cargo run -p copy-for-ai-ner --release --features coreml --example benchmark --offline -- 10240 256 1 8 fp32 varied mlprogram profile
```

CoreML compilation required execution outside the restricted sandbox. Other apps
and System Settings were not controlled. Default app does not enable CoreML.
Outer window parallelism cannot guarantee GPU gains; graph support, transfers,
shapes and batch cost matter.

Optional INT8 from the same pinned revision: 349,120,924 bytes, SHA-256
`995058c82c5f570601dd8a0ba74ee60a392f764268bc5f628455e44dd3b476ec`.
Across 40 synthetic cases it lost 6340 spans versus FP32 (`quality-comparison.txt`).
The golden_missing=33 sum includes both variants, not INT8 alone. This is not
precision/recall. INT8 does not replace FP32. Reproduce with install-model.py
--int8 followed by the release quality_compare example.

## Memory, cancellation and UI

Varied release 1 MiB peak RSS: 3,331,555,328 bytes (3.10 GiB). Highest recorded
after RSS: 3,569,729,536 bytes (3.32 GiB, debug repeated 1 MiB). Process RSS is not
a guaranteed ORT hard limit.

Cancellation requested after 250 ms returned Cancelled after 304 ms on a subsequent
varied release operation. CPU samples typically 250–477 ms. Checksums check
cancellation after each 1 MiB read; native synchronous ORT load/run cannot stop
mid-call. 304 ms is not a universal guarantee. Discard cancelled results without
changing live mappings or pasting.

Workers receive bounded private snapshots and prepare full output off the UI.
Acceptance checks generation, input, workflow revision and target; paste rechecks
clipboard/modifiers/permission. Publish mappings atomically only on success.
Rejected proposals zeroize owned text.

A headless debug GPUI 1 MiB test calls real finish_detection: queue handoff takes
microseconds; editor publication about 0.46–0.57 s. That is remaining synchronous
editor cost, not inference. It uses a synthetic validated span; actual inference
is verified separately. It does not prove native frame rate, physical interaction
or system paste. Native executable built; running user's app was not replaced.

## Explicit limits

- Anonymization: 1,048,576 UTF-8 bytes; reject larger unmarked text before model.
- Marked input and output/restoration: 33,554,432 bytes; prevalidate expansion.
- Vault: 16 sessions, 4096 total mappings, 8,388,608 bytes of originals.
- Model: 256 subwords including labels/specials; overlap up to 24 units and one
  third of budget; batch one, at most eight CPU threads.
- Token cache: 4,194,304 subword IDs. Exact-window cache: 64 entries, 4096 spans
  each. Total model spans: 1,048,576.
- Core: 2,097,152 candidates and 4096 distinct unknown markers.
- Detection deadline: 15 minutes. Limits/errors reject the whole result, never
  truncate silently or fall back to regex-only processing.

Complete byte coverage does not imply complete entity detection. Six real FP32
regressions passed, including seven languages, window seams and the 1 MiB tail.
Full precision/recall remains open. Context-window changes can alter detections
even with identical weights, labels and threshold.

## Language and historical evidence

Current source and documentation use English. Synthetic multilingual data retain
their test languages. Raw outputs, compressed source snapshots, manifests and
historical screenshots remain byte-exact records of the measured versions;
they are not rewritten or relabelled as current English UI evidence.
