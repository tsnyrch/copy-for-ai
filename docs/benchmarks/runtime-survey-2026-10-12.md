# Additional model and runtime paths — 2026-10-12

## Decision

Production remains **GLiNER multi-v2.1 FP32, CPU ORT**. Q4 and GLiNER-X-small were
faster in some trials but lost coverage of known synthetic entities. The pinned
ORT ARM BF16 switch does not affect macOS MatMul. None passed the joint speed and
quality gate. INT8, CoreML, static GPU and batch/thread tuning were not repeated.

## Primary sources and availability

Public revision, license, size and LFS SHA-256 snapshot:
[model-survey-2026-10-12.json](model-survey-2026-10-12.json). Registry metadata
establish artifact availability, not language quality. Japanese and Cyrillic were
therefore checked through actual inference.

| Candidate | Size / license | Language and compatibility | Outcome |
| --- | --- | --- | --- |
| [GLiNER multi-v2.1 Q4](https://huggingface.co/onnx-community/gliner_multi-v2.1/tree/6ddaeb9413b0e71ad8457da1aab378a165b24058/onnx) | 893,933,650 bytes; source weights Apache-2.0. | Same multilingual model/tokenizer, direct Rust ORT; downloaded and verified. | Measured; rejected on quality. |
| [GLiNER-X-small](https://huggingface.co/knowledgator/gliner-x-small) | ONNX 688,773,285 bytes; Apache-2.0. | mT5-small; metadata explicitly lists cs/en/de/fr/zh/uk among others, not ru/ja. Public ONNX/tokenizer downloaded and verified. | Isolated Rust ORT reproducer works; rejected on quality. |
| [GLiNER small-v2.5](https://huggingface.co/gliner-community/gliner_small-v2.5) | FP16 332,065,392 bytes; Apache-2.0. | Multilingual tag but `microsoft/deberta-v3-small` config; model card does not establish required languages. Revision has safetensors, no ONNX. | No convincing language/export advantage; not downloaded. |
| [GLiNER2.5 multi-v1](https://huggingface.co/fastino/gliner2.5-multi-v1) | Safetensors 1,149,461,028 bytes; Apache-2.0. | mDeBERTa-v3-base, 287M parameters, new boundary/schema extractor; no ONNX and incompatible with current span adapter. | Not a smaller drop-in replacement; not downloaded. |
| ARM64 BF16 fast-math | Original FP32 artifact. | Hardware reports `FEAT_BF16=1`, but [ORT 1.20 CPU MatMul](https://github.com/microsoft/onnxruntime/blob/v1.20.0/onnxruntime/core/providers/cpu/math/matmul.cc) gates this path on `__aarch64__ && __linux__`. | Ineffective for this macOS MatMul; rejected. |

[Source GLiNER](https://huggingface.co/urchade/gliner_multi-v2.1) is Apache-2.0.
Q4 is another export of the same pinned ONNX revision, distinct from earlier INT8:
its unoptimized graph has 81 MatMulNBits and 72 MatMul, versus 153 MatMul in FP32.
[ORT weight-only quantization](https://onnxruntime.ai/docs/performance/model-optimizations/quantization.html)
does not itself promise quality or GPU acceleration.

## Conditions and measurements

Same M5/macOS and ORT 1.20.0 as prior measurements; Rust release, 256 subwords,
batch one, eight CPU threads, eight original labels, threshold 0.4. Every run
used a fresh isolated process and the same synthetic `benchmark.rs` varied corpus.
All windows were inferred, **reuse=0**. Verification and session creation are
separate; run zero is first inference, run one warm. Filesystem cache was not
cleared. RSS is wait4 process maximum including cancellation, not weight size or
a hard memory limit.

100 KiB order: FP32 A → Q4 → BF16 probe → FP32 B, without concurrent owned inference.
User processes remained running and uncontrolled. Initial 10 KiB screening varied
substantially and is not the main Q4 speed evidence. N=1/2, not percentiles.

| Varied 100 KiB | Verify / load ms | First / warm ms | Peak RSS bytes | Cancel ms |
| --- | ---: | ---: | ---: | ---: |
| FP32 A | 4134 / 1346 | 20,423 / 22,946 | 2,641,510,400 | 293 |
| Q4 | 1961 / 1190 | 21,837 / 19,143 | 2,589,179,904 | 354 |
| FP32 B | 2065 / 1263 | 20,119 / 23,588 | 2,444,279,808 | 272 |

Q4 warm gain over mean controls: **1.215×, 17.7% less time**. First inference was
about 8% slower. RSS does not establish consistent savings. Every run processed
111 windows, replaced spans and restored exactly. BF16 warm was 20,310 ms; source
conditions mean its difference must not be attributed to BF16 computation.

| Varied 10 KiB, isolated X-small reproducer | Verify / load ms | First / warm ms | Peak RSS bytes | Cancel ms |
| --- | ---: | ---: | ---: | ---: |
| FP32 control | 2311 / 1293 | 2349 / 1974 | 2,802,843,648 | 417 |
| X-small | 1255 / 368 | 1150 / 1254 | 2,362,261,504 | 327 |

X-small warm was **1.574×, 36.5% less time**, with 15.7% lower RSS in this pair.
Both processed 11 distinct windows and restored exactly. No costly varied 1 MiB
speed run was performed after quality rejection.

## Quality and complete input

`quality_compare` contains 21 synthetic expectations: CS/EN/DE/FR/RU/JA/ZH, short
names, organization, location, event and addresses, each in four positions. Gate
prefixes are 0/27/31/35. Check exact span/kind and full coverage by any kind
individually; neutral prefix is not a gold identity. FP32 is not ground truth.
Count all span/kind differences and verify byte-for-byte restoration per case.

| 84 gate cases | Missing exact gold, FP32 / candidate | Uncovered gold, FP32 / candidate | Lost FP32 coverage |
| --- | ---: | ---: | ---: |
| Q4 | 13 / 14 | 11 / 11 | **1** |
| X-small | 13 / 27 | 11 / 15 | **10** |

Q4 case 13, prefix 31 lost the expected event. Equal aggregate uncovered counts
hide exchanged successes/failures. X-small lost a short Czech name and English,
Czech and Cyrillic addresses. A complete 1,048,576-byte document evaluated 1391
windows: two inferred and 1389 exact reuses. Of nine multilingual tail expectations,
FP32 covered eight, Q4 six and X-small nine. Entire-document restoration passed.
This repeated input proves tail coverage/restoration, **not model throughput**.

Original Q4 prefixes 0/180/210/240 had 14 exact gold misses per variant but 12,841
lost and 12,618 added span/kind pairs. That motivated the shorter positional gate.
The gate has 1532 lost / 1280 added pairs for Q4 and 1780 / 219 for X-small. These
include unlabelled filler and are not false-positive/negative rates or precision/recall.

## Reproduction and repository changes

FP32 stays the RuntimeConfig default. No application/UI switch. Only explicit
ModelVariant::Q4 evaluation, size/hash validation and an opt-in installer flag
were added; no silent fallback.

```sh
python3 scripts/install-model.py --q4
python3 scripts/install-model.py --q4 --verify-only
cargo run -p copy-for-ai-ner --release --example benchmark --locked --offline -- 102400 256 1 8 fp32 varied
cargo run -p copy-for-ai-ner --release --example benchmark --locked --offline -- 102400 256 1 8 q4 varied
cargo run -p copy-for-ai-ner --release --example quality_compare --locked --offline -- q4
cargo run -p copy-for-ai-ner --release --example quality_compare --locked --offline -- q4 gate
```

Q4 SHA-256: `3c54cf1332d491f7f4057504df4ac78a78712874aca1c5731330430e0c55259b`.
Raw outputs: `candidate-*.txt`. Append `bf16` to the FP32 benchmark to reproduce
the ineffective BF16 probe; this is not a production recommendation.

[xsmall-experiment.tar.gz](xsmall-experiment.tar.gz) contains an isolated Rust
workspace without weights. **Only there**, alias Q4 means X-small, with its own
hash/tokenizer and mT5 EOS=1 without initial CLS. The product Q4 adapter is unchanged.
Extract outside the app; download ONNX/tokenizer from the snapshot-pinned X-small
revision and verify sizes/hashes. Place FP32 as model.onnx/tokenizer.json and
X-small as model-q4.onnx/tokenizer-small.json in that reproducer's model directory.
Run the same examples using explicit `COPY_FOR_AI_MODEL_DIR`. This is a reproducer,
not an installer or a new production engine.

## Verification and delivery evidence

Passed locally for this isolated evaluation:

```sh
cargo fmt -p copy-for-ai-ner -- --check
cargo test -p copy-for-ai-ner --lib --locked --offline
cargo clippy -p copy-for-ai-ner --all-targets --locked --offline -- -D warnings
```

Window test 1/1; actual quality gates and Q4 installer checks are above. Broad
desktop/UI suites were not repeated. Graph reindexed after adding the eval variant.

FP32 remains default. Q4 and X-small are rejected for demonstrated coverage losses.
17.7% (Q4 100 KiB) and 36.5% (X-small 10 KiB) are isolated candidate gains, not
production app improvements. Historical varied FP32 1 MiB remains 308,388 ms /
3.10 GiB. UTF-8 offsets, labels, threshold, full input, atomic core processing,
timeout and between-run cancellation remain. Candidate cancellation was 327–354 ms;
ORT cannot interrupt a kernel midway. Runs used separate CLI processes without
restarting the user's app. No native UI latency, permissions or actual system paste
proof. A full independent multilingual precision/recall benchmark remains open.
