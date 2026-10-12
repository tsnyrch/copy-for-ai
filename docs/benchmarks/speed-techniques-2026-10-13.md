# New speed techniques — 2026-10-13

Status: first four new candidates completed. User goal: **1 MiB within 10 seconds**
while preserving sensitive-data coverage. None passed joint speed/quality gates.
Production engine and running app were not switched.

## Results and decisions

Apple M5, 10 physical CPU cores, 32 GiB RAM, macOS 27.0.1, Rust 1.99.0; eight
threads, sequential runs. GLiNER multi-v2.1, eight labels, threshold 0.4, 256
subwords, batch one. Rust probes use isolated ORT 1.31; its earlier improvement
over 1.20 is [separate evidence](ort-runtime-2026-10-12.md).

| Candidate | Varied 100 KiB | Quality versus baseline | Decision |
| --- | ---: | --- | --- |
| Reference GLiNER FP32 / ORT 1.31 | 8800 ms | 36 uncovered of 421 gold occurrences. | Reference, not complete anonymization. |
| A: auxiliary fixed shapes and precomputed relative-position buckets | 9741 ms | Identical final spans/kinds for all 93 documents. | Rejected: 10.7% slower in this screening. |
| B: A plus fused Q/K/V projections | 10,523 ms | Identical final spans/kinds for all 93 documents. | Rejected: 19.6% slower, about 54 MiB extra weights. |
| C: FP16, preserving IO types | 15,918 ms | No new gold loss, but **two new benign core spans**. | Rejected: quality regression and 80.9% slowdown. |
| Nym pruned/distilled mmBERT FP32 / ORT 1.24.4 | 5049 ms | 46 uncovered gold, **16 new losses**, six gains; no EVENT. | Rejected as direct replacement. |

Reference/A/B/C use run one after a full first pass; Nym one pass after short
warmup. Corpus bytes match across variants (SHA-256 in manifest). GLiNER inferred
111 windows, Nym 52; **reuse=0**. Nym uses 480 content tokens plus BOS/EOS, overlap
64, threshold 0.5 and a different tokenizer/runtime. Time differences cannot be
attributed to architecture alone. One screening is not p50/p95 or a controlled
estimate of small regressions. Failed gates stopped costly varied 1 MiB trials.
Historical ORT 1.31 1 MiB remains 93,356 ms; **10 seconds remains unmet**.

### Verified boundaries

- A: 4958 → 4912 graph nodes. Integer table evaluates the original branch, not an
  approximate logarithm; parity passed 11 lengths from 1–512. Batch one, maximum
  length 512 explicitly enforced.
- B: MatMul 153 → 129; 36 Q/K/V projections → 12 MatMul + Split. Q/K weights also
  remain in position branches, duplicating some memory.
- Python ORT 1.24.4: A/B bit-identical logits/spans at threshold 0.4 on all 14
  seven-language tensors. Indicative 4–5% small-suite gain did not transfer to
  Rust ORT 1.31.
- C: 551 MiB external weights; ONNX checker passed. ORT 1.24.4 optimizer failed
  initialization; **ORT 1.31 actually loaded and inferred**. NER differences:
  1417 missing / 28 added spans; core 76 / 45. One new NER and two new core spans
  occur on annotated negatives. Total differences are not false-negative rates.
- Gold: 92 short/positional documents plus complete 1 MiB multilingual-tail text,
  421 unique expected occurrences. All Rust variants performed real core
  replacement and byte-for-byte restoration.
- Nym tokenizes input once, returns UTF-8 byte spans, and keeps overlapping Unicode
  byte-fallback offset groups intact at seams. Same Rust core and exact restoration
  passed all 93 documents and varied 100 KiB.
- Nym lost Czech locations, a person in JSON-like text and an event. On annotated
  portions of three benign controls at four positions: zero false NER spans versus
  baseline 27; core zero versus 30. This motivates training, not compensation for
  lost PII.
- Nym processed repeated-fill 1 MiB with multilingual tail in 45,987 ms, all 485
  windows inferred. That proves coverage/restoration, not varied 1 MiB speed.
  100 KiB process peak RSS: 1,590,296,576 bytes.

Compare gold separately over NER spans and final core findings after propagation
and overlap selection. Coverage requires all non-whitespace expected characters
masked. Findings come from the same anonymize_with_entities used by a fresh Vault;
replacement counts and both complete restorations were checked. Gold counts happen
to match stages here, but ranges and benign findings differ. Negative gates compare
new span/kind pairs, not counts: C core has 30 → 30 benign findings yet adds two
new ones. Gold parity is not complete PII recall. No native UI, permission or paste
measurement.

