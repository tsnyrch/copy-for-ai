# Verification record

Records below distinguish implementation, automated tests, native UI, permissions
and actual system paste. Historical counts describe their respective versions,
not today's suite. Synthetic multilingual data and byte-exact raw/source archives
remain in their original languages. Historical screenshots show the UI of the
build captured, not evidence of the current English interface.

## Local landing preparation and review — 2026-10-29

Scope: the Signal Orange design, embedded font/license, Settings crash repair,
responsive navigation regression and accompanying synthetic screenshots/plans,
plus the new generic `.agents/skills/land/SKILL.md` workflow.

Inspected the complete pending application diff, new test/helper, font
provenance/license and documentation. Review found no additional blocking
correctness, privacy or integration issue. No application source changed during
this landing preparation; earlier automated/native results below remain
implementation evidence, not new test runs.

Fresh local checks:

- `cargo fmt --all -- --check`: passed. Exact formatter invocation is defined
  in `.github/workflows/ci.yml`; toolchain and style are defined in
  `rust-toolchain.toml` and `rustfmt.toml`.
- Diff whitespace check: passed.
- Bundled Roboto SHA-256 matches its recorded provenance.
- Land skill YAML, invocation restriction and Land metadata were validated.

Under the user's explicit local-first policy, no application tests, build,
remote CI, fetch or push were run during this landing preparation. Existing
test and native evidence is preserved below; permissions and actual system
paste were not newly verified. The configured publication remote is
`https://github.com/tsnyrch/copy-for-ai.git`, with `main` as the intended branch.
Any push requires separate user confirmation. Local remote-tracking refs were
not treated as proof of the live remote state.

## Settings crash and responsive-layout regression — 2026-10-29

The user reported a reproducible `hover style already set` panic on opening
Settings. The earlier migration checks did not cover painting Settings after
the final hover adjustment. The new rendered navigation test reproduced that
exact panic before the fix: GPUI Kit's Button already registers its own hover
style, so a second element-level `.hover(...)` conflicts. Selected Settings
navigation now configures its existing `ButtonCustomVariant` instead.

The resize test also found History expanding the entire compact page to 760px
inside a 640px viewport (scroll-region origin x = -60px). The main compact content
now has an explicit full-window width; History's table can exceed that width only
inside its local scroll region. The shell declares its keyboard tab group.
The accepted Signal Orange appearance and processing/permission logic are unchanged.

`tests/ui/navigation.rs` paints real GPUI components and dispatches pointer,
keyboard and scroll events using synthetic, temporary preview state. It checks:

- All four main navigation items and all seven Settings sections at widths
  **640, 768, 959, 960, 1024, 1120 and 1440px**, with a 700px content height,
  in both Light and Dark modes.
- Settings changes without panic, preservation of retained editor entities,
  their synthetic text and an ongoing busy state.
- Tab traversal (after initial focus) and Enter activation of Appearance.
- Vertical scroll reaching the last General row and horizontal History scroll
  revealing the final column.
- On-screen navigation, footer, editors and process/cancel controls; nonoverlapping
  editors; usable editor/history height; chart and legend containment.

Checks run:

- `cargo fmt --all -- --check`: passed.
- `cargo test -p copy-for-ai-desktop`: **20 passed, one real-model test ignored**.
- After extending the scroll assertions, the focused
  `cargo test -p copy-for-ai-desktop settings_navigation_and_resize_preserve_editors -- --nocapture`
  passed.
- `cargo clippy -p copy-for-ai-desktop --all-targets -- -D warnings`: passed.
- `cargo build -p copy-for-ai-desktop`: passed.
- Core/CLI/model suites and benchmarks were not rerun for these UI-only repairs.
  The unrelated upstream `block` 0.1.6 future-compatibility notice remains.

Native checks: ran the exact `cargo run` command and separately
`cargo run -- --settings-preview`. Both created native windows and remained
running without panic during bounded six-second checks. Only the owned process
groups were stopped afterward. The default launch read ordinary preferences;
no processing, clipboard operation or user settings mutation was invoked.

Added `--compact-preview`, effective only with an existing debug preview mode,
to open a 640 × 700 content-area window without Accessibility automation.
Native light-theme visual inspection completed for:

- [Settings, 1120 × 820](../evidence/signal-orange-settings-desktop-2026-10-29.png).
- [Settings, 640 × 700](../evidence/signal-orange-settings-compact-2026-10-29.png).
- [Workspace, 640 × 700](../evidence/signal-orange-workspace-compact-2026-10-29.png).

