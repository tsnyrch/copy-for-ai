# Anonymization of 1 MiB within 10 seconds

Status: MiniLM and window parallelism tested; a 50 MiB model test completed, but
quality and the actual 1 MiB core workflow still fail acceptance. Date: 2026-10-13.
[Research, four experiments and evidence](../benchmarks/speed-techniques-2026-10-13.md).

## Selected direction: trained 12-layer token student

The [Apple screening](../benchmarks/apple-backends-2026-10-13.md) reached
31.10 ms/window in its fastest lossless variant. A public multilingual MiniLM
12 × 384 has now been downloaded, hash-verified, fine-tuned and converted to FP32
CoreML with a BIO11 head for all five categories.
[Results](../benchmarks/student-minilm-2026-10-13.md).

- **Model throughput:** actual trained checkpoint, varied 1 MiB, 211,003 tokens /
  951 valid windows, reuse zero, warm median **4.02 s** including tokenization and
  decoding. Excludes core, load, compilation and native workflow. Keep 12 layers;
  reducing to six is currently unnecessary.
- **First training:** 200 steps / 55.29 s, FP32 MPS. After whitespace decoder repair
  using the same checkpoint: synthetic template DEV 840 TP / 0 FP / 0 FN, exactly
  252 gold documents; 28 review cases remain in the denominator. No teacher loss yet.
- **Frozen acceptance:** all 93 original documents / 421 gold occurrences through
  real Vault/Session, 93 exact restorations. Student covered 409 versus baseline
  385, but lost **nine previously covered LOCATION occurrences** and added **eight
  benign spans in final core**. Acceptance failed.
- **Long core workload:** varied 1 MiB produced 8820 unique model mappings against
  the vault limit of 4096 and core failed. The exact VaultFull variant was not
  confirmed. Of 8816 unique Person values, 8814 contain digits. Improve negative
  data; raising capacity alone would not establish anonymization quality.

[Parallelism and batching](../benchmarks/window-parallel-2026-10-13.md): one/two/four
CoreML B8 processes gave 1 MiB medians 4.958 / 5.917 / 6.792 s. More processes did
not help. Single-model B16 reduced a matched 1 MiB result from 4.901 to 4.094 s
with identical spans. B32 was slower.

Both actual 50 MiB single runs completed 48,028 valid windows / 10,662,245 tokens,
without reuse. B8 took 271.509 s and B16 261.526 s, both yielding 996,940 spans.
The 3.68% difference is model-only and proves no quality improvement. Current core
rejects exactly 50 MiB with typed `InputTooLarge` before NER. Limits remain intact.
Keep single-model B16 as a throughput candidate for 1 MiB; quality and completed
core workflow remain the substantial gates.

Next candidate: task-specific GLiNER distillation into this 12-layer head using
**new**, varied synthetic data, more LOCATION contexts in unspaced scripts and
hard negative references/codes/numbers. Split template families between TRAIN/DEV,
retain ADDRESS/EVENT and record byte boundaries that cannot be represented exactly.
Do not train or tune on the frozen 93 documents. Require zero new coverage losses,
zero benign regressions and actual 1 MiB core completion within 10 seconds before
adoption. Production model/backend stays unchanged.

## System One: Laya, CLM and Jev

Primary sources examined on 2026-10-13. [Architecture/availability evidence](../benchmarks/system-one-screening-2026-10-13.json).
These score predefined options rather than generating free text. The current
MiniLM BIO student already classifies all tokens in a window in one pass; an
advantage over autoregressive LLMs does not establish an advantage over this NER.
One pass applies to bounded context, not an entire 1 MiB or 50 MiB document.

