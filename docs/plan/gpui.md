# GPUI and verified sources

Checked against documentation and crate metadata on 2026-10-12.

## Decision

Use `gpui-kit = "=0.7.3"` with default features. This is the unified package
formerly called GPUI Component, re-exporting Zed GPUI, components and assets.
Zed provides the UI framework; Longbridge provides the component library.

GPUI Kit pins `gpui-pre = 0.3.9`. Do not also add `gpui = 0.2.2`, which creates
incompatible Entity/Context types. The Kit's macOS dependency enables `font-kit`
for text rendering. GPUI Fast and WebView are unnecessary. Commit `Cargo.lock`.

Tests use the dev-only alias `gpui-test` for the same `gpui-pre = 0.3.9` with
`test-support`. Cargo unifies it with runtime. Kit 0.7.3's own test-support feature
caused conflicting exact versions of unused Fast dependencies (`cocoa`,
`unicode-properties`); enabling support directly on the identical upstream
snapshot fixes resolution without replacing runtime or downgrading dependencies.
Tests do not use Kit-specific snapshot helpers.

## Primary documentation

| Source | Purpose |
| --- | --- |
| [Zed GPUI](https://gpui.rs/) | Framework origin and render model. |
| [Getting started](https://gpui-kit.com/docs/getting-started/) | Single dependency, init/open_window, Root and macOS fonts. |
| [Crate metadata](https://crates.io/crates/gpui-kit/0.7.3) | Exact version, features and dependencies. |
| [API](https://docs.rs/gpui-kit/0.7.3/gpui_kit/) | Re-exports and actual Rust signatures. |
| [Component catalog](https://gpui-kit.com/component/) | Input, Button, Switch and Dialog availability. |
| [Input](https://gpui-kit.com/component/input/) | InputState, multiline editing, placeholders and change events. |
| [Button](https://gpui-kit.com/component/button/) | Variants, disabled state and click handlers. |
| [Kit source](https://github.com/longbridge/gpui-kit) | Published implementation and tested recipes. |
| [global-hotkey](https://docs.rs/global-hotkey/0.8.0/global_hotkey/) | Global registration, macOS main loop, X11-only Linux. |
| [gline-rs](https://github.com/fbilhaut/gline-rs) | Local NER inference adapter. |

The [old Longbridge URL](https://longbridge.github.io/gpui-component/docs/getting-started)
redirects to GPUI Kit. Recheck versions before copying older examples.

## Implementation pattern

- `gpui_kit::application().with_assets(gpui_kit::assets::Assets)` creates the app.
- `gpui_kit::init(cx)` initializes component layers.
- `gpui_kit::open_window` wraps the view in Root for focus and overlays.
- Retain multiline editor entities and a single-line custom-term InputState.
  The Kit distinguishes Input, Textarea and code Editor; do not create editor
  state inside render.
- Use `cx.listener` handlers and `cx.notify()` after changes.
- Controllers own detectors and sessions; render reads state and composes elements.
- Retain the asynchronous shortcut receiver task with an explicit owner; never
  block the UI thread.
- Display permission and registration status from runtime, not intended settings.