These are native screenshots of isolated previews with temporary preferences,
disabled shortcuts and synthetic data, not a mockup or inference-quality proof.
The short Settings viewport intentionally scrolls: its bottom row's scroll
reachability is verified by the headless wheel-event regression.

In this follow-up, screen-capture preflight returned true, but `screencapture -l`
still failed. ScreenCaptureKit with AppKit initialization captured only the
owned, actual application window (excluding GPUI's hidden helper window).
`AXIsProcessTrusted()` remained false. No Accessibility permission was requested
or granted. Physical keyboard/clicks/resize, dark native screenshots, VoiceOver,
login registration and actual system paste remain unverified. No user clipboard,
existing app instance or System Settings were controlled.

## xsnyrch Signal Orange design migration — 2026-10-29

Implementation: native GPUI light palette from the selected Signal Orange seed,
application-derived dark companion, one semantic palette for custom surfaces and
Kit tokens, embedded Roboto, shared control geometry and responsive composition
across Workspace, History, Statistics and Settings. Original identity assets and
processing/clipboard/permission logic are unchanged. The source specification
and derived colors are separated in the [design contract](../design/signal-orange.md).
Codebase-memory MCP was unavailable in this turn; file and local dependency
source inspection were used instead. No structural graph reindex was available.

| Automated check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed. |
| `cargo test -p copy-for-ai-core -p copy-for-ai-cli` | Passed: 22 core and two CLI tests. |
| `cargo test -p copy-for-ai-desktop` | Passed: 19 tests; one opt-in real-model test ignored. |
| `cargo check -p copy-for-ai-desktop` | Passed. |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed. |
| `cargo build -p copy-for-ai-desktop` | Passed: native arm64 macOS debug executable. |

The broad checks above precede the final selected-navigation hover contrast
adjustment. After that adjustment, `cargo fmt --all -- --check`,
`cargo clippy -p copy-for-ai-desktop --all-targets -- -D warnings`,
`cargo build -p copy-for-ai-desktop` and `git diff --check` passed. Behavioral
suites were not repeated for that hover-only change. The existing upstream
`block` 0.1.6 future-compatibility notice remains; application lint warnings were
not suppressed. No model tests or benchmarks were added/run for this visual change.

Font: downloaded the unmodified, pinned google/fonts Roboto variable TTF and
verified SHA-256 against the recorded value. CoreText reads family Roboto, weight
axis 100–900 and width axis 75–100; the synthetic Latin/Czech sample has glyph
coverage. Font and full OFL notice are embedded; no font installation or runtime
network requirement.

Exact opaque sRGB checks:

- Ink on orange action: **5.88:1**; hover **5.30:1**; pressed **4.83:1**.
- Light secondary text on white / warm canvas: **6.08 / 5.53:1**.
- Light accent text on accent wash: **4.72:1**; dark: **8.42:1**.
- Light control border on white / warm canvas: **3.84 / 3.49:1**.
- Light focus on white / warm canvas: **5.25 / 4.77:1**.
- Dark control border on surface / canvas: **4.53 / 5.10:1**;
  dark focus: **9.59 / 10.79:1**.
- Success/warning/danger labels on their 10% status washes: light
  **6.16 / 5.75 / 5.52:1**; dark **8.13 / 8.75 / 7.19:1**.

The matrix also checked main/secondary text on both surfaces and neutral hover
in both modes. It found accent text on neutral hover at 4.36:1, so selected
navigation retains its accent wash on hover. These are calculated color pairs,
not a rendered-component/disabled-state or WCAG audit.

Native launch: ran only the newly built executable with `--ui-preview`, temporary
preferences/totals, synthetic input/history and disabled shortcuts. macOS reported
its 1120px window and the process remained running without startup errors. Window-
only `screencapture` failed (`could not create image from window`);
`AXIsProcessTrusted()` returned false. Closed only that owned preview afterward.
No screenshot or visual confirmation is available. The user's production instance,
clipboard, mappings, permissions and System Settings were not changed.

Not verified live: light/dark visual states, 640px and desktop reflow, long text,
200% zoom, Tab/Enter/Space, focus visibility, screen-reader announcements and
actual system paste. No Accessibility permission request/grant, login registration,
release packaging, install, signing or notarization. Headless tests and native
launch do not prove native interaction, cross-process paste or anonymity.

## English repository preparation — 2026-10-28

Implemented English UI, CLI, domain/model/platform errors, preview text, README,
agent rules, plans and benchmark documentation. Multilingual detection/UTF-8
fixtures were deliberately preserved. No detection algorithm, model weights or
runtime default was changed as part of the language conversion. Existing local
performance/cache changes and experimental evidence were retained for publication.

Repository checks: MIT root license matches workspace and inherited crate licenses.
GPUI Kit 0.7.3, gpui-pre 0.3.9 and gline-rs 1.1.0 locally resolved manifests declare
Apache-2.0. THIRD_PARTY.md documents the separate model/dependency licensing boundary;
a locked native macOS metadata check resolved 562 packages with declared licenses,
including MPL-2.0 cbindgen/option-ext and CDLA-Permissive-2.0 webpki-roots. This
is not a complete binary-distribution notice bundle. An initial unfiltered offline
metadata request needed an uncached Linux dependency; the native-filtered request
passed. Cargo
metadata now identifies the GitHub repository and project description.
.gitignore excludes nested targets, model weights/exports, Python caches/environments,
local environment secrets and temporary files; Cargo.lock and deliberate evidence
remain versioned.

| Current local check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed after applying rustfmt to translated long lines. |
| `cargo test --locked -p copy-for-ai-core -p copy-for-ai-cli` | Passed: 22 core and two CLI tests. |
| `cargo test --locked -p copy-for-ai-desktop` | Passed: 19 tests; one opt-in real-model test ignored. Relevant preview restoration test also passed after the final fixture translation. |
| `cargo check --locked -p copy-for-ai-desktop` | Passed. |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | Passed. |

All relative Markdown file links resolve. Python AST parsing passed for 39 source
files, including experiment snapshots; package-macos.sh passed bash -n. Staged
source/documentation whitespace checks passed excluding four byte-exact upstream
model-card/raw-test artifacts that retain their original whitespace. No weights,
build directories, Python caches or local environment secrets were staged.

No new model benchmark, native UI run, Accessibility approval, login registration,
release package or real system paste was performed for this repository preparation.
The upstream block 0.1.6 future-incompatibility notice is separate from application
Clippy warnings. The requested publication target is `origin/main` at
https://github.com/tsnyrch/copy-for-ai.git. A live pre-push remote check returned no
refs. Publication success is reported only after the actual push.

## Initial implementation — 2026-10-12

Environment: macOS, Rust 1.99.0, Xcode /Applications/Xcode.app/Contents/Developer.
M0 complete; M1 locally implemented, physical paste unverified; M2–M5 not fully
accepted. At this stage the repository was initialized on main without commit,
reset or push. Formatter, EditorConfig, toolchain, workspace lints, CI and agent
rules created; CI inspected locally, GitHub jobs not run. Modules follow
project-structure.md. GPUI Kit 0.7.3, in-memory workflow, regex/custom terms,
stable markers, restoration/forgetting, macOS adapter and transactional explicit
shortcuts implemented. Development bundle at target/debug/Copy for AI.app,
bundle ID local.copyforai.app, without Developer ID/notarization.

| Historical check | Result |
| --- | --- |
| cargo fmt --all -- --check | Passed. |
| cargo test --workspace --locked --offline | Ten core tests + one GPUI view test passed. |
| cargo check -p copy-for-ai-desktop --offline | Passed. |
| cargo clippy --workspace --all-targets --locked --offline -- -D warnings | Passed. |
| cargo build -p copy-for-ai-desktop --offline | Native executable built. |
| bash scripts/package-macos.sh | Development .app built. |

Core covered Czech UTF-8, exact round trip, stable identity across calls, whole
terms/overlap, idempotence, secrets enclosing markers, foreign/broken markers,
IBAN/Czech identifier validity, input/mapping limits and expansion bounds. GPUI
TestAppContext rendered the actual view/entities and called production handlers
for terms, processing, restoration and two-step forgetting. It did not establish
physical keys/clicks or paste into another process. block 0.1.6 emitted an upstream
future-Rust-compatibility notice; application warnings were not suppressed.

### Initial native UI

Development .app opened through Computer Use. Initial access was denied; after
explicit user instruction, only Copy for AI's window was accessible. No other apps
or System Settings were controlled. Observed fonts/buttons/editors/session/status,
adding a synthetic PERSON term, and actual Accessibility state as denied.

The user was working in editors; their content was not overwritten or recorded.
No controlled native full round trip. Later capture/no-window errors left removal
of the synthetic rule unconfirmed. Long text visibly escaped editor bounds;
overflow_hidden was added on panel/content containers. GPUI/lints passed after
repair, but no subsequent native visual confirmation of that fix.

## Logo and menu bar — 2026-10-12

Two-sheet/shield SVG, GPUI PNG, template status PNG and multisize CopyAI.icns
embedded in executable; runtime needs no source checkout. Regenerate:

```sh
swift -module-cache-path target/swift-module-cache scripts/generate-icons.swift "$PWD"
```

Retained NSStatusItem, menu target/action queue implement dashboard, shortcut toggle,
clipboard-only restore, Accessibility settings and quit. Status distinguishes off,
permission missing, ready and processing. Shortcut activation explicit; no automatic
permission grant. Closing dashboard with status item hides the same view/session;
menu/Dock reopen activate it. Quit cancels work and exits.

Formatter, core/CLI, two GPUI desktop tests, desktop check, workspace Clippy and
native build/package passed. An intentionally reversed test span uses explicit
Range to preserve the negative case without lint suppression. sips read ICNS as
1024 × 1024. Computer Use confirmed the new dashboard logo. AX did not expose the
system status item; menu clicks/state/reopen and hide/session preservation were
not conclusively verified. User editor content was not changed/recorded. No
permission grant, actual paste or completion of all M3.

## Generic multilingual pipeline and short scopes — 2026-10-12

Generic UTF-8 without format parsing/language switch; local GLiNER complements
patterns. Scope eight hex characters, full UUID internal; vault collision-checks
retained scopes. Repeated exact surfaces reuse markers. Locally installed FP32/
tokenizer verified against pinned SHA-256. No runtime downloads/network inference.
Model errors block anonymization; known-marker restore needs no model.

Passed on macOS:

- Formatter; core/CLI locked offline: 16 core (six generic, three limits, seven
  workflow); CLI compiled, no standalone CLI tests at that stage.
- Two headless desktop tests: old-session restoration/forgetting and rejection of
  cancelled/late results or changed input without mappings.
- Desktop check and workspace Clippy; upstream block notice remains.
- Four ignored real-model multilingual tests: 52.24 s concurrently, not app latency.
  Exact expected names in seven languages and byte-for-byte restoration. Synthetic
  serialized business record, CSV notes, short first names, event/places, beyond-first-
  window detection and cancellation; no user's original record.
- Development package built with verified model/tokenizer in Resources, without
  Developer ID/notarization.

Native observation within the authorized Copy for AI window showed automatic
workflow, detection in progress and later restoration. No user text overwritten
or stored. This was not controlled synthetic end-to-end acceptance; final category/
scope changes were verified by model/core/build, without restarting the user's
session. Accessibility was denied; no cross-app protected paste was established.

Seven-language smoke is not universal coverage. Precision/recall, editable findings
and aliases remain. Model/rules may miss or over-select. Unknown-only markers do
not automatically copy/paste. Mappings are memory-only and lost on exit. Zeroizing
owned originals does not erase upstream model/GPUI/OS copies.

## 1 MiB performance and reliability — 2026-10-12

M5, 10 CPU cores, 32 GiB, arm64, macOS 27.0.1 (26A434), Rust 1.99.0 Homebrew.
[Method, raw logs, source and capacities](../benchmarks/README.md).
All measurements used isolated synthetic processes.

| Historical check | Result |
| --- | --- |
| Formatter | Passed. |
| Core/CLI tests | 21 + two passed. |
| Desktop tests | Six passed, three headless GPUI. |
| Desktop check / workspace Clippy | Passed. |
| NER library tests | UTF-8 window coverage/overlap passed. |
| Ignored real-model multilingual tests | Six FP32 tests passed in 10.85 s concurrently, not app latency. |
| Native desktop build | Built, without launch/restart. |

Checks also used --offline. Raw final summaries are in benchmarks. Regressions
cover 1 MiB/tail in Latin/Czech/Cyrillic/Japanese/Chinese, byte offsets/seams,
batch two, repeated entities, exact restoration, expanded restore above old input
limit, global mappings/originals capacity, unknown markers and cancellation before
mutation. Desktop covers stale revision, forgetting during work, cancellation
without model load and full proposal acceptance. Not general precision/recall.
Same FP32/checksums/labels/threshold; changed context can change findings.

Repeated warm release: 1 KiB 192 → 317 ms; 10 KiB 2032 → 1306; 100 KiB
22,530 → 705 (31.96×); 256 KiB 76,665 → 734. Original 256 KiB restoration exceeded
262,144-byte limit. Original 1 MiB rejected before inference; new repeated 1 MiB
723 ms, all 1121 windows covered/restored.

Varied release 1 MiB: 308,388 ms, all 1143 inferred, reuse zero, 26,471 spans,
3400 bytes/s; core 30 ms, restoration 3 ms exact. Debug complete sample 325,366 ms;
another isolated sample manually interrupted, never counted as partial success.
CLI independently accepted all 1 MiB, exit zero, 1,401,477 output bytes.

Debug cold checksum/load/first detection 39,968 → 5692 ms (7.02×), checksum
38,110 → 2979 (12.79×), no verification skipped. No meaningful release checksum
gain. N=1/2, not p50/p95. Repeated text gains strongly; varied input remains model-
bound, short latency worsened. Peak RSS release varied 3.10 GiB, after maximum
3.32 GiB debug. Cancellation requested at 250 ms returned at 304 ms, not a hard
bound; synchronous ORT load/run cannot stop midway. Cancelled proposals rejected.

CoreML NN/CPUAndGPU MLProgram ~3× slower than CPU on varied 10 KiB; profile shows
split graph. Corrected static CPU passed; GPU MPSGraph shape abort SIGABRT 134;
static production path removed. INT8 lost 6340 FP32 spans on 40 cases, rejected.

Limits: anonymization 1 MiB; marked/restoration input/output 32 MiB; vault 16
sessions / 4096 mappings / 8 MiB originals; 256 subwords, batch one, eight threads,
15-minute deadline. Capacity/model errors reject whole results; atomic mappings,
no truncation/regex-only fallback.

Real headless finish_detection with synthetic span: 1 MiB editor publication
0.46–0.57 s, microsecond queue handoff. Detection/core off UI; editor synchronization
cost remains. Native long-document UI, Accessibility and actual paste not newly
proved. No user app restart, other-app or System Settings control.

## Compact UI, history and bidirectional paste — 2026-10-12

One primary action, editor-local actions, collapsible advanced controls/history.
History tests verify 100-record bound, completed/cancelled/failed states and clearing
including active entry. Bug: protected_paste routed known markers to Clipboard;
finish_detection only allowed anonymization paste. Current clipboard, not output
editor, is the source. Both directions now use guarded completion.

Headless handler regression uses test clipboard and stale output editor. Known
markers restore to test OS adapter without model/rescanning, mixed unknowns remain/
reported, unknown-only does not paste; editor unchanged. Final-paste regression
rejects cancellation/late generation, changed PID/clipboard/Accessibility, held
modifiers, vault revision and native failure, without mappings/paste. Model failure
has no regex fallback. Test adapter is not system paste.

Nine desktop tests passed; after adding explicit model failure only relevant
final_paste_rejects was rerun successfully. Core 21 + CLI two, formatter, workspace
Clippy, desktop check and native build passed. Upstream block notice remains.

A separate temporary Copy for AI UI Preview with another bundle ID, empty editors
and disabled shortcuts showed Workspace and expanding/collapsing advanced/history.
[Historical screenshot](../evidence/clean-ui-2026-10-12.png) has no user data.
Original app/editors untouched. Debug --ui-preview supports synthetic demos; this
screenshot was an empty instance, not model inference proof. Accessibility missing;
no permission changes or other-app/System Settings control.

Actual global registration/conflict, physical shortcut press and field receipt of
restored/anonymized text remain unverified. Code/docs use Cmd+Shift+V/R, rollback
second-registration conflict, no ordinary Cmd+V interception. Native boundary
rechecks Accessibility/PID/changeCount/modifiers; sending Cmd+V does not prove field
receipt. Remaining acceptance: both directions, unknown/mixed markers, clipboard-
only R, conflicts and changed state using authorized synthetic native targets.

Graph initially indexed as Users-tsnyrch-my-projects-copy-for-ai. MCP reindex twice
returned Transport closed; local CLI succeeded with 849 nodes/2250 edges. No changes
to another worker's inference/models/benchmarks in that UI slice.

## Settings, appearance and MIT — 2026-10-12

Native sections General/Appearance/Applications/Updates/About; navigation, Cmd+,
menu bar; technical direction caption removed without domain change. Scalar
appearance/startup-history preferences, no document/history persistence.
SMAppService checks .app packaging and returns OS state/rejection. MIT workspace,
root LICENSE/full About text; model/dependency licenses separate.

Ten desktop tests passed, including scalar-only/unknown-field settings validation.
After centralizing Settings action, one new GPUI preservation test passed for
input/output/busy/generation without editor focus (11 tests total then; old ten
not unnecessarily repeated). Formatter passed. Workspace Clippy found a missing
SAFETY comment in a second Objective-C block; fixed, desktop Clippy passed.
Native build passed. No model/NER changes or expensive benchmarks.

Isolated Settings Preview used another bundle ID, temporary preferences and disabled
login toggle. Observed light→dark, all sections/full MIT and return to empty editors;
Cmd+, worked from focused input. User app not changed/restarted. Evidence:
[light](../evidence/settings-light-2026-10-12.png),
[dark](../evidence/settings-dark-2026-10-12.png),
[MIT](../evidence/settings-about-2026-10-12.png),
[applications](../evidence/settings-applications-2026-10-12.png).

Not verified: actual installed SMAppService registration/unregistration, approval,
next-login start, or system appearance change through System Settings. Observer
subscription implemented. Updates/app detection remain inactive TODOs, not active
monitoring. No physical shortcut/paste evidence. Local graph CLI reindex used
because MCP remained closed.

## Ink + Emerald and lifetime statistics — 2026-10-12

Desktop 12/12 passed: totals beyond history cap, exactly-once counting, restart,
reset, invalid/oversize files, existing settings/paste tests. Core 21 + CLI two,
formatter, workspace Clippy and native build passed; no new inference benchmarks.

Isolated Final Preview with temporary preferences/totals showed Workspace, History
status tags, native pie Statistics, compact Settings and dark theme. Demo counts
one completed, one cancelled and one failed synthetic operation, not user usage or
quality. Navigation retains editors. Screenshots:
[workspace](../evidence/redesign-workspace-2026-10-12.png),
[history](../evidence/redesign-history-2026-10-12.png),
[statistics](../evidence/redesign-statistics-2026-10-12.png),
[settings](../evidence/redesign-settings-2026-10-12.png),
[dark chart](../evidence/redesign-statistics-dark-2026-10-12.png).
History/light chart precede final primary-button/subtab styling; their own render
was unchanged. Other captures use final native build.

Storage tests establish restart/reset, while preview does not persist. No OS-crash
or two-production-instance durability proof. Single process owner, atomic file and
serialized writes; forced exit may lose latest operation. Actual registration,
physical shortcuts, Accessibility, SMAppService and paste still unverified. User app,
System Settings and other apps untouched. Graph reindexed through local CLI.

A later targeted regression passed reversed execution: an old snapshot requested
before reset but executed after cannot overwrite zero totals. Assign sequence
numbers at request time; mutex protects write. Desktop Clippy passed. Screenshots
precede this storage-only guard; rendering unchanged.

## Local macOS release package — 2026-10-12

Optimized arm64 target/release/Copy for AI.app includes executable and pinned/hash-
verified FP32/tokenizer. Multisize ICNS through 1024 px visually checked and identical
in Resources; Info.plist references/plist validation passed. Mach-O has no nonsystem
dynamic dependencies. Ad-hoc full-bundle signing binds plist/resources/icon;
codesign --verify --deep --strict passed. No Developer ID/notarization. Not
launched/installed over user's instance. No new Finder/Dock icon/permissions/paste
proof. At this historical stage WIP preserved without commit/push.

## Long-clipboard audit — 2026-10-12

Reviewed current core/NER/workflow, pinned gline-rs decoder, raw benchmarks and
five competitors' public source/docs. [Findings](long-clipboard.md) distinguish
expected/worst-case complexity and historical/new evidence. Initial audit changed
docs only, without rerunning build/model tests or proving native UI/permissions/
paste. Production unchanged.

## Long clipboards and ORT probe — 2026-10-12

Formatter, desktop check, workspace Clippy passed; targeted desktop Clippy after
final edits. Core 22 / CLI two passed including marker-intersection regression.
Desktop 19 passed / one explicit model test ignored; then that real FP32 cache/
cancellation test passed separately in 4.52 s. Small cached service result 355 µs,
inferred zero / reused one, not UI/long latency. NER library 2/2 including progress
throttling and token coverage. Native debug executable built.

Isolated temporary CUA preview showed new action, diagnostic history and completed
short synthetic model input; only owned preview closed afterward. Final executable
rebuilt after final text changes. No production .app/user clipboard changes.

ORT 1.31 ARM64 archive/library checksums verified. Three alternating varied 100 KiB
pairs with reuse zero: median 19,902 → 8904 ms. 84 gold cases no new detection/
coverage losses, all exact restores; both still 13 missing exact / 11 uncovered.
One complete candidate 1 MiB: 93,356 ms, 1143 inferences, reuse zero, exact restore,
RSS 3,337,584,640 bytes; cancellation 315 ms. Own compilation/Clippy partly overlapped;
historical 308,388 ms is not a matched pair/UI benchmark. Adversarial regex: twelve
size/scenario combinations × three samples, exact restore; no evidence requiring
pattern change or proving universal worst-case bounds.

[Report/hashes/logs](../benchmarks/ort-runtime-2026-10-12.md). Runtime remains opt-in;
production FP32/default unchanged. Metadata adds no disk history; absent worker
report means unavailable diagnostics, not measured zero. Not verified: installed
TCC/Accessibility, actual paste, long native visual progress. Headless/model gates
prove neither system paste nor complete anonymization.

## Four new speed candidates — 2026-10-13

Formatter, NER all-targets Clippy, Python syntax/lightweight Unicode/BIO/groups
passed. Isolated release ORT 1.31 same varied 100 KiB, batch one/256/eight threads,
111 windows, reuse zero: reference/A/B/C 8800 / 9741 / 10,523 / 15,918 ms, real core
replacement/exact restoration every pass.

93 documents/421 gold, seven languages plus complete multilingual-tail 1 MiB:
A/B unchanged final spans; C no new gold loss but NER 1417 missing/28 added, core
76/45; one new benign NER/two core spans reject quality. Baseline/A/B/C still
36 uncovered in both stages. Negative gates compare exact new pairs, not counts:
C core benign 30→30 but two new findings.

Nym FP32 Python ORT 1.24.4: 100 KiB 5049 ms, 52 windows/reuse zero, RSS
1,590,296,576 bytes; 16 coverage losses/six gains, total 46 uncovered, no EVENT.
Same Rust core validated all ranges and 93 restores plus 100 KiB. Repeated-fill
1 MiB 45,987 ms/all 485 inferred proves coverage/restore, not varied speed. Failed
small gates stopped further long speed runs. No candidate accepted; 10-second goal
open. No production/user clipboard/permission changes, no repeated desktop/native
paste suites. [Evidence](../benchmarks/speed-techniques-2026-10-13.md).

## Apple backend and actual batches — 2026-10-13

Strict reconstruction: no missing/unused weights, correct LSTM gates/position IDs.
Torch CPU, MPS without fallback, GPU encoder/CPU head and direct FP32 CoreML retain
all valid threshold spans on 14 tensors. Static CoreML export/runtime works after
narrow sqrt promotion/fixed self-attention shape fixes. Preserve packed LSTM;
ONNX CPU tail checker/parity passed with no encoder compute remaining.

B8 encoder/batched CPU tail: 16 valid windows, five warm cycles, 31.10 ms/window,
no span changes, separate reference/reuse zero/spinning off. Tensor throughput,
not document latency. FP16/ALL loses 16/adds two; linear-only loses three/adds zero,
21.64 ms/window. MIL dtypes/expected hardware plan checked, no runtime hardware
trace. Threshold losses are not gold uncovered counts. Reject production switching.
Python syntax/export/load/stated inference checks passed; no 93-document gold/core,
100 KiB/1 MiB, Rust/desktop/native/permission/paste suites. At this stage only student
config/card checked; later checkpoint is below. [Report](../benchmarks/apple-backends-2026-10-13.md).

## Trained MiniLM FP32 — 2026-10-13

Verified public weights/revision/hash, strict pretrained/checkpoint BIO11/layers/
FP32 and frozen embeddings/pooler against original. Model/backend/CPU fallback off;
production unchanged. Real MPS training 200 steps/55.29 s; all 12 layers/head finite
nonzero gradients. TRAIN 748/756 gold, eight unrepresentable boundaries/84 review
excluded. Same-checkpoint repaired DEV: 252 exact gold / 840 TP / zero FP/FN,
28 review retained in denominator; original hashes unchanged. Templates do not
establish general quality. Three decoder regressions and seven AST checks passed.
No Rust app changes or repeated broad native suites in this stage.

CoreML first batch per 93 documents: owned-token max delta 3.93987e-5, zero argmax
changes, tolerance 1e-4; padding/specials/dummy/unowned overlaps diagnostic separately.
All positions finite/shape-valid and ownership complete. Earlier all-position gate
failed on padding only; retained diagnostic. Not every-batch 1 MiB CPU parity.

Frozen 421 gold: NER/final core baseline 36 uncovered, student 12; nine losses,
33 gains, new benign NER nine/core eight. Gate false. Existing release speed_probe
confirmed counts and both exact core round trips on all 93 documents. Actual trained
varied 1 MiB warm median 4020.81 ms, 211,003 tokens / 951 valid / 952 computed,
reuse zero, one owner. Core failed before restoration; 8820 unique mappings exceed
4096 default (likely VaultFull, unconfirmed type). 8814 unique Person values contain
digits. No accepted checkpoint or completed 10-second goal. [Report](../benchmarks/student-minilm-2026-10-13.md).
Large weights/checkpoints/packages remained temporary. No UI/permissions/clipboard/
OS paste; graph/reindex Transport closed.

## Parallel windows and 50 MiB — 2026-10-13

[Experiments](../benchmarks/window-parallel-2026-10-13.md): B8 processes 1/2/4 workers,
one process B16/B32, one model B8/B16 without IPC. Seven variants × three warm
1 MiB runs. Process B8 medians 4.958 / 5.917 / 6.792 s; no-IPC B16 4.094 versus
B8 4.901 s. All have 211,003 tokens, 951 valid windows, 20,456 identical ordered
spans/kinds/digest matching prior reference. B8 computes 952 rows, B16/B32 960;
reuse zero, one token owner, complete tail and finite FP32.

Both actual 50 MiB runs completed 10,662,245 tokens / 48,028 valid / 48,032 computed
windows, identical 996,940 spans/digest. B8/B16 271.509 / 261.526 s including tokens/
decoding; one warm run each, no p95. comparison.json checks all variants. Sampled
RSS 5.231 / 2.276 GiB; process maxima 6.949 / 5.031. 16 GiB guard is not a hard
allocation limit or full GPU/unified memory measurement; differences do not prove
batching memory savings.

Four AST checks, tokenizer/ownership UTF-8/B8/16/32 parity and helper source hash
passed. Independent read-only review found no result-changing issue. Cleanup smoke
used synthetic subprocess tasks, not CoreML hangs; running native thread cannot
be safely interrupted. Host predict overlap is not GPU-kernel concurrency proof.
Rust current-core probe confirms Error::InputTooLarge from Vault.direction for
52,428,800 UTF-8 bytes; NER not called/dependency. Desktop-before-load rejection is
source audit, not new native UI. No quality/core acceptance repair, 50 MiB core
anonymization/restoration or permission/paste proof. Production limits/model remain;
full 1 MiB / 10 s unmet.

## System One evidence boundary — 2026-10-13

[Primary-source evidence](../benchmarks/system-one-screening-2026-10-13.json) checks
Laya/CLM public architecture/contracts and Jev documented API. Laya option logits,
CLM pooling/candidate scores are not all-token BIO. Jev full network/internal pass
count publicly unverified. Public model-card availability is not download/hash
verification. No new M5 timing, 1/50 MiB inference, quality gate, Vault/Session or
native/paste proof. New head/encoder requires new acceptance. Production unchanged.

## Remaining native acceptance

- Grant Accessibility to the installed Copy for AI app and verify guarded paste
  into explicit synthetic targets (own field, TextEdit, browser textarea, terminal).
- Physically verify target/clipboard change rejection, held-modifier timeout and
  permission revocation while waiting.
- Cmd+Shift+R clipboard-only and both Cmd+Shift+V directions; preview/standalone
  processing must never paste. Check registration conflicts and mixed/unknown markers.
- Native clipping of long text and small-window layout.
- General quality benchmark, Windows/Linux native adapters, signing/notarization
  and separately designed encrypted persistence.

These gaps do not prevent local preview/copy. They preclude claims of complete
anonymization or public-distribution readiness.