### Evidence and reproduction

[Manifest/hashes](speed-2026-10-13/manifest.json),
[source manifest](speed-2026-10-13/source-manifest.json),
[exact Rust probe archive](speed-2026-10-13/rust-probe-source.tar.gz),
[A](speed-2026-10-13/A-quality.json), [B](speed-2026-10-13/B-quality.json),
[C](speed-2026-10-13/C-quality.json), [Nym](speed-2026-10-13/nym-quality-480.json),
[final C core gate](speed-2026-10-13/C-core-quality.json),
[final Nym core gate](speed-2026-10-13/nym-core-quality.json).
Directory includes compressed full range logs, timings, graph manifests and core
restoration evidence. Weights remained in isolated temporary storage.

```sh
python3.13 -m venv /private/tmp/copy-ai-speed/venv
/private/tmp/copy-ai-speed/venv/bin/pip install onnx==1.20.1 onnxruntime==1.24.4 tokenizers==0.22.2
python3 scripts/benchmark-speed.py prepare /private/tmp/copy-ai-speed/corpus
/private/tmp/copy-ai-speed/venv/bin/python scripts/prepare-graph-probe.py --help
/private/tmp/copy-ai-speed/venv/bin/python scripts/prepare-fp16-probe.py --help
/private/tmp/copy-ai-speed/venv/bin/python scripts/evaluate-graph-probe.py --help
python3 scripts/benchmark-graph.py --help
```

benchmark-graph.py verifies hashes, copies core/NER into a temporary workspace and
changes only that copy's experimental graph path. Original model verification
stays active. The temporary lockfile may contain only original package/source/
version combinations; build uses --locked --offline. speed_probe measures quality,
benchmark measures speed. Nym uses the three pinned files/hashes below:

```sh
/private/tmp/copy-ai-speed/venv/bin/python scripts/benchmark-speed.py nym --model-dir MODEL_DIR --corpus CORPUS_DIR --output NYM_LOG
python3 scripts/benchmark-speed.py summarize CORPUS_DIR BASELINE_LOG NYM_LOG QUALITY_JSON --stage ner
python3 scripts/benchmark-speed.py ranges NYM_LOG RANGE_DIR
target/release/examples/speed_probe validate CORPUS_DIR RANGE_DIR > CORE_LOG
python3 scripts/benchmark-speed.py summarize CORPUS_DIR BASELINE_CORE_LOG CORE_LOG CORE_JSON --stage core
```

Passed: cargo fmt --all -- --check; cargo clippy -p copy-for-ai-ner --all-targets
--locked --offline -- -D warnings; Python syntax and lightweight Unicode/BIO/offset
group checks. Desktop suites were not repeated for these isolated experiments.

## Follow-up research

1. **Complete Apple export with identical weights.** Direct PyTorch → CoreML
   bucket graph, precomputed positions, resident GPU/ANE weights. Profile dominant
   operations and expensive transitions before logit/gold gates. Prior split
   ONNX/CoreML is a different experiment; a new export alone proves no gain.
2. **Small multilingual student covering all fixed categories.** Pruned encoder
   and span/token head including EVENT/full ORG, distilled logits/boundaries,
   recall-weighted synthetic training. Nym demonstrates direction but cannot gain
   missing categories without training. Czech gold must be independent; do not
   blindly learn teacher errors.
3. **Sparse/causal student with bounded right context.** Potential sequence-cost
   reduction and KV cache, requiring multilingual checkpoint, seams and UTF-8
   offsets. Caching eight labels has lower priority.

Exact LRU reuse helps repeated/edited documents, not first varied 1 MiB. Confidence
routing cannot skip full-category inference: Nym cannot rule out EVENT it does not
model. Preserve complete generic UTF-8 input without silent regex fallback. Do not
attribute speedups to untested candidates.

## Primary-source research

Sources read 2026-10-13. Ranking is proposed experiment order for first varied
input, not measured latency. All inference stays local. Author model cards/timings
are not evidence on our M5/gold corpus.

