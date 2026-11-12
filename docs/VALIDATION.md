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

## Remaining targeted checks
- Core tests: unknown/shared/isolated/conflicting layouts, ambiguous roots, inheritance
  cycles, cross-library paths, same-name identity, cancelled scan, bounded metadata,
  managed identity mismatch, Java 8/modern parsing.
- Build: format, lint, release build and dependency tree.
- Live Java: inspect an existing explicitly selected executable; no game launch.
- Native UI: start, synthetic scan, filter, detail selection, keyboard and resize.
- Delivery: exclude caches/personal data, pin revision, hash package, upload source.

## Limits
No Minecraft authentication, visible game/world, installation, download, backup,
recovery, FRP, cross-device play, installer/upgrade or non-Windows UI acceptance.
Slint/WinUI selection, full Chinese IME/reader/DPI matrix and AMBER resource comparison
remain pending. Probe footprint cannot predict completed product footprint.

