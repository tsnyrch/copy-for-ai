# Milestones and delivery

## English repository preparation — 2026-10-28

Implemented English README, agent instructions, plans, benchmark reports, native
UI, CLI and typed error messages. Synthetic multilingual fixtures remain intact;
historical screenshots, raw reports and compressed source snapshots remain evidence
of the versions actually measured. No new model quality or native paste claim.

Reviewed MIT LICENSE and workspace inheritance, added repository/description
metadata and third-party license boundaries. Expanded .gitignore for nested Rust
builds, local model artifacts, Python caches/environments, local secrets and
temporary files. Cargo.lock, benchmark fixtures and evidence remain versioned.
The existing local performance/cache work is preserved in the prepared repository.
Automated checks and publication status are recorded in verification.md.

## M0 — bootstrap and plan

Workspace, pinned GPUI Kit, documentation and a runnable native window. Acceptance:
text/basic components render on the current Mac and the plan states limitations.

## M1 — functional local workflow

Core, typed markers, custom terms, exact restoration, regexes/validators; GPUI
input/output, copy/reset; explicit macOS shortcuts and safety checks.

Acceptance:

- Same exact value across operations uses the same marker; distinct people and
  sessions do not merge/share marker identities.
- Czech UTF-8, overlaps and repeated occurrences preserve text.
- Protected output is not anonymized again; restoration never cascades.
- Preserve/report unknown or foreign markers.
- Limits, clipboard/target changes, modifier timeout, unsupported content or missing
  Accessibility must not paste original input.
- Cmd+Shift+R restores only the clipboard; explicit Cmd+Shift+V restores known
  markers and performs guarded paste. Preview/clipboard-only processing never paste.
- Forgetting clears views/rules and makes old markers unrestorable.

## M2 — Czech NER and finding review

Local model, worker, cancellation, separate detection/identity, editable findings
and explicit alias confirmation. Benchmark synthetic Czech email, contracts, logs
and customer notes: precision/recall per kind, cold/warm p50/p95, RAM/package size.
Real sensitive data may only be used locally by the user. Until quality is proven,
model mode is not general protection.

## M3 — everyday desktop app

Initial slice implemented: logo, .app icon, native macOS menu bar with shortcut/
Accessibility status, dashboard activation, shortcut toggle, clipboard-only restore,
permission settings and quit. Closing dashboard hides it while preserving entities
and session; menu/Dock reopen activate the same window. Without a menu bar, closing
the final window quits. Shortcuts do not enable automatically. See verification.md
for the limited native evidence.

Later slices implement Settings and scalar statistics below. Remaining ideas:
custom shortcuts, conflict handling, named sessions, configurable TTL. Encrypted
vaults need explicit design, Keychain/DPAPI, no plaintext fallback, atomic writes,
no cloud-synced mappings, encrypted import/export and permission-revocation/deletion
tests.

## M4 — platforms and distribution

Windows and X11 adapters, separate Wayland capabilities. Stable macOS bundle ID
and signing/notarization using the user's signing materials. Actual paste after
install/update because TCC binds executable/signature. Native keyboard/accessibility
checks required before claiming release readiness.

## M5 — optional extensions

Explicit opt-in AI target recognition and ordinary Cmd/Ctrl+V interception, without
silent fail-open. Requires permissions, suppression before detection, actual
protection status and per-target validation. Rich text, PDF/DOCX and OCR need
separate adapters and metadata tests, not a few extra clipboard regexes.

## Automated first-version checks

```sh
cargo fmt --all -- --check
cargo test -p copy-for-ai-core -p copy-for-ai-cli
cargo test -p copy-for-ai-desktop
cargo check -p copy-for-ai-desktop
cargo clippy --workspace --all-targets -- -D warnings
```

Native build/launch is a separate check. Core tests cover leaks of original matches,
stable mappings, single-pass restore, foreign/malformed markers, UTF-8, existing
markers, changed rules, identifier validity and input/mapping/output limits. GPUI
TestAppContext invokes the real view/handlers for editors, custom terms, processing,
restoration and two-step forgetting. It does not prove physical clicks or macOS
paste.

## Manual native acceptance

Use synthetic text only:

1. Launch GPUI; verify fonts, editors, clicking, scrolling and resizing.
2. Process synthetic email/phone/IBAN, copy and restore.
3. Add a custom name/company; verify stable markers and rule removal.
4. Try foreign markers; require warning, no guessing.
5. After Accessibility approval, guarded paste into local TextEdit, browser textarea
   and terminal. Require plain text and no Enter.
6. Change target/clipboard while waiting; paste must stop.
7. Restore a copied synthetic reply with Cmd+Shift+R; target field must not change.
   Test both directions of Cmd+Shift+V separately.
8. Forget the session; old markers become unknown.

Automated tests do not establish permissions, cross-process paste, signing or
distribution. Record completed checks in verification.md.

## Generic pipeline — implemented 2026-10-12

