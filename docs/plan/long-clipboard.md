# Long clipboards — audit and implementation plan

2026-10-13: [research and four candidate experiments](../benchmarks/speed-techniques-2026-10-13.md)
are complete. No accepted candidate meets 1 MiB within 10 seconds. See the
[Apple/student/streaming follow-up](anonymization-speed.md).

Audit date: 2026-10-12. Subsequent implementation includes progress, in-memory
phase diagnostics/cache, clipboard-only workflow and binary marker protection.
The isolated ORT 1.31.0 experiment passed performance and existing quality gates;
the production runtime/package remains unchanged. See [new results](../benchmarks/ort-runtime-2026-10-12.md).
Competitor research examined public source/documentation, without installing or
benchmarking those apps. Historical figures below were not remeasured by the audit.

## Findings and evidence

The model dominates varied long documents. A historical release 1 MiB sample ran
1143 inferred windows with no reuse: detection 308,388 ms, inference 308,167 ms,
core replacement 30 ms and restoration 3 ms. See the [raw sample](../benchmarks/after-release-varied-1048576.txt)
and [methodology](../benchmarks/README.md). A single sample does not establish
percentiles or native UI latency.

A user screenshot showed 645,204 bytes / 182,228 ms, followed by rejection due to
target, clipboard or permission state. Average throughput was about 3.54 kB/s,
close to the previous 3.4 kB/s. Similar magnitude does not establish the exact
cause. At that point history had no phase times or specific rejection reason.
Two 23,043-byte samples taking about 6.3 seconds do not prove identical content.

Reading input and writing output require at least O(n + o). Whole-document
O(log n) is impossible; aim for work proportional to input/detections and
O(c log c) overlap selection. A strict O(n log n) bound for the entire pipeline
has not been proven.

## Algorithm audit

n = input bytes; o = output bytes; w = token units; e = model spans; c = all
candidates; p = existing markers; s = total unique search-value lengths;
h = total repeatedly hashed value lengths. Account for generated matches as
well as input length.

| Stage | Approach | Complexity and conclusion |
| --- | --- | --- |
| Tokenization | `Windows::prepare`: regex segmentation, units up to 128 characters, splitting over budget, exact token cache. | Bounded tokenizer cost per unit; no whole-prefix re-encoding for each window. See regex caveat below. |
| Windows | Monotonic `Windows::next` cursor, 256 subwords including prompt, overlap at most 24 units and one third of text budget. | O(w) under fixed limits. |
| Model | Sequential bounded windows, one resident ORT session, batch 1, up to eight intra-op threads. | Attention is quadratic within a fixed window. Window count scales approximately linearly, with a large compute constant. |
| Reuse | Up to 64 exact windows within one call; HashSet deduplication of offsets/kind. | Expected hash lookup. Different contexts are not shared; the original audit had no cross-operation cache. |
| Decoding | gline-rs 1.1.0 `SpanSort` and `GreedySearch`, comparing the previous span. | O(a log a) sorting and O(a) greedy per window, with fixed width/label count. |
| Repeated values | Aho–Corasick over model surfaces and rules; one overlapping scan. | O(n + z) search after construction, plus s and h. Many long overlapping values cannot be counted as O(n) alone. |
| Marker protection | Two binary searches for first/last intersection in sorted disjoint markers. | O(c log p). A Secret must enclose extreme markers completely and cannot equal a marker alone. |
| Overlap selection | Priority/length/offset sorting and BTreeMap predecessor checks over disjoint accepted intervals. | O(c log c), without scanning every accepted entity. |
| Vault | Randomly seeded hashes and exact collision-bucket comparisons; 4096 mappings, 8 MiB originals, 16 sessions. | Expected O(h), exact even on collisions; no deterministic worst-case logarithmic-map guarantee. |
| Replacement/restoration | Prevalidate capacity and append to one buffer; scan markers in original input only, up to 16 sessions per lookup. | O(n + o) with expected lookup cost. Two passes are not quadratic. |
| UI snapshot | Clone bounded vault/rules before worker; compare input before paste. | Stored-state copying plus O(n) comparison. Up to 8 MiB of originals may affect UI, but cannot explain minutes of inference. |

