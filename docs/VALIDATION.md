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

## Remaining targeted checks at the initial checkpoint (historical)
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

## Continued milestone: batch cache preparation (2026-11-12)
- Eight targeted preparation tests passed with injected streams and actual Windows
  cache/hash operations: index-first discovery and deduplication; verified cache reuse;
  interrupted batch recovery; cancellation before request/after index; preflight and
  discovered-asset budget rejection; corrupted-index detection/recovery; linked metadata
  and game-root rejection; active-writer/non-JSON metadata rejection. All eight existing
  download tests passed after the cache API change.
- Strict Clippy and release build passed. The CLI shares Ctrl+C cancellation handling
  between single-resource fetching and batch preparation; actual interactive Ctrl+C
  latency is still unverified. Remote CI remains unexecuted because upload is blocked.
- The packaged CLI smoke script uses three seeded cache files totaling 87 bytes and
  synthetic metadata. It verifies planning, cache preparation, budget rejection and input
  preservation. This is intentionally separate from the unsuccessful live HTTPS check.
- No game resource materialization, archive extraction, login or actual launch has been
  added. Completing the batch means all planned resources were verified in app cache.

Packaging now disables host `core.autocrlf` conversion for the source archive and checks
every archived file against its Git blob SHA. The first resumed package check exposed
CRLF-only differences caused by the host setting; the corrected package preserves exact
committed source bytes. Existing prior packages are retained, not overwritten.

## Resumed Windows checkpoint (2026-11-12, America/New_York)

The original Windows repository was available, clean and exactly matched local commit
`812872f34a8ce21e6ea2f7cdef296e1eef15fd8d` / tree
`60e16b6665d205f441d3915da06c3c08456733fc`. Rust 1.99.1 MSVC is usable. All original
45 tests passed in the normal Windows execution context. Restricted execution first
denied ancestor directory handles; that environment failure is not recorded as a product
regression. No unrelated AMBER test suite or original game/account data was used.

- Actual AmberL WinHTTP HTTPS of the pinned 16,726-byte LICENSE passed its expected
  SHA-256, then returned a verified cache hit. crates.io lookup/dependency retrieval works.
- Remote inspection found only the unchanged initial main, no previous PR. The ayr1k
  connector's create_blob/create_pull_request still returned 403, separately from network
  reachability. Normal Git with the verified ayr1k identity successfully uploaded source.
- Integration commit `c581c20ca744a8bab2fe0dc13c1e9db708a19872` parents remote main and
  the original local history. Its source tree is exactly the recovered tree above. Neither
  history was rewritten. Main remains `b6abee821f2354014e22b289f9e1b340bbd0c159`.
- [Draft PR #1](https://github.com/ayr1k/AmberL/pull/1) exists. The initial
  [Windows run](https://github.com/ayr1k/AmberL/actions/runs/43359836356) passed tests,
  strict Clippy, release build, native-control automation, CLI resource-plan smoke and
  package publication. Subsequent PR revisions/artifacts identify their exact tested SHA.

## Continued milestone: new-directory installation

Targeted catalog/installation tests cover official metadata ID/hash/origin and cancellation;
new directory copy isolation; budgets; cancellation/incomplete stage retention; complete
stage reopen/recovery; corrupt/extra/hardlinked/replaced/unknown-schema stages; competing
parent locks; pre-commit destination conflicts; active cache writers; native path traversal,
case and file/directory collisions, symlinks, CRC corruption and a real Windows junction;
Deflate extraction; native expansion budget; virtual/resource asset copies. Existing
download/preparation regressions were rerun because the cache gained a pinned-file reader.

`scripts/test-install-network.ps1` used the actual release CLI and a new disposable
app/cache + installation parent. No other downloader seeded these resources. The official
catalog, selected metadata, libraries, asset index and assets were requested through Ambrum.
Each installed receipt was independently rehashed using PowerShell; client JAR SHA-1 was
also checked against its official descriptor. The second preparation reused every resource.

| Version | Verified cache objects | Planned cache bytes | Second preparation hits | Materialized files | Materialized bytes | Extracted native files |
|---|---:|---:|---:|---:|---:|---:|
| 1.21.4 | 4,128 | 526,105,870 | 4,128 | 4,129 | 526,147,716 | 0 |
| 1.12.2 | 1,328 | 181,001,390 | 1,328 | 1,340 | 182,877,641 | 11 |

Metadata SHA-1: `87fcc1eec25874d12bca737d6579a247ce2d2746` (1.21.4),
`832d95b9f40699d4961394dcf6cf549e65f15dc5` (1.12.2). The independent receipt check
also includes one pending journal per instance (4,130 / 1,341 checked files). See
`docs/evidence/install-network.json` for the tested executable hash and counts. That hash
identifies the local live-test executable; a separate CI build may have a different hash.
The packaged synthetic CLI check covers a Chinese/space-containing target name, metadata
preservation and rejecting an existing installation without modifying it.

Recovery tests simulate cancellation and reopened handles; they are not a claim of
power-loss durability or a full process-kill fault matrix. Native extraction is not evidence
of Java 8 or an old-game window. Both live results explicitly set `launch_verified:false`.

Packaging the expanded dependency graph exposed `r-efi` without bundled license files.
It is a UEFI-only dependency, not linked into the Windows binary. Packaging now uses
Cargo's filtered build-target resolve graph and includes notices for every reachable
Windows dependency, retaining strict failure for a missing notice in that graph.

## Limits
No Minecraft authentication, visible game/world, existing-instance update, installation UI,
general backup/restore, FRP, cross-device play, installer/upgrade or non-Windows UI acceptance.
New-directory materialization and complete-stage recovery are the only new write capabilities.
Slint/WinUI selection, full Chinese IME/reader/DPI matrix and AMBER resource comparison
remain pending. Probe footprint cannot predict completed product footprint.