Multi-session in-memory vault, automatic direction, exact cross-session restoration,
external UTF-8 span API, UUID/locations/addresses, local verified GLiNER multi-v2.1
FP32, bounded overlapping windows and segmentation of unspaced scripts. Worker
handoff checks generation/cancellation and editor/clipboard/target changes. UI/CLI
share core. Model failure does not let original input through protected paste.
Smoke is not quality acceptance. Per-category/language precision/recall, editable
findings and explicit aliases remain. No disk mappings. Paste/signing/permissions
are separate evidence.

## Performance and long documents — 2026-10-12

Implementation and isolated before/after measurements completed;
[methodology/raw results](../benchmarks/README.md). Token-budget windows replace
640-character windows; words/labels are not repeatedly tokenized; byte-identical
windows reuse inference within a document. FP32, eight labels, threshold 0.4
unchanged. Batch two/CoreML measured; default 256 subwords / batch one / up to eight
threads was better. INT8 was not adopted due to detection differences.

Core uses one repeated-value search, indexed mappings and capacity prevalidation.
Detection/replacement/restoration run on workers. Accept private vault proposals
atomically after generation/input/revision/target checks; protected paste additionally
requires final clipboard/permission/modifier checks and successful native dispatch.
Stale/cancelled proposals never create live mappings; rejected owned text zeroizes.

All adapters: anonymization 1 MiB, output/restoration 32 MiB; CLI bounded reading
and whole-document --stdin; vault 16 sessions / 4096 mappings / 8 MiB originals.
Other cache/span/unknown-marker/deadline limits are explicit in benchmarks. Every
accepted window covers its original UTF-8 range, including tail. Any error rejects
the whole operation. Automatic direction, collision-checked short scopes and
single-pass restoration remain.

Verified: full 1 MiB through model, core, CLI and headless GPUI handoff. Repeated
100 KiB release detection 31.96× faster; debug cold startup 7.02×. Varied release
1 MiB still 308.4 s, dominated by inference. Short release 1 KiB regressed
192 → 317 ms. Headless debug editor publication remains 0.46–0.57 s; native
responsiveness not measured. Need general quality benchmark, accepted faster
model/runtime, long-input native UI and actual permission-enabled paste. Native
build passed; user's app was not restarted/changed. No distribution proof.

## Compact UI, history and bidirectional paste — 2026-10-12

One Process input action, load beside input and copy beside output. Progress/cancel
remain visible; lengthy repeated notices removed. Initial advanced options held
custom rules/session/clipboard-only actions and technical ID. History capped at
100 in-memory records: time/direction/outcome/duration/input bytes/replacements,
without text, markers, mappings or hashes. Forgetting clears active/history state.
Later navigation redesign moves configuration into Settings.

Cmd+Shift+V uses current clipboard and the same workflow as dashboard: unmarked
text requires model; known markers restore once and paste; unknown-only stops;
mixed unknowns remain/reported. Cmd+Shift+R is clipboard-only. Registration is
transactional, conflicts reported, ordinary Cmd+V untouched.

Original bug routed known markers to Purpose::Clipboard and limited paste
completion to Direction::Anonymize. Output editor was never the shortcut source.
Both directions now use Purpose::Protected and the same final checks. No fallback
on model failure. Mappings publish only after dispatch; dispatch alone is not
proof the target field received text. Native paste is verified separately.

## Settings, MIT and roadmap — 2026-10-12

Native General/Appearance/Applications/Updates/About, available through navigation,
Cmd+, menu bar; retain editors/work. Remove technical direction caption without
changing domain direction. System/light/dark and startup history persist as small
scalar preferences, without content/history. SMAppService reports actual approval
or rejection; login acceptance is separate from compilation.

Own code/workspace use MIT, root LICENSE and complete About text. Other licenses
remain separate. [Roadmap](roadmap.md) M4 covers trusted update feed/safe install;
M5 selected Zed/Delta/Codex/Claude desktop/terminal variants. UI marks these inactive
TODOs, never simulates updates or monitoring.

## Ink + Emerald and lifetime statistics — 2026-10-12

Three design alternatives under docs/design; selected Ink + Emerald implemented
directly in GPUI. Dark blue navigation, strong headings, emerald action, colored
status tags, light/dark palettes. Separate Workspace/History/Statistics/Settings;
configuration only in Settings, editors retained.

Scalar lifetime totals survive restart without document history: native pie,
legend, replacements, input volume, average and direction. Collection starts with
this version; demos are synthetic. Async serialized atomic writes, Forget reset,
normal-quit flush, no automatic overwrite of unreadable files, reported failures.
Ordinary preferences survive forgetting. Snapshot sequence numbers assigned at
request time prevent stale writes from undoing resets; targeted regression passed.

Historical checks: desktop 12/12, core 21/21, CLI 2/2, workspace Clippy, formatter,
native build passed. No new proof of physical shortcuts, permissions, login or paste.

## Local macOS release package — 2026-10-12

