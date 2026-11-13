# Third-party notices

Project source: Mozilla Public License 2.0; see LICENSE.
Ambrum's behavior references AMBER at 3af52c7 (MPL-2.0). No private credentials,
AMBER artwork/fonts or Electron runtime are included.

The initial locked Rust graph includes serde/serde_core/serde_derive,
serde_json, proc-macro2, quote, syn, unicode-ident, itoa, memchr, zmij,
windows-sys and windows-link. The metadata planner adds regex, regex-automata,
regex-syntax and aho-corasick (MIT OR Apache-2.0). Exact versions are in Cargo.lock. Most are
MIT OR Apache-2.0; unicode-ident requires Unicode-3.0 in addition to MIT or Apache-2.0,
memchr offers Unlicense or MIT, and zmij is MIT. The packaging script
includes original license files from the exact resolved crates; those texts govern.
Rust standard libraries are MIT OR Apache-2.0 and carry additional third-party notices.
Windows system libraries and system fonts are used from the OS, not redistributed.
The download adapter also calls Windows WinHTTP (TLS/system certificates) and CNG
(SHA-1/SHA-256). No separate networking/crypto runtime is bundled.

No Slint, egui, WinUI, Minecraft, Java, mod or FRP binary is redistributed by this probe.
Dependencies and notices must be reviewed again as those capabilities are introduced.

New-directory installation uses zip 9.5.1 (MIT), with only stored/Deflate reading
enabled through flate2 (MIT OR Apache-2.0) and zlib-rs (Zlib). No home-grown ZIP or
compression implementation. constant_time_eq offers CC0-1.0 OR MIT-0 OR Apache-2.0;
the complete packaged notices preserve these alternatives and all transitive licenses.
The expanded lock graph includes their transitive dependencies; exact resolved
versions and original license texts are included by package-probe.ps1. Windows
SetFileInformationByHandle performs the no-replacement directory commit.
Official Minecraft metadata/resources are downloaded only into a user's explicitly
chosen development cache/install directory, never redistributed in the probe ZIP.

