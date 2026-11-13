# Third-party notices

Project source: Mozilla Public License 2.0; see LICENSE.
Ambrum's behavior references AMBER at 3af52c7 (MPL-2.0). No private credentials,
other products' account files, bundled fonts or Electron runtime are included.
The user supplied and authorized reuse of the AMBER graphic logo for AmberL.
The derived dark logo retains that shape, with a teal center and terminal L.
The voxel landscape and dark lockup in assets/launcher were produced with Image Gen;
they are interface artwork, not screenshots or redistributed Minecraft game resources.

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

The core probe does not redistribute Slint, egui, WinUI, Minecraft, Java, mod or FRP binaries.
Dependencies and notices must be reviewed again as those capabilities are introduced.

New-directory installation uses zip 9.5.1 (MIT), with only stored/Deflate reading
enabled through flate2 (MIT OR Apache-2.0) and zlib-rs (Zlib). No home-grown ZIP or
compression implementation. constant_time_eq offers CC0-1.0 OR MIT-0 OR Apache-2.0;
the complete packaged notices preserve these alternatives and all transitive licenses.
The expanded lock graph includes their transitive dependencies; exact build-target
versions and original license texts are included by package-probe.ps1. Dependencies
for other targets (for example UEFI or WebAssembly) are not part of this Windows binary.
Windows
SetFileInformationByHandle performs the no-replacement directory commit.
Official Minecraft metadata/resources are downloaded only into a user's explicitly
chosen development cache/install directory, never redistributed in the probe ZIP.

Account authentication uses form_urlencoded and percent-encoding (MIT OR Apache-2.0),
and zeroize/zeroize_derive (MIT OR Apache-2.0). Windows DPAPI provides current-user
credential encryption. No separate authentication broker, browser runtime or crypto
service is bundled. Exact locked versions and original license texts ship in the package.

The separate UI comparison package adds Slint 1.18.1 under its desktop Royalty-free
2.0 option, with the standard AboutSlint widget accessible from About. Original Slint
and all resolved Windows dependency license texts are included. Framework types are
not exposed through the core/bridge API. See docs/UI-COMPARISON.md for scope.
The C++/WinRT host uses locked Microsoft Windows App SDK 2.6.5 components and SDK
runtime assets, with original Microsoft notices. WebView2 metadata/loader is a build
dependency of WinUI projections, not an embedded browser or instantiated WebView.
Windows and the Microsoft Visual C++ x64 runtime remain host prerequisites.

The integrated launcher also uses image 0.25.10 (MIT OR Apache-2.0) for a cached
Gaussian blur of the embedded static scene. The PNG decoder and its locked transitive
dependencies are covered by the UI package's original license files. It does not
capture or blur the desktop or other applications.


Built-in instance icons reuse AMBER's project-owned generated grass-block illustration
and unmodified Lucide 0.468.0 box/anvil/pickaxe SVGs. Original ISC/Feather MIT notices
are retained in assets/instance-icons/LICENSE-lucide.txt and copied into test packages.
SVG theme tint is applied only by the native renderer. No Minecraft textures are bundled.
Official Minecraft and loader binaries are downloaded for the user's selected instance,
not included in source or test ZIPs. Forge/NeoForge installers execute only when that
component is explicitly selected and are not redistributed with AmberL.
