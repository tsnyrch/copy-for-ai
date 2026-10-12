# Architecture and safety boundaries

See the [generic multilingual pipeline](generic-processing.md) for the current
contract. Automatic direction and an in-memory vault replace manual direction
selection and the original single-session design. `crates/ner` performs local
inference without JSON-key routing or dependence on UI language. Loading and
inference run outside the UI thread.

## Workspace

| Crate | Responsibility |
| --- | --- |
| `copy-for-ai-core` | Rules, validators, UTF-8 spans, overlap resolution, sessions, vault, anonymization and restoration; no UI, clipboard, network or disk. |
| `copy-for-ai-ner` | Verified local model/tokenizer assets, token windows and inference; no clipboard or UI. |
| `copy-for-ai-desktop` | Retained GPUI editors, application coordination, clipboard and shortcut workflows, native platform adapters. |
| `copy-for-ai-cli` | Interactive or whole-document stdin/stdout adapter to the shared core; no persisted mappings. |

## Detection pipeline

Validate input limits → protect existing markers → local model and literal custom
rules → explicit secrets → email/IBAN/validated Czech personal identifiers → phone
heuristics → deterministic overlap resolution → span replacement → output and
metadata.

Priority: custom rule > entire secret > validated identifier > generic contact
information. Never replace an overlap twice. Spans are byte offsets on valid
UTF-8 boundaries. Compile rules once and treat invalid patterns as initialization
errors. Phone detection is heuristic. Czech personal identifiers validate dates,
historical checksum rules and permitted month offsets.

Restoration scans the original input once and checks output size before replacing
markers. Inserted originals are never scanned, preventing cascading substitutions.
Unknown/foreign markers remain unchanged and are reported. Existing protected
markers are preserved during anonymization. Output is bounded at 32 MiB,
including expansion of repeated long secrets.

## Sessions and memory

The vault has one active session and retains up to 16 sessions. A session has a
random UUID and a unique short scope. Exact `(kind, value)` pairs reuse markers;
reverse mappings restore originals. Values are not serialized or exposed through
Debug/logs. Owned strings are zeroized when forgotten/dropped, without claiming
erasure of all OS, swap or editor undo-buffer copies.

Limits include 4096 mappings, 8 MiB of originals and 256 custom terms. Worker
snapshots are bounded too. TTL and encrypted persistence require later design.

## Protected paste

1. Handle an explicit global shortcut, without monitoring every copy or replacing
   ordinary Cmd+V.
2. Capture the active application's PID and native clipboard change count.
3. Read plain text. Anonymize unmarked input using model and rules, or restore
   known markers exactly once. Unknown-only input, failure or unsupported content
   must not paste.
4. Wait asynchronously for Cmd/Ctrl/Alt/Shift release; stop on timeout.
5. Recheck generation, workflow revision, permissions, target and clipboard.
6. Write only the plain-text result through GPUI, removing original HTML/RTF.
7. Recheck at the native boundary and send Cmd+V to the same target, never Enter.
   Publish the prepared vault only after successful paste dispatch.

Cmd+Shift+V leaves the processed result in the clipboard. Cmd+Shift+R only restores
the clipboard. Never use a timer to put the original back: without confirmation
that the target read the clipboard, that creates a disclosure race. A clipboard
write and arbitrary application's paste are not a universal atomic transaction;
physical target-specific validation remains necessary. If native paste fails,
clear the unchanged result owned by this operation without restoring the original
or publishing pending mappings. Clipboard-only restoration also checks the native
change count before overwriting.

## Platform adapters

macOS uses global-hotkey on the main run loop, an asynchronously polled receiver,
NSWorkspace for active PID, NSPasteboard.changeCount for versioning, CoreGraphics
for modifiers and synthetic Cmd+V, and AXIsProcessTrusted for actual Accessibility
status. The app cannot grant itself permission; it explains manual approval and
reports runtime state. Register the two shortcuts transactionally.

Future Windows support needs RegisterHotKey, foreground process checks, clipboard
sequence numbers and SendInput, including integrity-level restrictions. X11 needs
clipboard ownership and focused-window checks. Wayland needs a separate portal
capability design; do not promise parity with X11.

## Model and worker

Local GLiNER through gline-rs/ONNX complements validated identifier rules. Keep the
model loaded. Cancellation/generation IDs prevent late results from replacing
newer state. Model failure or timeout rejects the result, never pastes original
text and never silently falls back to regexes. Quality, license, byte offsets,
packaging and memory are independent acceptance requirements.

## Protection limits

No runtime upload, telemetry, proxy or cloud model. Build-time downloads and the
explicit model installer are separate from text processing. Plain text does not
cover file metadata, pictures or screenshots. Other processes and OS clipboard
synchronization may read clipboard content; the app cannot erase existing copies.
AI replies may contain newly introduced sensitive data absent from mappings.
Input remains untrusted data and cannot issue application instructions.
