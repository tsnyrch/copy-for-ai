# Apple backends and next direction — 2026-10-13

**Next main direction: a smaller multilingual token student distilled from GLiNER.**
Direct Apple export is technically feasible, but verified variants do not meet
1 MiB / 10 s. At this screening stage the student was not yet trained/accepted;
see the [later student report](student-minilm-2026-10-13.md). Production model,
backend and app stay unchanged.

## What was actually verified

Apple M5, 10 CPU cores, 32 GiB, macOS 27.0.1. Isolated Python 3.13, Torch 2.7.0,
GLiNER 0.2.21, Transformers 4.51.3, CoreMLTools 9.0, Python ORT **1.24.4**.
Synthetic inputs only, without clipboard, user documents or UI.

Weights reconstructed from the verified original ONNX, not a different checkpoint:
219 initializers → 224 parameters, no missing/unused weights. Preserve weight
aliases, LSTM gate order, vocabulary 250105 and marker 250103. The sole nonpersisted
position buffer is created exactly as in the original Transformers constructor.
Model SHA-256: `57af7f3d8d72a3ca4e5619c247344b4b5ee362a5ebcf307c0634d67b65e721c0`.

Fourteen tensor cases: CS/EN/DE/FR/RU/JA/ZH, budgets 96/256, all eight original
labels, threshold 0.4. Compare logits and valid start/end/label triples. **Not
independent gold recall, final core spans or a complete document test.**

| Pilot batch one | Median of seven language warm medians, 256 tokens | Lost / new threshold spans, all 14 cases |
| --- | ---: | ---: |
| Torch CPU FP32 | 112.76 ms | 0 / 0 |
| Model moved to MPS, fallback off | 105.09 ms | 0 / 0 |
| MPS encoder + original Torch CPU head | 59.33 ms | 0 / 0 |
| Direct CoreML FP32 encoder + Torch CPU head | 36.44 ms | 0 / 0 |
| Direct CoreML FP32 encoder + original ONNX CPU head | 40.12 ms | 0 / 0 |
| CoreML FP16 / ALL + Torch CPU head | 39.16 ms | 16 / 2 |

Pilot timings alternate candidate/reference ORT 1.24.4 with default spinning
threadpools. They are indicative, not document p50, and cannot be ratio-extrapolated
to historical ORT 1.31 / 1 MiB. A separate nonspinning control is in raw evidence.
B1 peak RSS 3.92–4.99 GiB includes reference runtime and reconstructed Torch model,
not candidate alone.

## Direct export boundary

GPU block contains all token_rep_layer: mDeBERTa and projection 768 → 512. CPU
retains prompt/word extraction, the original packed bidirectional LSTM and span
head. Do not substitute an ordinary padded LSTM.

Static MLProgram export required two narrow fixes:

1. CoreML sqrt must promote integer input to FP32 as Torch does.
2. Scripted build_rpos has branches of different ranks. For fixed self-attention
   length, trace validates query/key lengths against the bucket and uses the
   original relative-position tensor. Token/mask values are not specialized.

Restore the original helper after tracing. Two failed conversions and successful
third are archived. CPU tail extraction retains 323 original ONNX nodes including
LSTM; two encoder-named nodes are constants, not residual encoder compute.

FP32 CoreML plan prefers GPU for known compute operations. FP16/ALL plan estimates
63% ANE and 37% GPU cost. **Expected plan, not runtime hardware trace.** ALL alone
does not establish ANE execution.

Actual FP16 graph inspection ruled out vocabulary-index overflow: embedding gather
stays INT32. Two narrowed relative-index arrays retain values 64–448; binary mask
conversions are exact. The source of numerical drift was not isolated.

## Real batches of eight windows

Eight computed language rows per batch, including one recomputed Czech row;
16 valid windows across budgets 96/256. Encoder fixed bucket 8 × 256; CPU tail
also batched. Keep original lengths/span validity, adding right padding only.
Trim/compare every output against independent original B1 ONNX. No cached compute
or unaccounted padded rows.

Both ORT pools disable spinning, intra eight/inter one. Reference runs before
warm cycles. Five cycles measure wall time including preparation, synchronous
CoreML, CPU head and copying; no concurrent owned inference.

