# Validation record: initial native probe

Do not infer gameplay
acceptance from compilation, synthetic inspection or a visible launcher window.

## Initial verified results
- Ten targeted core regressions passed on Windows: unknown/shared/isolated/conflicting
  layouts, ambiguous roots, inheritance cycles, cross-library paths, same-name identity,
  cancellation, bounded metadata, managed identity mismatch and Java version parsing.
- Release binaries compiled successfully. `cargo clippy --workspace --all-targets -- -D warnings`
  passed. The source is formatted with rustfmt.
- Actual Microsoft OpenJDK 25.0.4 executable inspection succeeded and reported amd64.
  Java 8 parsing was synthetic; an actual Java 8 runtime has not yet been tested.
- All package resolution used existing cached registry archives copied into ignored local
  build storage. Direct crates.io DNS resolution failed. No Tauri/WebView dependency
  was adopted even though those crates were present in the machine cache.

## Continued milestone: persistent library registry
- Ten new Windows registry tests passed: persistence/deduplication, stable scan identity,
  original library preservation, same-path directory replacement, concurrent writer lock,
  corrupt/future schema preservation, cancellation/offline detach, failed commit with
  ignored temporary residue, app/game overlap, hardlinked lock rejection, rejecting a
  game root before app writes, and holding a directory against rename while open.
- The existing ten inspection tests passed after the library identity change. Subsequent
  registry/UI fixes reran only the affected registry tests and UI script.
- `cargo clippy --workspace --all-targets --offline --locked -- -D warnings`, rustfmt
  and the release build passed. Rust reported incremental-cache hardlink fallback to
  copying on this host; this was a build-cache notice, not a test/lint failure.
- `scripts/test-native-probe.ps1` passed using the real compiled Win32 controls:
  120 generated profiles, scan, filter, keyboard row selection/detail, attach, normal exit,
  restart, reopen saved library, page changes and 720x680 / 1140x820 window resizing,
  detach and normal exit. Every fixture file path and SHA-256 remained unchanged.
- This is hidden-window message automation. It does not validate rendered appearance,
  actual Chinese IME composition, screen reader output, physical keyboard or DPI behavior.
- Packaged source is pinned by TEST-REVISION.txt; SHA256SUMS.txt covers package content.
  The package includes a source archive so the milestone is recoverable before GitHub upload.
- Network still cannot resolve crates.io. GitHub returned 404 for `ayr1k/AmberL` through
  the available connector; it has no repository-creation action. No remote upload or PR
  is claimed. The local branch/commit and development ZIP are the current delivery.

## Remaining targeted checks
- Slint/WinUI comparable UI samples and license decision once dependencies are accessible.
- Real Java 8, authentication and visible Minecraft launch; native tray/process lifecycle.
- Rendered UI, Chinese input, accessibility, per-monitor DPI and AMBER resource baseline.
- GitHub repository creation, source upload, CI execution and downloadable PR delivery.

## Limits
No Minecraft authentication, visible game/world, installation, download, backup,
recovery, FRP, cross-device play, installer/upgrade or non-Windows UI acceptance.
Slint/WinUI selection, full Chinese IME/reader/DPI matrix and AMBER resource comparison
remain pending. Probe footprint cannot predict completed product footprint.

