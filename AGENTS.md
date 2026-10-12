# Copy for AI — agent rules

## Workflow

- Before making changes, read `docs/plan/README.md`, the relevant plan and `git status --short`.
- Preserve work in progress. Do not reset, commit or push without explicit user instructions.
- Keep `docs/plan/delivery.md` and `docs/plan/verification.md` current. Distinguish implementation, automated tests, native UI, permissions and actual system paste.
- Prefer a small completed change with concrete verification. Do not describe regex-only processing as complete anonymization.
- Do not propose MCP, a skill, a cloud service or a web frontend as a replacement for the standalone app.

<!-- codebase-memory-mcp:start -->
## Codebase Knowledge Graph

ALWAYS prefer codebase-memory-mcp over grep/glob/file-search for code discovery.
Index this repository first if it is not indexed. Priority: `search_graph`,
`trace_path`, `get_code_snippet`, `query_graph`, `get_architecture`.
Use grep/glob only for string literals, error messages, configuration/non-code
files, or when graph results are insufficient. Re-index after structural changes.
<!-- codebase-memory-mcp:end -->

## Structure and responsibilities

- `crates/ner`: local multilingual inference, tokenization and verified model assets; no GPUI or clipboard.
- `crates/core`: domain types, detectors, validators and sessions; no GPUI, OS, clipboard, network or disk.
- `crates/desktop/src/application`: application workflows, coordination and services; no rendering.
- `crates/desktop/src/ui/screens`: screen composition, retained GPUI entities and workflow bindings.
- `crates/desktop/src/ui/components`: small UI components with explicit inputs and callbacks; no detection, IO or vault mutations.
- `crates/desktop/src/platform`: OS adapters and the only native `unsafe` boundary.
- `crates/cli`: a thin input adapter to the same core; do not introduce a second anonymization engine.
- `docs/plan`: product, architecture, dependencies, milestones, evidence and known limitations.
- Behavioral tests belong in `tests/`; small tests of private validators may live beside the implementation.

## OOP in Rust

- Use encapsulated `struct` + `impl`, composition and ownership. Do not imitate class inheritance.
- Domain state has one owner; methods enforce its invariants. Fields are private unless deliberately exposed as immutable DTO values.
- Introduce traits for alternative detectors/platforms only when multiple implementations or a test adapter are actually needed. Avoid an abstraction for every function.
- Enums represent entity kinds, modes and result states. Avoid boolean parameters that conflate distinct operations.
- Domain errors are typed and propagated through `Result`; the UI presents understandable messages. Clipboard content, permission failures and shortcut conflicts must not panic.
- Keep workflows, native adapters and rendering separate; the main view must not become a universal object.

## Code style

- Rust 2024 and stable toolchain. `rustfmt.toml`, workspace lints and CI are authoritative.
- Use `snake_case` for modules/methods, `PascalCase` for types/traits and `SCREAMING_SNAKE_CASE` for constants.
- Code, identifiers, comments, user-facing text and documentation must be in English. Synthetic multilingual test/benchmark data retain their original languages to exercise detection, tokenization and UTF-8 invariants. Historical screenshots are evidence of their original build, not current UI translations.
- Prefer explicit types and small methods. Aim for modules around 350 lines; split larger modules by responsibility rather than mechanically by line count.
- Business logic does not belong in `Render` or reusable components. `Render` composes elements and reads state.
- Long operations and waits must not block the UI thread. Preserve cancellation/generation guards and target checks before paste.
- Create GPUI entities during construction or explicit reset, not every render. Retain subscriptions/tasks for their required lifetimes.
- Dependencies need a clear reason; keep `Cargo.lock` committed. Do not mix incompatible GPUI runtime types.
- Do not hide lint issues with global `allow`. Narrow exceptions need substantive justification.

## Comments

- Keep comments minimal. Explain invariants, non-obvious decisions or safety boundaries.
- Do not narrate obvious code, add change history, decorative separators or TODOs without a concrete milestone.
- Use brief public API `///` comments where they explain contracts, limits or side effects.
- Every `unsafe` block needs a specific `SAFETY:` comment explaining valid inputs, ownership and lifetimes. This is an intentional exception to comment brevity.

## Generic input and languages

- Input is UTF-8 text. Do not introduce JSON field routing, format parsing or language-specific name lists.
- Detectors return byte spans in the original text; preserve all other content exactly.
- Unspaced Unicode scripts need correct token boundaries, not identity regexes for each language.
- The local in-memory vault chooses direction using exact markers. Never scan restored originals again.
- Model failure stops anonymization; never silently fall back to regex-only processing.

## Data protection

- Never put originals, clipboard content or mappings in logs, Debug, errors or telemetry. Sensitive types must not automatically derive Debug/Serialize.
- Originals and sensitive custom terms stay in memory. Persistence/export requires a separately designed explicit milestone.
- Errors, timeouts, target changes or clipboard changes must not paste the original input through protected paste.
- Write only the plain-text result; remove original HTML/RTF representations. Directly pasting restored text requires the explicit protected Cmd+Shift+V command.
- Cmd+Shift+R restores only the clipboard. Cmd+Shift+V anonymizes or exactly restores according to local markers, then pastes into the original target after safety checks. Preview, loading and other processing do not paste.
- Preserve and report unknown markers. No identity guessing or fuzzy restoration.
- History contains at most 100 metadata records in memory. Explicitly requested lifetime statistics may persist only scalar aggregates without content, markers, mappings or hashes. Forget everything resets both.
- Tests and demos use synthetic data. Zeroizing owned originals does not prove that all GPUI/OS/swap copies have been erased.

## Development priority and proportionate verification

- Finish the functional change. Do not spend disproportionate time on tests, repeated checks or expanding test infrastructure.
- During development, use the smallest relevant check. Do not run several broad suites after one minor fix.
- Add tests only for substantial regressions or risky invariants; do not mirror implementation or test trivial changes.
- Do not repeat successful checks unless later changes affect that area or a concrete new concern arises.
- Run the broad suite below once when handing over a cohesive change across several parts of the app, not after every small edit.
- Run expensive model tests and benchmarks for inference changes, quality uncertainty or explicit user requests, not unrelated repairs.
- Documentation-only changes require no build or tests. Explicit user-requested checks take precedence.

## Verification of a cohesive change

```sh
cargo fmt --all -- --check
cargo test -p copy-for-ai-core -p copy-for-ai-cli
cargo test -p copy-for-ai-desktop
cargo check -p copy-for-ai-desktop
cargo clippy --workspace --all-targets -- -D warnings
```

Test substantive invariants: UTF-8, overlaps, stable sessions, foreign markers,
single-pass restoration, capacity limits and cancelled paste operations. Do not
write tests that merely repeat implementation or check decorative UI styles.

Perform native build/UI and paste proof on a supported platform. State explicitly
in the handover if permissions or actual paste were not verified.
