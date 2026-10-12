# Window parallelism and an actual 50 MiB test

Date: 2026-10-13. Isolated experiments complete, without changing production model
or application limits.

Windows can run concurrently and merge in order, but two/four CoreML processes
were slower than one on 1 MiB. Single-model batch 16 improved a matched 1 MiB test
by **16.47%**. Actual 50 MiB improved only **3.68%**, reaching **261.53 s**.
These are model-pipeline timings, not completed app anonymization.

## Measurement scope

Same Apple M5 / 32 GiB and trained checkpoint as the [MiniLM experiment](student-minilm-2026-10-13.md).
Multilingual MiniLM L12 H384: BERT encoder, XLM-R tokenizer, BIO11 for person,
organization, location, address, event. Supervised training; no GLiNER teacher loss.

- Public revision: `6e8c1ec6b4ec4e3fc6eb7d2cd834fcd582b61daf`.
- Checkpoint SHA-256: `918636493f4cd7428c672c44bb3ea472033f5243cf17e3223f226c3b4773c5eb`.
- FP32 CoreML CPU_AND_GPU, coremltools 9.0, CPython 3.13.16 with GIL; batches
  8/16/32, length 256, overlap 32. No backend fallback.
- Retokenize complete synthetic UTF-8 input per run. Include tokenization, byte
  offsets, inference, validation, merging and decoding. Exclude text generation,
  loading/compilation, warmup, Vault/Session and native UI.
- Each worker owns a persistent model and calls it serially. Processes use spawn,
  not fork of a live Metal context. In-flight batches are bounded by worker count.
- Predict every valid window; pad the last batch to static size. One owner per
  token. Merge by window index into original byte spans. Reuse zero.

## 1 MiB: processes versus batching

Three warm runs per variant. Memory is sampled summed host RSS of parent/workers,
including initialization.

| Mode | Workers | Batch | Runs (s) | Median (s) | Sampled peak RSS (GiB) |
| --- | ---: | ---: | --- | ---: | ---: |
| Process | 1 | 8 | 4.785 / 5.166 / 4.958 | **4.958** | 2.087 |
| Process | 2 | 8 | 8.105 / 5.917 / 5.914 | **5.917** | 2.594 |
| Process | 4 | 8 | 6.918 / 6.391 / 6.792 | **6.792** | 5.053 |
| Process | 1 | 16 | 4.380 / 4.354 / 4.460 | **4.380** | 1.907 |
| Process | 1 | 32 | 6.313 / 6.809 / 7.376 | **6.809** | 1.952 |
| One thread / model | 1 | 8 | 4.901 / 4.742 / 4.966 | **4.901** | 1.492 |
| One thread / model | 1 | 16 | 4.123 / 4.094 / 3.992 | **4.094** | 1.504 |

B8 processes were 19.36% slower with two and 37.01% slower with four workers.
Measured host predict-call overlap was 1/2/4, not proof of concurrent GPU kernels.
The cause was not independently isolated. B16 helped both runners; B32 was slower.
Historical 4.02 s from a different direct runner is not a control for this IPC/
instrumented runner. Percentages use matched runners only.

All seven variants returned **20,456 spans**, **211,003 tokens**, **951 valid windows**.
B8 computed 952 window rows, B16/B32 960 including padding. Identical ordered
byte-span/kind digest:
`6fcc5287370a931588fa28f6d9add36a12c6908a3a34b50355d788ce444587b1`.

## 50 MiB: complete text inference

Exactly **52,428,800 bytes**, **10,662,245 tokens**, **48,028 valid windows**. Both
computed 48,032 padded rows, without reuse. Each token owned once; the final token
ends at byte 52,428,800.

Both use one thread/model; multiprocess experiments were limited to 1 MiB. Each
50 MiB variant completed **one warm run**, not a variance/p95 series.

| Batch | Tokenization/preparation (s) | Inference/merge (s) | Full model pipeline (s) | Sampled peak RSS (GiB) | Process peak RSS (GiB) |
| --- | ---: | ---: | ---: | ---: | ---: |
| 8 | 16.351 | 254.838 | **271.509** | 5.231 | 6.949 |
| 16 | 20.843 | 240.412 | **261.526** | 2.276 | 5.031 |

Decoding: 0.320 / 0.270 s. Prediction calls: 6004 / 3002. Total saving 9.984 s;
different preparation times and one run limit conclusions. Both produced exactly
**996,940 spans**, digest:
`39ca5b1250eef9a9ec575b64a4e36bab1a5ece0869bf2a1e06597f871e0c4021`.
Shared synthetic input SHA-256:
`1684417aee48dfbdd3313e4b105e3d4b4dd817a7d2d3a961736b8f8f81b41a9a`.

