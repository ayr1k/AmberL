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

## Continued milestone: verified downloads
- Eight targeted download tests passed with injected streaming transports and actual
  Windows CNG/file operations: known SHA-1/SHA-256 vectors and cache reuse; short/long/
  corrupt payloads; cancellation before request and commit; interrupted transfer and
  explicit retry; corrupt-cache replacement and failed commit preserving the old file;
  concurrent artifact exclusion; invalid URLs/digests; hardlink/game-root protection.
- This does not establish HTTP/TLS success: a real WinHTTP request for AMBER's LICENSE
  at 3af52c7 failed with Windows 12007 (DNS resolution). No artifact was committed.
  The expected 16,726-byte fixture and SHA-256 were independently obtained through
  the authenticated GitHub connector. The bundled network smoke script can be run
  from a normal Windows terminal; it reports failure honestly and requires a fresh download.
- rustfmt, strict Clippy and release build passed. Existing unrelated library/UI tests
  were not repeated for this separate module. Earlier native UI evidence remains the
  f28cdba milestone's evidence, not a new visual acceptance result.
- The newly connected GitHub identity was confirmed as ayr1k. `ayr1k/AmberL` still returned
  404, and the connector has no repository-creation action. Upload/PR remains pending
  creation of a repository with an initial commit (for example an initialized README).
- Remaining: live network success, redirect/provider policy, resumable downloads,
  game manifest/planning and installation, task UI, login and visible game launch.

## Continued milestone: Vanilla metadata plans (2026-11-12)
- Nine manifest tests passed: ordered platform/feature rules; argument boundaries and
  format limits; platform-specific libraries/native archives; invalid/conflicting paths;
  unsupported inheritance/entrypoints; unknown Java requirements; deduplicated asset
  mappings; conflicting asset sizes; missing required OS version stays an error.
- rustfmt, strict Clippy and release build passed. Existing unrelated inspection/registry/
  UI tests retain their prior evidence; they were not rerun for this pure planner.
- Read-only CLI checks against two existing QA-copy Vanilla descriptors succeeded:

| Version | Required Java | Planned files | Classpath entries | Metadata SHA-256 |
|---|---:|---:|---:|---|
| 1.21.4 | 21 | 87 | 85 | `110a9db5df120e614728e248ee0d705af07438d1de1d261bc7c3c85d17cf8e12` |
| 26.3 | 25 | 77 | 75 | `7755c56601d666849aad5b4266503cacc17dac1b747018fccd5a19201b3dda82` |

Both used explicit OS version `10.0.26100` and planned zero legacy native archives.
These are metadata checks, not successful downloads/installation/launches. No personal
account files or game content are included. `plan-assets` CLI also passed a synthetic
two-name/one-hash deduplication check.

## Delivery/network checkpoint (2026-11-12; supersedes earlier 404 reports)
- `ayr1k/AmberL` exists. Remote `main` is initial commit
  `b6abee821f2354014e22b289f9e1b340bbd0c159`; complete branch pagination showed only
  `main`, and PR search returned no PRs. No duplicate upload was found.
- The explicitly selected ayr1k connection still returns GitHub 403
  `Resource not accessible by integration` for `create_blob`. Earlier attempts to create
  a tree/branch also failed. Installed-account enumeration is empty. Metadata reporting
  push permission did not establish integration write access. No remote commit/branch/PR
  was created; the cause of the authorization mismatch has not been proven.
- Retrying the actual compiled WinHTTP downloader still failed with Windows 12007 (DNS),
  leaving zero committed download files. This remains separate from GitHub delivery.
- The retained local history and source ZIP are the recovery path. When writes are
  available, the upload must parent remote main, preserve local history, record the
  local/API commit mapping, and verify remote files and a Draft PR without force-pushing.

## Limits
No Minecraft authentication, visible game/world, installation, content download UI, backup,
recovery, FRP, cross-device play, installer/upgrade or non-Windows UI acceptance.
Slint/WinUI selection, full Chinese IME/reader/DPI matrix and AMBER resource comparison
remain pending. Probe footprint cannot predict completed product footprint.

