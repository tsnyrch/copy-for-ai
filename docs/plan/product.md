# Product and controls

A standalone local GPUI app with an English interface. Detection combines a
multilingual model with deterministic rules; it does not guarantee complete
anonymity. See the [processing contract](generic-processing.md).

## Main screen

Two retained Input / Output editors. **Process input** automatically chooses
anonymization or exact restoration using local markers. **Load clipboard** is
attached to input and **Copy** to output. Progress and cancellation remain
available while processing; failures appear in the status bar. Remind users once
to review the output before sharing.

Navigation separates Workspace, History, Statistics and Settings. Configuration,
custom terms, session management and technical IDs belong in Settings. New session
keeps older mappings in the current process. Forget everything requires a second
confirmation and clears sessions, rules, editors, history and lifetime totals.

## History and statistics

History is a compact table of at most 100 in-memory operations. It records UTC
time, direction, completed/cancelled/failed status, duration including modifier
wait, input bytes and replacement count. Scalar phase diagnostics distinguish
model, preparation, loading, core and paste time. No documents, markers, mappings
or content hashes are stored.

Statistics have their own page: operation and replacement totals, input volume,
average duration, an outcome pie chart, percentages and direction counts. Only
scalar aggregates persist in `statistics.conf` in the app directory, as explicitly
requested. Totals survive restart and history eviction. Collection begins with
this implementation; earlier use cannot be reconstructed. Writes run off the UI
thread using atomic rename and serialized snapshots. Forced exit may lose the
latest pending snapshot. An unreadable file is not overwritten automatically;
explicit forgetting permits reset. Save failures are reported.

The Ink + Emerald design uses dark blue navigation, an emerald primary action,
contrasting status colors, system typography and clean panels in both themes.

## macOS commands

- **Cmd+Shift+V** reads the current clipboard, anonymizes unmarked text or exactly
  restores known local markers, then performs guarded paste into the original
  target. Restored originals are not scanned. Mixed unknown markers are preserved
  and reported; unknown-only input does not paste.
- **Cmd+Shift+R** and the menu bar restore only the clipboard.
- Loading, preview, clipboard processing and copy never trigger paste themselves.
- Shortcuts require explicit activation. Conflicts are reported; failure to
  register the second shortcut unregisters the first. Ordinary Cmd+V is not
  intercepted globally.
- Protected paste requires Accessibility, unchanged target/clipboard, current
  generation and vault revision, and released modifiers. Model failure stops the
  operation. Never send Enter or submit a prompt.

## Markers

Examples: `[[PERSON_A1B2C3D4_1]]`, `[[ORG_A1B2C3D4_1]]`. The random session scope
is not derived from identity. The same exact value in a session reuses its marker;
different values are not merged by inference. Unknown markers are preserved.
Mappings stay in memory and are not appended to output.

## Settings

Sections: General, Appearance, Terms, Sessions and data, Applications, Updates,
About. Open with navigation, Cmd+, or the menu bar. Navigation preserves editors,
vault and ongoing work. Automatic direction still comes from the domain workflow;
its technical explanation need not occupy the main screen.

General includes macOS launch at login, history at startup and current-run global
shortcuts. Appearance offers system, light and dark modes. Ordinary preferences
persist separately and survive forgetting sensitive data. About displays version
and the full MIT license. Updates remain manual without a configured release
source. Selected desktop/terminal app detection is an inactive roadmap item.
