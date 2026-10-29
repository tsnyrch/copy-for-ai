# Copy for AI — Signal Orange

Selected by the user for the xsnyrch design-system migration. This is the native
Rust/GPUI application, not a web frontend. Product name, icon, local workflows,
permissions and paste safety boundaries are unchanged.

## Source specification and application-derived roles

Source: the attached `xsnyrch-design-system` skill. Signal Orange is a brand
seed, not a complete pre-tested UI theme. Only these four colors are original
palette values: Signal Orange `#FF4D00`, Ink `#0B0C0E`, Warm Paper `#F5F4F0` and
Accessible Orange `#C83700`. All other colors below are derived for Copy for AI.
The dark companion is application-derived, not an original dark source design.

| Role | Light | Dark |
| --- | --- | --- |
| Canvas | `#F5F4F0` | `#0B0C0E` |
| Surface / navigation | `#FFFFFF` | `#191A1D` |
| Main text | `#0B0C0E` | `#F5F4F0` |
| Secondary text | `#65625B` | `#B4B1A9` |
| Decorative divider | `#DEDCD5` | `#3E3E40` |
| Control boundary | `#85827A` | `#85827A` |
| Action / hover / pressed | `#FF4D00` / `#F04A00` / `#E54500` | Same |
| Text on action | `#0B0C0E` | `#0B0C0E` |
| Accent text / focus | `#C83700` | `#FFAD87` |
| Accent wash | `#FFF0E8` | `#362017` |
| Neutral hover | `#ECEAE4` | `#2B2B2E` |
| Success | `#216343` | `#81D5A8` |
| Warning | `#805300` | `#F2C66D` |
| Danger | `#B42332` | `#FF9AA4` |

One shared palette maps these roles into GPUI Kit's component colors and resolved
tokens. Selected navigation uses an accent wash, persistent line and text; it is
not another primary CTA. Selected navigation retains the accent wash on hover:
Accessible Orange on the neutral hover would be only 4.36:1. Appearance choices
use inverse neutral styling. Destructive actions use an outline instead of a
competing filled action. Informational notices are neutral, not automatically
green; History and Statistics explicitly label each outcome.

## Typography and geometry

- Embedded, unmodified Roboto variable TTF; no runtime font download. Body 400,
  emphasis 500, large metric values 300. Font provenance and license accompany
  the asset, with the full OFL license embedded in Settings → About.
- Page 18/24, section 16/24, body 14/22, metric 28/36 px.
- Spacing follows 4/8/12/16/24/32 px. Controls are 36px tall with 5px rounding;
  panels have 8px rounding and 1px dividers. No automatic card shadows.
- A single dominant Process input action; clipboard processing remains secondary.
  Load and Copy stay next to their respective retained editor entities.
- Desktop sidebar 240px. Below 960px, navigation becomes horizontal, the page
  header stacks and Input/Output stack vertically. Statistics cards wrap and the
  chart stacks above its legend. Settings navigation/rows wrap. The native window
  minimum is 640 × 700 logical pixels; this is not a mobile implementation.
- History keeps its column headers and metadata in a locally horizontally
  scrollable table rather than truncating or dropping columns. Compact main
  content is explicitly width-constrained so table intrinsic size cannot expand
  the whole page outside the window.
- Continue using GPUI Kit buttons, switches and inputs with their keyboard,
  disabled and focus behavior. Editors and custom-term input have explicit
  accessible labels. No per-render editor entity creation.

## Evidence boundary

See [verification](../plan/verification.md) for actual checks. Exact sRGB pairs
meet the checked text/boundary thresholds, but that is not an accessibility
certification. The first migration pass had no usable native capture. The Settings
crash follow-up adds real rendered navigation/resize tests and native light-theme
screenshots of desktop Settings and compact Settings/Workspace. Dark mode, all
four pages, scroll reachability and keyboard activation are covered headlessly,
not by physical UI automation. VoiceOver, physical resize/focus and actual system
paste remain manual acceptance work. Historical Ink + Emerald screenshots and
proposals are not evidence of this UI.

To reproduce isolated previews without changing ordinary preferences:

```sh
cargo run -- --settings-preview
cargo run -- --settings-preview --compact-preview
cargo run -- --ui-preview --compact-preview
```