| Rank | Technique | Result preservation | Smallest useful experiment |
| --- | --- | --- | --- |
| 1 | Same FP32, newer ORT, graph profiling, static buckets and fusion without GELU approximation. | Intended same function; FP32 operation order can still change threshold decisions. | Whole tensors/spans then 100 KiB against equally new runtime; count earlier ORT gains separately. |
| 2 | Fixed BIO pruned/distilled mmBERT in Nym. | Different weights/taxonomy, not GLiNER-equivalent or complete replacement. | Existing gold/negatives, UTF-8/seams, then 10/100 KiB with full input coverage. |
| 3 | Product-specific model distilled from current multilingual model. | Recall may change; retain EVENT and independent gold. | Smaller token/span head, recall by category/language/seam, not teacher agreement alone. |
| 4 | Sentence-aware windows and smaller adaptive overlap. | Changes context/results; byte coverage is not recall. | Same labels/threshold, gold around every seam; count saved tokens and individual new misses. |
| 5 | Bi-encoder with resident label embeddings. | Cache exact within a bi-encoder, not a conversion of current uni-encoder. | Multilingual gate, then eight labels cached/uncached rather than 1024-label marketing. |
| 6 | StreamingSpan causal KV cache and rolling context recomputation. | Different architecture/results/right context, not caching current BERT. | Append/full same checkpoint, seams/memory; first-document speed separate from append latency. |

### Same model: graph, shapes and Apple