Rust regex guarantees O(m n) for one search, while the general bound for
`find_iter`/`captures_iter` is O(m n²). This does not establish that our patterns
exhibit the worst case. Markers have bounded length; keys, email and PEM include
unbounded repetition. A strict guarantee requires pattern analysis and adversarial
inputs. Do not automatically use `earliest`: it changes spans.
[Regex documentation](https://docs.rs/regex/latest/regex/#iterating-over-matches).

Implemented marker protection finds first/last intersecting markers by binary
search. Ordinary entities need only an intersection test; Secret spans must
fully enclose the extremes and must not replace a marker alone. Tests need
multiple enclosed markers, partial intersections, exact marker and touching edges.

## Competitors

| Tool | Examined approach | Relevance to long input |
| --- | --- | --- |
| [PII Shield](https://github.com/swissprismia/pii-shield) | Presidio + spaCy, `en_core_web_lg`, EN/FR; sidecar passes all text to `analyzer.analyze`. | Examined path has no custom chunking. Another engine does not prove Czech quality. Replacements repeatedly slice; restoration uses one regex callback. |
| [ClipRedact](https://github.com/prithvi1236/redactor) | English `dslim/bert-base-NER`, ONNX classification, PERSON/ORG trigger words and allowlist; HF pipeline without explicit stride. | Complete long-input coverage is unproven. Repeated prefix/suffix splitting and per-entity string copies cost O(e n); per-mapping restoration costs O(v n). |
| [BeforePaste](https://github.com/beforewire/beforepaste) | README describes local credentials/common PII redaction, paste guard and target detection. | No demonstrated multilingual throughput or full 1 MiB coverage; no model optimization can be inferred without engine audit. |
| [Secret Stripper](https://github.com/kalix127/secret-stripper) | One-shot Rust CLI and system hotkey helper. | Narrower than reversible multilingual NER pseudonymization; README is not a benchmark. |
| [Quieta](https://quieta.ai/en/personal/) | Vendor describes a local model, almost 1 GB package and copy-time masking. | Preprocessing may hide wait before paste but does not accelerate inference. Window/backend/long-input benchmark details are unpublished. |

Examined source: [PII sidecar](https://github.com/swissprismia/pii-shield/blob/main/sidecar/presidio_sidecar.py),
[ClipRedact runner](https://github.com/prithvi1236/redactor/blob/main/redactor.py).
Their `main` branches move; findings are an audit-date snapshot. No comparative
measurement establishes that a competitor is faster.

spaCy recommends `nlp.pipe`, batching and disabling unused components, while
warning of macOS multiprocess memory/startup overhead. That is library guidance,
not evidence of PII Shield implementation. [Source](https://spacy.io/usage/processing-pipelines).

## Implementation following the audit

1. **Progress and specific failures — implemented.** NerService returns Metrics.
   History receives phase times and window/reuse counts only, without content,
   hashes or mappings. Limit UI updates to four per second and check generation.
   Show completed/total windows, cancellation and an estimate based on observed
   windows; separate cold loading. Distinguish target, clipboard, permission and
   held-modifier failures.
2. **Safe long workflow — implemented.** Indicate long work from token-window
   counts. Protected paste keeps its original target/clipboard contract. Offer
   explicit clipboard processing or a completed preview followed by a new guarded
   operation with a fresh target snapshot. Never paste automatically into a newly
   active app. Publish vault/text consistently; rejected paste cannot leave
   orphaned markers without mappings.
3. **Exact cross-operation cache — implemented.** Keep one bounded last detection,
   comparing all input bytes and model/tokenization configuration, not just hashes.
   Store model spans; rebuild replacements with current vault/rules. Do not retain
   failed/cancelled results. Use zeroizing input storage, bounded memory and clear
   on Forget everything. Recheck target, clipboard and revision. It speeds repeated
   identical input, not the first varied document.
4. **Faster first-pass inference — isolated experiment verified.** Batch two,
   larger windows, split CoreML, INT8, Q4 and X-small had already been tried;
   [results](../benchmarks/runtime-survey-2026-10-12.md) do not justify blind
   repetition or production switching. Compare existing ORT with a newer compatible
   CPU runtime/export in isolation. If insufficient, investigate a complete Apple
   backend or a smaller multilingual fixed-category model. These are experiments,
   not speed promises. Gate on varied 100 KiB and independent gold cases before
   running 1 MiB.

Copy-time preprocessing may later be an explicit opt-in feature. It reduces
shortcut wait only if work has already completed; CPU, memory and inference cost
remain. It must neither paste nor publish incomplete mappings. Do not introduce
a silent background clipboard scanner.

## Acceptance and evidence limits

- Process or exactly reuse every window, preserve all UTF-8 bytes and restore
  byte-for-byte. No tail truncation, sampling, language omission or regex fallback.
- Separate cold/warm, varied/repeated and model/core/UI/paste. Runtime comparisons
  use the same harness, inputs, threads, labels, threshold and gold cases.
  Reuse is not model throughput.
- Cache regressions cover changed byte/configuration, forgetting, cancellation
  and new sessions. Different bytes must never share a detection result.
- Check generation/revision after workflow changes; native paste is separate.
  Headless adapters do not establish system paste.
- The initial audit changed documentation only. Later implementation and runtime,
  service and native UI evidence are in the linked report. Actual system paste
  and competitor installations were not verified.
