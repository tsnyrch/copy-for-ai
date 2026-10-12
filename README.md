# Copy for AI

A local Rust app for reversible text pseudonymization before sharing with AI.
The native interface uses Zed GPUI and GPUI Kit. Text is processed locally by
multilingual GLiNER and identifier rules. Detection can miss sensitive information;
review the result before sharing. This is reversible pseudonymization, not a
guarantee of complete anonymity.

## Run

The primary supported platform is macOS. GPUI/Metal requires Xcode and its
command-line tools. Use the stable Rust toolchain configured in this repository.

```sh
python3 scripts/install-model.py
cargo desktop
```

Build a local `.app` with a stable bundle ID:

```sh
bash scripts/package-macos.sh
open "target/debug/Copy for AI.app"
```

The bundle is ad-hoc signed for local use. It has no Developer ID signature or
notarization. Add `--release` to the packaging command for an optimized build.

For an interactive terminal session using the same engine:

```sh
cargo run -p copy-for-ai-cli
```

Use `-- --stdin` to process an entire document rather than one line at a time.

## Workflow

- **Process input** prepares a preview; **Copy** copies the result as plain text.
- **Process clipboard** applies the same automatic operation to the clipboard.
- **Cmd+Shift+V** reads the current clipboard, anonymizes text without markers or
  restores known local markers, then pastes into the original target after safety
  checks. Unknown-only markers stop paste; mixed unknown markers are preserved
  and reported. Restored originals are never scanned again.
- **Cmd+Shift+R** restores the clipboard only. Ordinary Cmd+V is not intercepted.
- Enable global shortcuts explicitly. Protected paste requires macOS Accessibility
  permission, an unchanged target and clipboard, and released modifier keys.
- **New session** keeps older mappings in this process. **Forget everything**
  clears sessions, custom terms, editors, history and lifetime statistics.

Originals and mappings stay in process memory. Quitting or forgetting the session
removes the ability to restore old replies. Other local processes, clipboard
managers and OS clipboard synchronization may still access clipboard content.
Zeroizing owned strings does not prove that all OS, swap or editor copies are gone.

The pinned FP32 model (~1.16 GB, Apache-2.0) is installed separately and verified
using SHA-256. Set `COPY_FOR_AI_MODEL_DIR` for a custom location. Processing does
not upload user text or download models. Missing or failed models stop
anonymization; restoration of known markers still works.

Anonymization accepts up to 1 MiB; restoration and output accept up to 32 MiB.
Long, varied documents can take minutes with the production model. Experimental
speed results do not establish production quality or a complete 10-second workflow.
See the [processing contract](docs/plan/generic-processing.md).

## Settings and data

Open Settings using the navigation, Cmd+, or the menu bar. Settings include
launch at login, system/light/dark appearance, custom terms, sessions and the
full MIT license. Appearance and the startup history preference are saved locally.
Launch-at-login uses macOS SMAppService and requires an installed app bundle.

History contains at most 100 operations in memory: UTC time, direction, outcome,
duration, input bytes, replacement counts and scalar phase diagnostics. It stores
no text, markers, mappings or hashes. Statistics persist only scalar lifetime
totals. Forgetting everything resets history and totals, keeping ordinary
preferences. Updates are manual; app and terminal detection are inactive roadmap
items, not active monitoring.

## Development

```sh
cargo fmt --all -- --check
cargo test-core
cargo test -p copy-for-ai-desktop
cargo check -p copy-for-ai-desktop
cargo clippy --workspace --all-targets -- -D warnings
```

Read the [agent rules](AGENTS.md), [project plan](docs/plan/README.md),
[structure](docs/plan/project-structure.md) and
[verification record](docs/plan/verification.md). Code, comments, UI and
documentation use English. Synthetic multilingual fixtures intentionally retain
the languages needed to test detection and UTF-8 handling.

## License

The application source is licensed under [MIT](LICENSE). Dependencies and model
weights retain their own licenses; see [third-party notices](THIRD_PARTY.md).
Downloaded weights, build products, Python caches and local secrets are excluded
from Git. Benchmark fixtures and historical evidence are retained deliberately.