| Model | Local availability | Output contract | Copy for AI decision |
| --- | --- | --- | --- |
| [Laya](https://github.com/NandhaKishorM/laya) | Public Apache-2.0 weights, multilingual mmBERT-base plus decision head, 322M parameters; 1024 default / up to 8192 configured tokens. | Option logits at marker positions; each question is a separate encoder row even within a batch. | Possible encoder for a newly trained token/span head; published decision head is not NER. |
| [CLM](https://huggingface.co/Contrastive-LM/CLM-v0.1-8B) | Public local model requiring frozen Qwen3-8B; the ~75.6 MB file contains only projection heads. | Last-token pooling and contrastive scores for supplied candidates; context/question embedded per question. | Higher encoder cost with full-span discovery still unsolved; no measured PII quality or speed gain over the student. |
| [Jev](https://docs.typesafe.ai/models) | Hosted TypeSafe API documented; no public local weights found in examined primary sources. | Choice/Score/Noul; complete network and internal forward count are unverified publicly. | Does not meet local-backend requirements. Extraction cookbook chooses among previously discovered values, not all unknown sensitive spans. |

[Laya structured API](https://nandhakishorm.github.io/laya/structured/) supports
enums, booleans and bounded integer scores; it rejects free strings, arrays and
nested objects. It cannot return arbitrary `{start, end, entity_type}` lists under
that contract. [Implementation](https://github.com/NandhaKishorM/laya/blob/main/laya/common.py#L524-L605)
scores options rather than every document token. Querying each token/span repeats
encoder computation and is not equivalent to a BIO head.

Retain **Laya multilingual/mmBERT with a new NER head** as a research possibility,
without language/JSON routing. It requires training and the same frozen acceptance,
not a switch to the existing decision API. Replacing the encoder alone does not
fix MiniLM quality. None of these three models was newly timed on M5/1 MiB/50 MiB;
published decision latency on other hardware is not our anonymization result.
Weights were not downloaded or hash-verified. Production backend and limits remain.

## Baseline decision

Keep FP32 and the original production runtime. The previously verified isolated
ORT 1.31 step still took about 93 seconds for varied 1 MiB, missing the 10-second
goal. Position precomputation, QKV fusion and FP16 brought no useful speedup.
Smaller Nym introduced quality losses and lacks EVENT. Do not repeat these trials
without a concrete new hypothesis.

## Candidate 1 — Apple graph, screening complete

Direct encoder export, original packed CPU head and GPU/CPU batches were examined
in the [Apple report](../benchmarks/apple-backends-2026-10-13.md). FP32 preserved
small tensor-suite detections but failed speed acceptance. Full and selective
FP16 changed detections. No production integration is planned from this result.
The original hypothesis below is not proof of complete-model export including head.

Export the original PyTorch model directly to CoreML using bounded buckets and
resident weights. Preserve tokens, masks, labels, span head, UTF-8 offsets and
decoder. First verify export and one complete matching window; profile dominant
MatMul/attention placement and CPU/GPU transitions. Prior split ONNX/CoreML is not
proof of this export. Reject unsupported dominant graphs before costly document
runs. After numerical/gold acceptance, use the same varied 100 KiB in alternating
pairs; run 1 MiB only after a measured gain. Padding must preserve words/tail.
Batch/bucket dispatch belongs only in `crates/ner`.

## Candidate 2 — fixed-category student

Use a smaller multilingual token/span encoder for PERSON, ORG, LOCATION, ADDRESS,
EVENT and any other required existing categories. Avoid a repeated label prompt.
Evaluate depth pruning/distillation independently from quantization to identify
loss causes.

Train on synthetic data only. Teacher spans/logits may supervise, but known
teacher mistakes cannot be the only gold. Independent Czech/multilingual evaluation
needs inflection, generic formats, address parts, events and negative text. Never
tune checkpoint/threshold on current benchmark gold. Nym demonstrates architecture
and known weaknesses, not an accepted all-category checkpoint. No name dictionaries,
JSON routing or language-specific identity regexes.

## Candidate 3 — sparse/streaming context

Explore sparse attention or causal span students with bounded right-context
recomputation. Measure first-document and append latency separately. Require a
verifiable multilingual checkpoint, complete categories and UTF-8 seam contract
before timing. KV caching bidirectional GLiNER is not an exact local repair.
English bi-encoders and 1024-label marketing results do not meet this gate.

## Acceptance gates

1. Load and complete one valid pass; no silent model/backend fallback.
2. Exact optimization needs logit/span parity; another model needs independent
   gold and negatives. No new loss of known coverage.
3. Same varied 100 KiB, reuse zero, separate startup/warm timing, no concurrent
   owned inference/builds, and measured RSS.
4. Complete varied 1 MiB within 10 seconds, covered tail, byte-for-byte restoration,
   bounded memory and cancellation before considering integration.
5. NerService identity/epoch/cache and worker tests; separately native progress,
   cancellation and actual permission-enabled system paste.

Exact LRU reuse is an optional optimization of repeated documents, never a
first-varied-clipboard speed claim. Direction, mappings and protected paste stay
in the existing core/workflow.
