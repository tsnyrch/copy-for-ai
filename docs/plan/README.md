# Copy for AI — application plan

Status: first-version implementation. Initial research: 2026-10-12.
English repository preparation: 2026-10-28.

## Goal

A standalone local Rust app. Users copy text normally and use a protected shortcut
to paste a pseudonymized version. Copied AI replies can be restored using mappings
held only by their app. The AI agent needs no MCP, plugin or skill.

Product: **Copy for AI**. UI: Zed GPUI through GPUI Kit 0.7.3. Primary first-version
platform: macOS. This is reversible pseudonymization, not guaranteed irreversible
anonymity or removal of every sensitive contextual clue.

## Documents

- [New techniques and the 1 MiB / 10-second goal](anonymization-speed.md)
- [Long clipboards: complexity, model, concurrency and next steps](long-clipboard.md)
- [Generic multilingual processing and automatic direction](generic-processing.md)
- [Product and controls](product.md)
- [Settings, updates and application-detection roadmap](roadmap.md)
- [Architecture and safety boundaries](architecture.md)
- [GPUI, sources and dependency decisions](gpui.md)
- [Milestones, acceptance criteria and delivery](delivery.md)
- [Project structure and implementation rules](project-structure.md)
- [Verification and evidence limits](verification.md)
- [Benchmarks and raw evidence](../benchmarks/README.md)

## First implementation scope

- Rust workspace with shared core, GPUI desktop, local NER and a thin CLI.
- Local multilingual GLiNER for people, organizations, locations, addresses and
  events; rules for UUIDs, email, phones, IBANs, Czech personal identifiers and secrets.
- Automatic direction from exact local markers; up to 16 in-memory sessions and
  4096 mappings, with one active session and stable typed markers.
- Literal, whole-word, case-sensitive custom PERSON/ORG/TERM rules, without inferred
  merging of people or inflected names.
- Exact single-pass restoration, including reporting unknown markers.
- GPUI input/output, detection preview, clipboard loading, plain-text copy,
  explicit global shortcut activation and forgetting sessions.
- Cmd+Shift+V processes the current clipboard and performs guarded paste;
  Cmd+Shift+R restores the clipboard only. Known markers restore without detection;
  unknown-only markers block paste.
- Limits: anonymization input 1 MiB; restoration input/output 32 MiB; originals
  8 MiB; 256 custom terms. Errors or target/clipboard changes stop paste.
- No network service or persistence of originals, mappings or clipboard history.

Workspace, History, Statistics and Settings are separate pages. History is capped
at 100 metadata entries in memory; lifetime statistics persist scalar aggregates
only. See the product and verification documents for their separate boundaries.

## Outside first-version acceptance

A verified multilingual precision/recall benchmark, inflected alias merging,
encrypted disk vaults, configurable global shortcuts, ordinary Cmd/Ctrl+V
interception, Windows/Linux automatic paste, images, PDF, RTF and attachments.
UI limitations and model misses must remain visible. Experimental throughput
is not proof of an accepted production model or complete workflow latency.

## Reference products

- [BeforePaste](https://github.com/beforewire/beforepaste): Rust paste guard,
  especially macOS; compare controls and target identification limits.
- [swissprismia/PII Shield](https://github.com/swissprismia/pii-shield): Rust/Tauri,
  Python/Presidio and bidirectional tokenization.
- [ClipRedact](https://github.com/prithvi1236/redactor): shortcuts and reversible
  in-memory mappings.
- [gregmos/PII-Shield](https://github.com/gregmos/PII-Shield): document CLI and
  sessions, alongside its MCP variant.
- [CamoText workflow](https://camotext.ai/blogposts/use-claude-cowork-with-anonymization.html):
  typed markers separating originals from text visible to an agent.

These are public-documentation research references. Their runtime and privacy
claims were not independently verified.