[ORT transformer optimizer](https://onnxruntime.ai/docs/performance/transformers-optimization.html)
can apply offline fusions absent at load time. Inspect actual DeBERTa fusions;
a generic BERT optimizer is not automatically appropriate.
[Graph optimizations](https://onnxruntime.ai/docs/performance/graph-optimizations)
distinguish optimization from numerical approximation. Static buckets must retain
tokens, positions, masks and decoded ranges; padding does not permit truncation.

[CoreML EP](https://onnxruntime.ai/docs/execution-providers/CoreML-ExecutionProvider.html)
documents dynamic-shape costs and RequireStaticInputShapes,
SpecializationStrategy=FastPrediction, ProfileComputePlan. Further Apple work
requires evidence that dominant operations execute on the intended device, not
renaming the previous CPU/GPU split. Direct complete PyTorch export is distinct
but first needs DeBERTa/span operator support. [Apple compression guidance](https://apple.github.io/coremltools/docs-guides/source/opt-overview.html)
covers palettization/INT8; results depend on model, chip and compute unit.
Quantization always requires a fresh quality gate.

### Fixed BIO categories and Nym mmBERT

[Nym model card](https://huggingface.co/Wismut/nym-pii-multilingual-small): MIT ONNX,
16 layers, vocabulary pruning, distillation and recall-weighted training. Metadata
lists CS/EN/DE/FR/RU/JA/ZH among 22 languages. FP32 ~429 MB; int8/ compresses embeddings
and body weights but still computes FP32. edge-int8/ is another numerically distinct
candidate.

Taxonomy: 40 kinds / 81 BIO labels. PERSON combines GIVEN_NAME/SURNAME; ORG maps
only COMPANY_NAME; LOCATION combines CITY/COUNTRY/STATE; ADDRESS combines parts.
**No EVENT category.** Aggregate scores cannot justify replacing full scope.
[Training dataset](https://huggingface.co/datasets/Wismut/nym-pii-multilingual-data)
has Czech metadata/synthetic examples but no separate Czech result in the card.
Local gold is required, not assumptions from tags.

Pinned revision: `4348999cd3c2e20c49615e9af7c6bbb45b64cd85`. Sizes/LFS SHA-256 from
[HF metadata](https://huggingface.co/api/models/Wismut/nym-pii-multilingual-small?blobs=true);
small-config hashes computed after exact-revision download.

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| model.onnx | 429,218,752 | `60c2ae1a5992e43d4022a29dcc81cc45250bdb08fd2ee472cfb4d8800bbe87aa` |
| tokenizer.json | 12,389,891 | `c299144e68dfec1dc536204a7ae3712710c5c6cade9269a83f5042250d47d8de` |
| config.json | 5688 | `3f07065571e22bb73eba28ddb1fae4509c0cf703762e3bfa7a6ab2111cf7cb88` |
| tokenizer_config.json | 573 | `02055b886266b1a475bc324da83e273fe96e4974002f9d970f26e94aa73e5885` |
| int8/model_int8.onnx | 138,730,982 | `139006aea2cbd8e709d322f056232570de54661f624143be4893aaa387190286` |
| edge-int8/model_int8.onnx | 108,233,527 | `e9a3a8c8cd55b3bcf329de5a9307cfae5053ce93c00330c33facd022e60daa17` |

Resolve each file under
`https://huggingface.co/Wismut/nym-pii-multilingual-small/resolve/4348999cd3c2e20c49615e9af7c6bbb45b64cd85/`.

Config contains full id2label. [Export contract](https://github.com/byteowlz/nym/blob/main/scripts/export_onnx.py):
input_ids/attention_mask INT64 [batch,sequence]; logits FP32 [batch,sequence,81],
dynamic dimensions. Tokenizer is BPE/Metaspace/byte fallback, BOS=2/EOS=1. Python
returns character offsets, not necessarily Rust byte offsets. Several fallback
tokens may share one Unicode span.

Isolated nym_probe.py tokenizes once, adds specials per bounded window, never
splits overlapping offset groups, converts to UTF-8 bytes. BIO/fragment merge
follows the [reference Rust decoder](https://github.com/byteowlz/nym/blob/main/src/engine/ner_token.rs).
Its fixed taxonomy adapter is not a second production anonymization engine.
Syntax/lightweight Unicode BIO/offset-group checks alone prove no model quality
or performance.

### Distillation preserving product scope

[XtremeDistil](https://arxiv.org/abs/2004.05686) studies multilingual NER compression
with measured quality reduction. Its numbers do not transfer to our model.
Train a smaller multilingual classifier covering current categories including
EVENT rather than repeating unused zero-shot flexibility per window. Teacher
logits/hidden states may supervise but are not ground truth. Keep Czech/other gold
separate from synthetic filler and frozen after evaluation. Evaluate pruning and
quantization-aware training one step at a time.

### Bi-encoder and label-cache limits

[GLiNER architecture](https://urchade.github.io/GLiNER/architectures.html) distinguishes
joint bidirectional label/text encoding from separate encoders. Current multi-v2.1
contextual label embeddings depend on text and cannot be exactly cached; neither
can overlap hidden states across different windows. Exact token-ID caching is not
contextual-compute caching.

[Bi-small-v2.0](https://huggingface.co/knowledgator/gliner-bi-small-v2.0): Apache-2.0,
108M parameters, edge 60M. Published English/ModernBERT-Ettin variants are not an
accepted multilingual replacement. Claimed 130× is 1024 entity types on H100.
For eight labels the published cached/uncached throughput is 15.3/11.6 ex/s (~1.32×),
not our CPU result. Eight labels cannot yield thousand-label savings. ONNX needs
a separate embedding-matrix contract; do not switch gline-rs export by filename.
[Primary study](https://arxiv.org/abs/2602.18487).

### StreamingSpan and sparse context

[StreamingSpan](https://urchade.github.io/GLiNER/architectures.html#gliner-streamingspan)
uses causal decoding, KV/prompt/word cache and rolling right-context span
recomputation. Cache grows with document, not constant cost.
[Stream PII](https://huggingface.co/knowledgator/gliner-stream-pii-v1.0): Apache-2.0,
Qwen3-0.6B, public weights ~2.71 GB, gliner>=0.2.28. No ready ONNX replacement for
current Rust. Card acknowledges weaker multilingual legal text; research candidate,
not first choice for copied complete documents.

Sentence overlap, sliding/local attention and rolling context are distinct ways
to reduce repeated work. None may skip input. Changing the current attention mask
to sparse/local changes model function and needs training or an explicitly separate
quality experiment. [mmBERT](https://github.com/JHU-CLSP/mmBERT) trains alternating
local/global attention, not a free flag for DeBERTa. ONNX may implement local masks
with dense compute; kernel profiling must establish savings.

[Token-level early exit](https://aclanthology.org/2021.acl-long.16/) has labeling
train/inference mismatch and needs fine-tuning. [SpanDec](https://arxiv.org/abs/2604.20447)
changes decoder/filtering. Both can alter recall; neither is a safe postprocessing
switch. Capitalization/PII-trigger filters cannot replace detection for names,
CJK and contexts lacking those triggers.

## Comparison baseline and experiment locations

Optimize **first processing of long varied UTF-8**, not exact repeated-input cache.
Require reused_windows=0 and inference of every planned window. Earlier audit
identified inference, not core/tokenization, as dominant. Keep production ORT 1.20
and verified isolated 1.31 gains separate. Earlier reports linked above are historical,
not new 2026-10-13 measurements.

Relevant paths: ner/src/lib.rs (load_with_backend, detect_with_progress, metrics),
chunks.rs (Windows::prepare), pipeline.rs (encode/decode/byte offsets), examples/
benchmark.rs (varied corpus and phase timing), quality_compare.rs (gold expectations).
Original per-call cache keeps the first 64 exact windows; desktop additionally
holds one exact whole-input detection. Changing reuse is not first-varied speed.

| Technique | Required checks | Claim boundary |
| --- | --- | --- |
| Backend/kernels/equivalent graph | Same tensors, complete spans/kinds, final text and restoration. | Changed FP32 order can cross thresholds; same weights are insufficient. |
| Windows/context/model representation | Gold recall, false positives, seams, languages, replacements/restoration. | Context changes are not exact optimization. |
| New/distilled/quantized model | All quality gates including known failures. | Baseline is not truth; equal recall needs labelled data. |
| Exact reuse | Byte equality, config identity, shifted offsets and safe clearing. | Separate repeated workload, not reuse zero. |

## Progressive candidate gates

1. Record pinned artifacts, license, hashes, tensors, labels, threshold/tokenizer;
   distinguish existing implementations from new training/export proposals.
2. Short isolated varied screening and existing gold. Reject lost coverage before
   expanding costly benchmarks.
3. Complete varied 100 KiB, reuse zero, cold/warm phases, full ranges and exact
   restoration; sequential matched controls and separate loading.
4. Expanded multilingual gold, negatives and seams; classify every changed span/kind.
5. Only passing candidates run full varied 1 MiB with result/memory/cancellation
   checks, equal completed-run counts for candidate/control and reported N.
6. Integrate only after evidence. Benchmarks do not prove native UI, permissions
   or actual protected paste.

## Performance gate

Use release, same machine/assets/corpus, sequential owned inference without
agent-run overlap. Do not restart user apps or touch their data. Required sizes:
102,400 and 1,048,576 bytes, generic varied content without routing by JSON, format
or language. Report planned/inferred/reused windows, replacements and actual
completion. Skipped windows are not inference; changed planners must prove full
coverage.

Separate checksum/load, windows, tokenization, preprocessing, inference, decoding,
core and restoration; report total and whole-process peak RSS. Model size is not
runtime memory. Cold means fresh process/session without clearing filesystem
cache unless explicitly designed. Warm means another complete resident-session
pass with reuse zero. Report N. Screening may be N=1; decisive winners need controls
before/after when load/temperature can drift. Small N does not establish p50/p95.
Report total/inference ratios against the same baseline; absolute full 1 MiB time
determines usability, not faster windows alone.

## Quality gate

**Output equivalence and PII recall are distinct.** Matching baseline spans/kinds
proves parity on that input, while baseline may miss entities. Exact restoration
proves reversibility, not anonymization completeness.

| Property | Metric |
| --- | --- |
| Model equivalence | Added/removed/reclassified byte spans versus FP32; equal counts insufficient. |
| Exact gold | Exact expected range/kind; report different categories separately. |
| Gold coverage | Entire expected range masked, not partial coverage counted as success. |
| Protection regression | Every baseline-covered gold newly uncovered by candidate, per case. |
| False positives | Unjustified replacements on labelled negatives; inspect new nongold differences. |
| Reversibility | restore(anonymize(input)) equals complete original UTF-8 byte-for-byte. |

Gold includes synthetic people, organizations, locations, addresses, events in
CS/EN/DE/FR/RU/JA/ZH: short names, diacritics, Cyrillic, unspaced scripts,
punctuation and mixed languages. Labels/spans derive from synthetic specification,
not current model output. The existing 21 expectations at four positions are a
minimum, not general multilingual precision/recall.

Prelabel benign technical/general terms, numbers and prose; do not change
expectations after results. Report precision/recall only on genuinely labelled
data, never baseline difference counts. Window changes need gold before/across/
after each actual token-plan seam/overlap, varied prefixes/entity lengths and
multibyte characters. Fixed ASCII prefix lengths do not guarantee seam coverage.
Check byte boundaries, deduplication, replacements, full coverage/restoration.
Long decisive tests need gold within varied 100 KiB/1 MiB; repeated padding with
few inferred windows tests offsets/reuse, not long inference recall.

Reject newly uncovered known positives. Better coverage still needs false-positive
and category review. Passing this limited suite never means complete anonymization
of arbitrary input.

## Safety and integration gate

Model failure, timeout/cancellation must return a specific error, never regex-only
fallback or incomplete-document success. Do not publish cache results or paste
originals on error/cancellation. Generation/epoch, target/clipboard checks and
current vault snapshots remain separate integration invariants. Evidence contains
synthetic data/scalar diagnostics only, never user originals, system clipboard or
mappings. Quality/runtime experiments do not change production without a concrete
subsequent integration and verification.
