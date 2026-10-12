# Copy for AI roadmap

## Settings and app information — implemented 2026-10-12

Native sections: General, Appearance, Terms, Sessions and data, Applications,
Updates and About. Open with Settings, Cmd+, or the menu bar. Navigation preserves
retained editors, vault, history and ongoing processing. Automatic direction is
unchanged while the main screen avoids its technical explanation.

System/light/dark appearance and history-at-startup persist in a fixed scalar
schema. Documents, markers, originals, mappings and history do not persist.
Global shortcuts are enabled per run. Launch at login uses the installed main
.app's SMAppService; actual registration and next-login behavior need separate
native acceptance. About displays version and full MIT text; dependencies/models
retain their own licenses.

## M4 — updates and distribution: TODO

- Configure a trusted channel-specific release source and compare actual versions;
  never claim the installed version is current without verification.
- Provide signed/notarized macOS bundles, integrity verification and safe install
  with rollback on failure.
- Respect unfinished editors, vault and operations before exit/restart. Never
  silently discard a session during updates.
- Optional update checks must show their state and explicitly describe metadata-only
  network traffic. Never transmit user text.

Updates currently display the built version and manual mode. The check action is
inactive without a release source, and the TODO remains visible in UI.

## M5 — detection of selected apps: TODO

Prepare an explicit opt-in list for **Zed, Delta, Codex and Claude**, distinguishing
editors/standalone applications from terminal tools. Current UI rows are inactive;
none of these apps or processes is monitored.

- Identify desktop targets by actual bundle ID/executable and PID, not window
  names alone.
- Distinguish terminal host, active pane/tab and CLI process/subtree. Focusing
  Terminal/iTerm does not identify a tool or enable protection for every command.
- Document the supported app/editor/CLI matrix and actual identification limits.
  Do not assume similarly named Delta or other variants are the same product.
- Default off; users enable each target. Show actual protection, permissions,
  conflicts and unsupported targets.
- Ordinary Cmd+V interception would require suppressing the original event before
  detection. Model errors, timeout, cancellation and target/clipboard changes
  must not pass through the original text. No regex-only fallback.
- Reuse the existing workflow, generation/cancellation and single native paste
  boundary. Restore known markers once, without scanning originals again.
- Verify each supported target using actual system paste with synthetic data.
  Headless tests, PID detection and key dispatch alone do not establish paste.

Until M5 is complete, use explicit Cmd+Shift+V / Cmd+Shift+R only. Ordinary Cmd+V
is not intercepted globally.

## M3 — native UI and statistics: implemented 2026-10-12

Ink + Emerald was selected from three designs. Sidebar navigation separates
Workspace, History, Statistics and Settings. Configuration leaves the workspace.
History retains at most 100 metadata entries in memory. Statistics show a native
pie chart, percentages and scalar totals across restart. No documents, markers,
mappings or hashes persist. Forgetting resets totals too. Pre-implementation
historical data are unavailable.