RSS guard: 16 GiB. Sampling is not a hard allocation limit and can miss brief peaks.
The second RSS column uses process maximum; these single-process runs avoid the
nonconcurrent-multiprocess-peak issue. Neither is complete GPU/unified-memory
footprint. RSS differences do not prove B16 memory savings.

## Concurrency research and runner limits

[Apple guidance](https://developer.apple.com/videos/play/wwdc2023/10049/) says
synchronous prediction on the same model is unsafe concurrently; newer async
Swift APIs support concurrency. This experiment therefore does not share one
synchronous instance across workers.

[CoreML Python 9.0 binding](https://raw.githubusercontent.com/apple/coremltools/9.0/coremlpython/CoreMLPython.mm)
does not release GIL during synchronous prediction. Python threads cannot establish
concurrent native prediction; thread mode here measures one model without process
IPC. See [pybind11 GIL](https://pybind11.readthedocs.io/en/stable/advanced/misc.html#global-interpreter-lock-gil).
[Torch MPS 2.7.0](https://github.com/pytorch/pytorch/blob/v2.7.0/aten/src/ATen/mps/MPSStream.mm)
uses a default singleton stream and serial dispatch queue; Python threads do not
create CUDA-style independent MPS streams. ORT CPU could use concurrent sessions
with budgeted intra-op pools/[spinning](https://onnxruntime.ai/docs/performance/tune-performance/threading.html).
Those ORT variants and native Swift async were not benchmarked.

The first single-model B16 run failed because the C++ bridge received readonly
NumPy input. Runtime now copies only readonly arrays into owned writable buffers
and records time/bytes, preserving values. Corrected runs use separate files;
original failure remains in evidence.

Process runner has finite future waits and bounded terminate/join/kill cleanup
for its own workers. Smoke checks used synthetic subprocess tasks, not a native
CoreML hang. A running synchronous native thread call cannot safely be interrupted
by this runner.

## Application boundary and decision

Production anonymization limit **1 MiB**; output/restoration input **32 MiB**.
A separate Rust probe using current core returned typed **Error::InputTooLarge**
from Vault.direction on exactly 50 MiB UTF-8. NER was neither called nor a dependency.
Source audit shows desktop rejects before loading the model; no new native UI
measurement. Limits were not raised.

Cross-batch span parity preserves this checkpoint's output, not its
[failed quality acceptance](student-minilm-2026-10-13.md): nine previously covered
LOCATION occurrences missing, eight new benign core spans, and previous varied
1 MiB failure in Vault/Session. Mapping count is the likely cause, without typed
confirmation. Complete production 1 MiB / 10 s remains unmet. The 50 MiB experiment
proves neither core anonymization/restoration nor permissions/system paste.

Choose **one model, batch 16** as the 1 MiB performance candidate. Next fix student
quality with separate varied training data, hard negatives and task-specific
GLiNER distillation, then repeat frozen acceptance and actual core workflow.
More CoreML processes do not justify integration. 50 MiB requires a separate
capacity/memory design, not a product-limit change.

## Evidence and reproduction

[Evidence directory](window-parallel-2026-10-13/) contains completed JSON/logs,
all-variant comparisons, B16/B32 exports, prior 1 MiB reference, tokenizer/ownership
parity, cleanup smoke, core source audit, Rust limit probe and four script snapshots.
[Manifest](window-parallel-2026-10-13/manifest.json) records sizes/SHA-256. Large
packages/inputs remained in /private/tmp.

Four Python AST checks passed. Independent read-only review found no result-changing
issue. Parity helper checked UTF-8, ownership, B8/16/32 padding and digest, not model
quality. MCP graph returned Transport closed, so known sources were read directly.
Production Rust inference unchanged in this experiment.

```sh
PYTHONDONTWRITEBYTECODE=1 PYTORCH_ENABLE_MPS_FALLBACK=0 \
  /private/tmp/copy-ai-apple-2026-10-13/venv/bin/python -B \
  scripts/benchmark-student-parallel.py \
  --model-dir /private/tmp/copy-ai-student-2026-10-13/model \
  --checkpoint /private/tmp/copy-ai-student-2026-10-13/train-12-200 \
  --package /private/tmp/copy-ai-window-parallel-2026-10-13/minilm-trained-b16.mlpackage \
  --output /private/tmp/copy-ai-window-parallel-2026-10-13/reproduction-b16-50m.json \
  --sizes 52428800 --workers 1 --runs 1 --executor thread --batch 16
```

For 1 MiB process comparison use B8 package
/private/tmp/copy-ai-student-2026-10-13/minilm-12-trained-b8.mlpackage and
`--sizes 1048576 --workers 1 2 4 --runs 3 --executor process --batch 8`.
Use a new output path to preserve original evidence.