Optimized arm64 Release at target/release/Copy for AI.app, executable and verified
FP32/tokenizer. Multisize ICNS through 1024 px visually inspected; Resources copy
matches asset. Info.plist icon/executable references validated. Mach-O has no
nonsystem dynamic dependencies. Packaging ad-hoc signs the whole bundle, binding
Info.plist/Resources/icon; codesign --verify --deep --strict passed. No Developer ID
or notarization. Not installed/launched over the existing instance. No new Finder/
Dock icon, permission or actual paste proof. At that historical stage WIP was
preserved without commit/push.

## Long-clipboard audit and completion — 2026-10-12

[Audit](long-clipboard.md) separates model latency from guarded rejection. Existing
1 MiB inference 308,167 ms versus core 30 ms; overlap O(c log c), without a strict
whole-pipeline worst-case guarantee (regex iteration/hash lookup). Initial audit
was documentation-only, not implementation or new production speed evidence.

Subsequent work implements typed rejection reasons, scalar phase diagnostics,
nonblocking progress up to four publications/s, exact 8 MiB last-detection cache
with invalidation epoch/current vault/rules, explicit Process to clipboard and
O(log p) marker checks. Native failure clears only the unchanged owned result,
never restores original or publishes mappings. Adversarial samples did not justify
regex changes.

Isolated ORT 1.31 through identical rc.9 bindings: three alternating varied 100 KiB
pairs, median 19,902 → 8904 ms, reuse zero; 84 gold cases without new coverage loss.
Candidate 1 MiB 93,356 ms with exact restoration. Opt-in experiment, not deployed
bundle. [Evidence](../benchmarks/ort-runtime-2026-10-12.md). Isolated native preview
showed Workspace/History and short synthetic model work; TCC/actual paste and long
visual progress remain unverified.

## Four speed candidates and Apple screening — 2026-10-13

[Graph/QKV/FP16/Nym](../benchmarks/speed-techniques-2026-10-13.md): same varied
100 KiB reference 8800 ms; A/B/C 9741 / 10,523 / 15,918; Nym 5049 but 16 new coverage
losses and no EVENT. Reproducible scripts/corpus/Rust core bridge; A/B exact findings
on 93 documents; C no gold loss but two new benign core spans; Nym restoration
passed, quality failed. No production switch or 10-second goal.

[Apple](../benchmarks/apple-backends-2026-10-13.md): reconstructed original weights
for Torch/MPS/direct CoreML without changing assets. FP32 + original packed CPU
head preserves 14 tensor cases; B8 16 cases at 31.10 ms/window, insufficient. Full
FP16 loses 16 threshold spans; linear-only 21.64 ms/window loses three. These are
not gold/core or document timings. At this stage only MiniLM configs/card were
pinned, not weights/training; later work follows below.

## MiniLM first checkpoint and acceptance — 2026-10-13

Isolated verified public weights, synthetic TRAIN/DEV, FP32 MPS training, hash-checked
checkpoint, CoreML export, model benchmark and byte-span bridge to existing core.
Five categories including ADDRESS/EVENT, no second application engine.

Trained 12-layer MiniLM varied 1 MiB model-only 4.02 s including tokenization/decoding,
reuse zero, one token owner. Not complete anonymization: long core failed with
8820 unique model mappings versus capacity 4096. Template DEV after whitespace
repair: 840 TP / 0 FP / 0 FN without retraining. Frozen 93 documents not used for
learning/tuning; all exact Vault/Session restores passed. Final coverage 409/421
versus 385/421, but nine LOCATION losses/eight new benign spans reject deployment.
Keep 12 layers; improve LOCATION/negative-reference data and task-specific
distillation. Production/native permissions/paste unchanged/unverified by this
stage. [Report](../benchmarks/student-minilm-2026-10-13.md).

## MiniLM parallelism and batches — 2026-10-13

[Completed isolated experiments](../benchmarks/window-parallel-2026-10-13.md)
use per-worker models, bounded queue and ordered byte-span merging. B8 processes
1/2/4 workers: 4.958 / 5.917 / 6.792 s medians on 1 MiB. Single-model B16 4.094 s
versus matched B8 4.901 s, 16.47% saving, exact findings; B32 slower. Actual 50 MiB
B8/B16 one warm run each: 271.509 / 261.526 s, identical 996,940 spans, all 48,028
valid windows inferred, reuse zero.

Quality/core acceptance still fails. No production integration; GLiNER remains
sequential. Full 1 MiB / 10 s unmet. Rust probe confirms typed 50 MiB rejection
before NER; product limits remain 1 MiB / 32 MiB. No new UI/permission/paste proof.

## System One architecture screening — 2026-10-13

Laya/CLM repositories/cards and Jev API examined; [decision](anonymization-speed.md#system-one-laya-clm-and-jev).
No public contract is a direct token-NER replacement. Possible local candidate:
Laya/mmBERT encoder with newly trained BIO/span head. CLM requires full Qwen3-8B;
Jev is hosted with no public local weights found. No model download/inference,
new PII tests or production change. Speed/quality goals remain unmet.
