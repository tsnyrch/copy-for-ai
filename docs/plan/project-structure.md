# Project structure and implementation rules

The [generic multilingual pipeline](generic-processing.md) uses automatic
direction and a multi-session memory vault. NER does not route by JSON keys or UI
language. Model loading and inference run outside the UI thread.

```text
copy-for-ai/
├── AGENTS.md                 agent contract and data protection
├── Cargo.toml / Cargo.lock   workspace, shared dependencies and lints
├── LICENSE / THIRD_PARTY.md  source license and dependency/model boundaries
├── rustfmt.toml              stable formatter
├── rust-toolchain.toml       stable Rust, clippy and rustfmt
├── .cargo/config.toml        development aliases
├── .github/workflows/ci.yml  Linux core and macOS desktop checks
├── crates/
│   ├── core/                domain, detectors, validators, sessions and vault
│   ├── ner/                 local inference, tokenization and verified assets
│   ├── desktop/src/
│   │   ├── main.rs          GPUI bootstrap, window and quit
│   │   ├── application/     workflows, workers, diagnostics and shortcuts
│   │   ├── platform/        macOS adapters and explicit unsupported capabilities
│   │   └── ui/
│   │       ├── screens/     retained screen state and composition
│   │       └── components/  editor, session, terms and status components
│   └── cli/                 thin terminal adapter
├── scripts/                 model installation, packaging and experiments
└── docs/                    plans, benchmark data and historical evidence
```

Behavioral tests live in each crate's `tests/`; small private validator tests may
remain next to implementation.

## Composition and OOP

`Session` protects originals and stable identity; `Vault` owns retained sessions.
`Detector` builds spans and resolves overlaps. Application `Workflow` coordinates
the domain and custom rules; the GPUI view owns editors and publishes accepted
results. Native adapters own clipboard/target state, permissions and paste.
Reusable components receive values/entities and callbacks, never vault ownership.

Add alternate model interfaces with actual implementations, not speculative
layers. Platform-specific capabilities share a facade and report their real
support status.

## Formatting and comments

Format the workspace. CI rejects formatting differences and Clippy warnings.
Comments explain non-obvious invariants and decisions; API documentation explains
contracts. Each unsafe block requires a concrete `SAFETY:` rationale. Do not add
comments that simply repeat code. All prose is English; multilingual synthetic
fixtures intentionally exercise language and Unicode behavior.

## Current organization

Core separates detector, session, vault, types, validators and markers. Desktop
separates workflows, shortcuts, settings, statistics, history, detection cache and
diagnostics from native adapters and UI components. Screen state, event handlers,
clipboard processing and rendering have distinct modules. Keep actual checks and
known limitations in [verification](verification.md).
