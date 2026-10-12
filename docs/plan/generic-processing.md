# Generic multilingual processing

## Contract

Input is UTF-8 text. The engine does not parse JSON, interpret field names or
select language from the interface. Bytes outside detected spans remain exactly
unchanged, including formatting, code, punctuation and line breaks. Local GLiNER
multi-v2.1 detects people, organizations, locations, addresses and events;
deterministic detectors supplement identifiers and secrets. Multilingual does not
mean every language or identity is recognized correctly. Inflected variants are
separate exact values, never merged by inference.

## Automatic direction

1. Known markers select single-pass restoration from the local vault. Inserted
   originals are not scanned again. Unknown markers, including case changes made
   by an AI, remain unchanged and are reported. Do not canonicalize case or guess
   identity.
2. Unknown-only markers block processing: without local mappings the direction is
   ambiguous. User content must not create a new mapping for an existing marker.
3. Unmarked text runs through model and rules. Validate UTF-8 spans, resolve
   overlaps and emit typed markers such as PERSON/ORG/LOCATION/ADDRESS/EVENT/UUID.
   The scope has eight hex characters, e.g. `[[PERSON_A1B2C3D4_1]]`; internal UUIDs
   are not embedded. A new session cannot reuse another retained session's scope.

Mixed input with known markers is restored only; newly added originals are not
scanned. Cmd+Shift+V processes the current clipboard and pastes after safety
checks. Unknown-only markers block paste; mixed unknowns are preserved/reported.
Cmd+Shift+R restores only the clipboard. Preview, clipboard loading and standalone
clipboard processing never paste. Copying a completed preview does not process
markers twice.

## Ownership and boundaries

- Core owns validated byte spans, priorities, sessions and direction.
- NER owns local inference and verified model/tokenizer assets, without UI/clipboard.
- Application workers load and infer in the background with generation/cancellation.
- Only the current generation can publish results. Mappings are accepted with the
  result, never at task start or from a stale completion.
- The vault retains at most 16 sessions and 4096 total mappings in memory. New
  sessions keep previous ones; forgetting clears all. No disk content history.
- Missing/failed models stop anonymization with an explanation. Known-marker
  restoration works without a model. Regex-only mode must not imply full protection.

## Model and verification

Model: [ONNX export](https://huggingface.co/onnx-community/gliner_multi-v2.1),
revision `6ddaeb9413b0e71ad8457da1aab378a165b24058`, Apache-2.0.
Adapter: [gline-rs 1.1.0](https://docs.rs/gline-rs/1.1.0/gliner/).
The FP32 file is approximately 1.16 GB. The installer checks SHA-256 before
publishing assets. Runtime processing neither sends text over the network nor
downloads weights.

Windows must cover the complete input without silent truncation. Offset checks
must establish byte semantics for diacritics, Cyrillic and CJK. Core tests cover
format-independent replacement, direction, multiple sessions, unknown markers,
single-pass restoration, limits and invalid offsets. Model smoke tests use
synthetic multilingual data. Record misses in verification rather than hiding
them with language-specific identity regexes.

## Long documents and performance — 2026-10-12

Anonymization accepts at most 1 MiB (1,048,576 bytes). Restoration/marked input and
output accept at most 32 MiB. Unmarked input above 1 MiB is rejected before model
loading. CLI `--stdin` accepts a complete document with line breaks; interactive
mode retains sessions.

Windows are planned using actual subword IDs, at most 256 including labels and
special tokens. Segment/tokenize once per operation; identical words share IDs
and labels are prepared during loading. Overlap retains up to 24 whole units,
bounded by one third of the token budget. Boundaries preserve ordinary words and
original UTF-8 offsets, covering whitespace and the document tail. Extremely long
lexical units split before tokenization. Empty encodings use the official `[UNK]`;
no input bytes are discarded.

Per-operation limits: 4,194,304 stored subword IDs, 1,048,576 model spans,
2,097,152 core candidates and 4096 distinct unknown markers. The inference cache
holds at most 64 byte-identical windows per operation, each up to 4096 spans.
Every window is accounted for and cached spans shift to exact offsets. There is
no similarity-based cache or skipping of distinct content.

Vault originals are bounded at 8 MiB. Workers hold bounded working copies of the
vault and rules. Acceptance requires valid generation, unchanged input,
clipboard, target, permissions and workflow revision. Protected paste publishes
mappings only after the last check and successful shortcut dispatch. Failure or
cancellation discards pending state.

Production keeps FP32, pinned origin and SHA-256, all eight labels and threshold
0.4. CPU defaults to at most eight hardware-available threads, batch one. Batch
two and CoreML/GPU were slower on the measured M5. INT8 lost detections and is not
the default. The overall detection timeout is 15 minutes, not a latency promise;
timeout rejects the entire result. See [benchmarks](../benchmarks/README.md) for
the distinction between repeated and varied documents.

Later application work adds an exact 8 MiB last-detection cache with invalidation
epoch, retaining current vault/rules when reused. Progress is bounded to four
publications per second. Neither optimization accepts a lower-quality model.
