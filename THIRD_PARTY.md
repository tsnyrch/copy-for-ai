# Third-party licenses

Copy for AI's original source code is licensed under the [MIT license](LICENSE).
The workspace and all four crate manifests inherit that license. This does not
relicense dependencies, pretrained weights or bundled third-party assets.

| Component | Version or revision | Declared license |
| --- | --- | --- |
| GPUI Kit | 0.7.3 | Apache-2.0 |
| Zed GPUI (`gpui-pre`) | 0.3.9 | Apache-2.0 |
| gline-rs | 1.1.0 | Apache-2.0 |
| GLiNER multilingual ONNX export | `6ddaeb9413b0e71ad8457da1aab378a165b24058` | Apache-2.0 |
| Roboto (bundled variable TrueType font) | google/fonts `1c627bfa375fc51cf86fabeca4f6e08a95f0aa5c` | SIL Open Font License 1.1 |

Roboto's [source and checksum](crates/desktop/assets/fonts/README.md) and
[full license](crates/desktop/assets/fonts/OFL.txt) accompany the asset. The font
and its license are embedded in the app; the license is available in Settings → About.

The three Rust dependency declarations above were checked against the locally
resolved crate manifests on 2026-10-28. Model provenance and pinned hashes are
recorded in [the processing plan](docs/plan/generic-processing.md) and
`scripts/install-model.py`. Weights are downloaded separately, not committed.

`Cargo.lock` records the complete resolved dependency graph. A locked offline
Cargo metadata check for `aarch64-apple-darwin` resolved 562 packages, all with
declared license metadata. Most offer MIT and/or Apache-2.0; the graph also
includes BSD, ISC, Unicode, Zlib and other licenses. In particular, `cbindgen`
0.28.0 and `option-ext` 0.2.0 declare MPL-2.0, and `webpki-roots` 0.26.11/1.0.9
declare CDLA-Permissive-2.0. Their terms remain applicable to those components.
This metadata review is scoped to the native macOS dependency graph; the table
is a summary, not a complete shipped-component notice bundle.
Before distributing binaries or model bundles, include the applicable upstream
license texts and notices for all shipped components. Experimental models in
benchmark reports have separate provenance and license requirements; screening
them does not approve them for production or redistribution.
