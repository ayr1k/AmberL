# Third-party notices

Project source: Mozilla Public License 2.0; see LICENSE.
Ambrum's behavior references AMBER at 3af52c7 (MPL-2.0). No private credentials,
AMBER artwork/fonts or Electron runtime are included.

The initial locked Rust graph includes serde/serde_core/serde_derive,
serde_json, proc-macro2, quote, syn, unicode-ident, itoa, memchr, zmij,
windows-sys and windows-link. Exact versions are in Cargo.lock. Most are
MIT OR Apache-2.0; unicode-ident requires Unicode-3.0 in addition to MIT or Apache-2.0,
memchr offers Unlicense or MIT, and zmij is MIT. The packaging script
includes original license files from the exact resolved crates; those texts govern.
Rust standard libraries are MIT OR Apache-2.0 and carry additional third-party notices.
Windows system libraries and system fonts are used from the OS, not redistributed.

No Slint, egui, WinUI, Minecraft, Java, mod or FRP binary is redistributed by this probe.
Dependencies and notices must be reviewed again as those capabilities are introduced.

