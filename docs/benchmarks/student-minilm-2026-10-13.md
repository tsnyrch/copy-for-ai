# MiniLM: first trained token candidate

Date: 2026-10-13. Goal: anonymize an entire varied UTF-8 1 MiB input within
10 seconds without losing existing coverage. Production GLiNER remains unchanged.

## Model and compute screening

Public `microsoft/Multilingual-MiniLM-L12-H384`, revision
`6e8c1ec6b4ec4e3fc6eb7d2cd834fcd582b61daf`, has 12 layers, hidden size 384,
a BERT encoder and XLM-R tokenizer. Original weights SHA-256:
`cce170910f4d4f3b45be025508f51ef1ad6a1de69f2d46dce7e4603ad31aaaeb`.
The new head has 11 BIO classes for five existing categories, including ADDRESS/EVENT.

FP32 CoreML CPU_AND_GPU, batch eight, bucket 256, overlap 32, no inference reuse.
Every token belongs to exactly one window, including the tail; final incomplete
batches are computed. Apple M5 / 32 GiB / macOS 27.0.1.

| Synthetic input | Valid windows | Warm median of full Python pipeline |
| --- | ---: | ---: |
| 100 KiB | 92 | 405.34 ms |
| Varied 1 MiB | 951 | 4060.96 ms |

This initial screening uses an **untrained, nonzero head**. It includes tokenization,
neural inference, token ownership, argmax and BIO decoding, but not actual core
masking/restoration. It does not establish complete anonymization. 1 MiB has
211,003 tokens; warm runs 4089.99 / 4060.96 / 4040.86 ms, first document after
numerical warmup 4131.90 ms. CPU/CoreML on 4 KiB: max absolute delta 4.61936e-7,
no argmax changes. Peak RSS 2,183,888,896 bytes includes CPU reference and CoreML.
This headroom supports keeping 12 layers; six layers were not tried.

After training, the actual checkpoint with deterministic whitespace-trimmed offsets
repeated varied 1 MiB: warm median **4020.81 ms**, runs 4003.50 / 4020.81 / 4021.20 ms,
first document 4023.77 ms. Again 211,003 tokens / 951 valid / 952 computed windows,
reuse zero, complete token ownership, 20,456 decoded spans. CPU/CoreML 4 KiB max
delta 1.0252e-5, unchanged argmax. Peak RSS 2,481,504,256 bytes includes checkpoint
verification, CPU reference and CoreML. Still excludes core, load, compilation
and native workflow.

## First checkpoint

200 FP32 MPS steps, batch eight, AdamW, learning rate 2e-5, 55.29 s. All 12 layers
and BIO head received finite nonzero gradients. Embeddings/unused pooler remained
frozen. Checkpoint SHA-256:
`918636493f4cd7428c672c44bb3ea472033f5243cf17e3223f226c3b4773c5eb`.

Synthetic TRAIN/DEV cover seven languages and five categories. Texts, groups and
canonical entities are disjoint, but similar templates permit only smoke-level
conclusions. Of 756 gold TRAIN documents, 748 were used; eight unrepresentable
boundaries and 84 review cases were not trained as negatives. DEV has 252
representable gold documents and 28 unresolved review cases in the total denominator.

The original 93 documents / 421 gold occurrences were not used for training.
No GLiNER teacher loss yet: supervised initialization, not completed task-specific
distillation.

## DEV evaluation repair

Original DEV F1 0.81879 was not authoritative: ignored whitespace tokens ended
entities. The gold BIO oracle reproduced all 216 false / 108 missing spans.
The repaired decoder skips empty whitespace offsets without ending the entity;
no expansion or snapping to gold boundaries.

Same checkpoint, no more training: **252/252 gold documents exact, TP 840 / FP 0 /
FN 0**, all five categories and seven languages. Review cases remain unresolved:
252/280 documents exact overall. Original report/checkpoint hashes remained
unchanged. Three decoder regressions passed. Template DEV is not proof of general
quality.

## Frozen acceptance and actual core

All 93 original documents processed without training/tuning on their texts/gold.
Offsets trim Unicode whitespace only. Empty tokens do not create boundaries;
BIO decoding runs once after merging owned tokens from all windows.

Every document's first batch was checked against CPU. Owned-token max delta
3.93987e-5, no argmax changes. Shape/finiteness checked at all positions of all
windows. Padding reached 1.16616e-4, so the initial all-position check stopped after
52 documents. One diagnostic prediction located the over-limit difference in
unused padding. Tolerance remains 1e-4 for owned tokens plus identical argmax.
Padding, specials, dummy rows and unused overlap positions are reported separately.
This does not prove CPU parity for every batch of a long document.

UTF-8 byte spans passed existing Rust `speed_probe validate`: actual
Vault.process/restore and fresh Session.anonymize_with_entities/restore,
matching counts and exact restoration for all 93 documents. Original regexes,
validators and overlap resolution remain active.

| Metric / 421 gold occurrences | NER alone | Final core |
| --- | ---: | ---: |
| Baseline uncovered | 36 | 36 |
| Student uncovered | 12 | 12 |
| Lost previous coverage | 9 | 9 |
| Newly covered gold | 33 | 33 |
| New spans on annotated benign text | 9 | 8 |
| Acceptance | Failed | Failed |

Student covers 409/421 versus baseline 385/421. All nine regressions are LOCATION
in Japanese, Chinese and the multilingual tail. Better totals cannot override
regression acceptance. Coverage means original characters are masked; it is not
category accuracy or an overall false-positive rate over unannotated characters.

The additional varied 1 MiB with 20,456 spans failed the core bridge before
restoration verification. Spans have valid UTF-8 boundaries and no overlaps.
Before core: 8820 distinct `(kind, original)` mappings / 172,872 original bytes,
against defaults 4096 mappings / 8 MiB. Likely VaultFull, but the generic probe
suppresses the variant; typed confirmation was not performed. Limits were not
raised. See varied-1m-core-failure.json. **4.02 s does not meet the complete
anonymization goal.**

Of 8816 unique Person values, 8814 contain digits, strongly suggesting over-detection
of reference fragments. The speed workload lacks complete gold. Raising capacity
would not establish quality. Next training needs hard negative references/codes,
not only similar positive templates.

## Decision

Keep 12 layers and FP32; do not deploy this candidate. Distill GLiNER into the
fixed BIO head on a new, varied synthetic corpus, particularly LOCATION contexts
in unspaced scripts and hard negatives. Expand template families and split them
across TRAIN/DEV; retain five categories and report unrepresentable boundaries.
Do not learn/tune from the frozen 93 documents. Require zero new coverage losses,
zero benign regressions and successful actual varied 1 MiB core processing.

## Evidence and limits

Small evidence, hash manifests and source snapshots: student-2026-10-13/.
Large public weights/checkpoint/CoreML stayed in /private/tmp/copy-ai-student-2026-10-13;
production model not replaced. No native UI, permissions or actual system paste.

Checks: AST of seven Python sources, three decoder regressions, real FP32
training/recheck, CoreML numerical checks and 93 core round trips. Rust application
code was unchanged in this experiment. MCP graph/reindex returned Transport closed;
known source paths were read directly.