| Candidate | Wall / valid window | Encoder / window | CPU head / window | Lost / new spans |
| --- | ---: | ---: | ---: | ---: |
| FP32 CPU/GPU | 31.10 ms | 21.50 ms | 9.52 ms | 0 / 0 |
| FP16 linear only, rest FP32 | 21.64 ms | 12.23 ms | 9.31 ms | 3 / 0 |

This is mixed-tensor-suite throughput, **not 100 KiB/1 MiB latency**. Component
summaries use separate medians. B8 RSS was not measured. Load/compile is separate;
FP32 B8 load here was 5145 ms.

Selective export has 73 FP16 linear operations but 48 FP32 attention matmul,
25 FP32 layer_norm and 12 FP32 softmax. Integer narrowing is disabled. Threshold
spans still change; max logit delta 1.685, versus 4.064 for full FP16. Lost threshold
spans are **not counts of newly uncovered gold data**; core/gold gate was not run.
Neither precision-changing variant is accepted.

Historical varied 1 MiB has 1143 windows. Ten seconds under that plan permits only
8.75 ms/window for the **whole operation**. No current Apple result meets it;
long document runs were not attempted after failed smaller gates. Batching FP32
encoder alone did not fundamentally reduce cost.

## Selected next candidate

A token student removes joint label prefix, all-width span combinations and
bidirectional LSTM. Retain PERSON, ORGANIZATION, LOCATION, ADDRESS, EVENT, mapping
eight teacher labels using the existing domain contract. Core rules/validators,
vault and restoration remain. Model failure still blocks anonymization.

Baseline control: multilingual MiniLM **12 × 384**. More aggressive candidate:
**6 × 384**, initialized from that encoder and further distilled. Half depth is
not a ready public NER checkpoint. Both require a trained five-category BIO head
and calibration.

[Pinned model card](https://huggingface.co/microsoft/Multilingual-MiniLM-L12-H384/blob/6e8c1ec6b4ec4e3fc6eb7d2cd834fcd582b61daf/README.md)
states 21M transformer + 96M embedding parameters, BERT encoder and
**XLMRobertaTokenizer**, explicitly warning against AutoTokenizer. XNLI/MLQA
results are not NER/PII evidence. This stage downloaded only config/model card,
not new weights. [Distillation plan and gates](../plan/anonymization-speed.md).

## Reproduction and evidence

[Raw JSON, logs, pinned configs, source snapshot and manifest](apple-2026-10-13/manifest.json).
Large packages/models stayed under /private/tmp/copy-ai-apple-2026-10-13; the archive
records fingerprints. Python source is in scripts/.

```sh
PYTHONDONTWRITEBYTECODE=1 /private/tmp/copy-ai-apple-2026-10-13/venv/bin/python \
  scripts/prepare-apple-coreml-probe.py --model models/gliner-multi/model.onnx \
  --configs /private/tmp/copy-ai-apple-2026-10-13/configs \
  --output /private/tmp/new-encoder.mlpackage --report /private/tmp/new-export.json \
  --batch-size 8 --sequence-length 256 --precision fp32

PYTHONDONTWRITEBYTECODE=1 /private/tmp/copy-ai-apple-2026-10-13/venv/bin/python \
  scripts/prepare-apple-head-probe.py --model models/gliner-multi/model.onnx \
  --output /private/tmp/new-head --dynamic-batch

PYTHONDONTWRITEBYTECODE=1 PYTORCH_ENABLE_MPS_FALLBACK=0 \
  /private/tmp/copy-ai-apple-2026-10-13/venv/bin/python \
  scripts/benchmark-apple-batch-probe.py --model models/gliner-multi/model.onnx \
  --head /private/tmp/new-head/model.onnx --coreml-package /private/tmp/new-encoder.mlpackage \
  --output /private/tmp/new-gate.json --batch-size 8 --budgets 96 256 --runs 5
```

GPU/CoreML required execution outside the restricted sandbox. Script syntax and
actual load/export/inference checks passed the stated gates. This experiment did
not verify Rust, desktop, native UI, permissions or system paste.
