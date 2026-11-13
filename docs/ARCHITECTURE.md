# Architecture decisions

## Accepted
1. Ambrum is a UI-independent Rust library; a normal desktop session uses one app
   process plus necessary Java/FRP children. No Node daemon or localhost microservices.
2. Commands carry explicit LibraryId, ClientInstanceId/ServerInstanceId, operation
   and session identity. UI selection never supplies authority implicitly.
3. Profiles and actual game directories are different objects. Shared game directories
   and dependencies require locks at their real resource boundary, not just profile IDs.
4. AmberL installation/app/settings/cache/update identity is independent. Public
   Microsoft Client ID `ab7cf49a-fea7-432d-8ce8-beb922eb554e`, consumers authority and
   `XboxLive.SignIn XboxLive.offline_access` match AMBER's source by user instruction.
   No client secret is required/copied. Actual login is not implemented/accepted yet.
5. Filesystem mutations must use plan / validate / stage / verify / revalidate / commit
   / journal recovery. Never introduce a write API by treating this read-only scanner's
   canonical path check as a race-free capability.
6. Incomplete or conflicting evidence stays unknown. No automatic migration on attachment.

## Current module boundary
- `ambrum::library`: identity, layout evidence, cancellable bounded profile inspection.
- `ambrum::paths`: bounded reads and conservative reparse/traversal rejection.
- `ambrum::java`: bounded inspection of explicitly selected executables.
- `ambrum::task`: cancellation token; no speculative generic task framework.
- `ambrum::registry` (Windows): app-only registration, GUID identities, directory handles,
  serialized writes and same-directory staged replacement. No game-file write capability.
- `ambrum::download` (Windows cache adapter): bounded streaming, cancellation, progress,
  hash-keyed locks, verified cache reuse, same-directory commit. HTTP is an injected
  `Transport`; Windows WinHTTP and CNG implement TLS and hashes without custom protocols.
- `ambrum::catalog` (Windows): bounded official version discovery and metadata digest/ID checks.
- `ambrum::install` + private `transaction` (Windows): fresh isolated Vanilla directories,
  verified copies, bounded native extraction, parent lock, staged identity/receipt and
  no-replacement rename through a held handle. See INSTALLATION.md for recovery limits.
- `amberl-inspect`: independent CLI for reproducible inspection and explicit installation.
- `ambrum::manifest` (currently Windows): pure Vanilla/resource plans with ordered
  platform/feature rules and bounded regexes. No authentication, network, writes or execution.
- `ambrum::prepare` (Windows): two-phase Vanilla cache preparation, index first then
  unique resources; explicit byte budget, cancellation and reusable verified cache objects.
- `amberl-native-probe`: Win32 standard controls, virtual list, background worker.

Unregistered inspection uses canonical path + profile name. Registered scans use a GUID
plus profile name, and reopen/revalidate the stored volume/index/creation identity.
Registry scans keep non-delete-sharing handles to the directory and its ancestors.
All paths are checked for reparses before use; this conservative implementation can reject
otherwise legitimate linked libraries. Actual game contents are still read-only and are
not locked against external launchers. Deserialization does not authorize game mutations.
Win32 unsafe calls live in UI/platform adapters. The separate installation API writes only
new explicitly requested directories; registered external-library inspection remains read-only.
No game launch exists.

The registry holds a non-delete-sharing lock file with an exclusive OS lock; concurrent
writers fail fast. Updates write/sync a unique new file before same-directory replacement.
Uncommitted temporary files are ignored. Invalid/unknown schemas are preserved, not reset.
This is app configuration persistence, not a journal/backup engine for game files. Third-party
writers, hostile same-user processes and power-loss/storage failures are not fully modeled.
Only the local Windows filesystem used in the recorded tests is verified; network, ReFS,
removable filesystems and cross-machine directory identity need separate evaluation.

Download attempts use 64 KiB buffers, a 16 GiB descriptor ceiling and a 30-minute attempt
budget checked between blocking operations. WinHTTP uses five-second phase timeouts;
these are not a hard total cancellation latency guarantee. Progress is coalesced to
roughly ten updates/second plus phase transitions. Success is emitted only after commit.
TLS uses OS certificate validation. Cookies, automatic auth/logon and redirects are disabled.
URLs are not included in diagnostics; the cache contains no account credentials.
Retry starts a new attempt; there is no automatic retry storm or partial HTTP resume.
Normal failures/cancellation remove staged files; process-crash residue is ignored, with
cache quota/cleanup and crash-recovery UI still pending. No archive extraction or game
installation is implemented, and returned cache paths do not authorize later game writes.

Version planning preserves argument arrays, explicit Java requirements, native archive
separation and legacy asset mappings. It rejects custom entrypoints, inherited metadata,
Maven-only descriptors, unsupported extraction/logging rules and conflicting destinations.
Assets are deduplicated by hash with an ordered map; up to 100,000 logical objects are
accepted. Plans are not mutation authority or authenticated metadata. Installation must
independently validate resource ownership, archives and actual file state before committing.

Batch preparation checks known metadata against the byte budget before requesting its
asset index. The index is fetched and then reread with a fresh digest check using a file
handle that denies concurrent writes/deletion; reads reject links and exceedance of 8 MiB.
Asset discovery has no claimed total/percentage. Once it succeeds, the entire unique
resource plan must fit the caller's byte budget (cache hits included; at most 105,000 files).
Only committed resources contribute to the completed count. Cancellation/failure does not
report batch completion and retains already verified files for the next explicit retry.
Preparation is sequential, with no automatic retries or partial-stream resume. This does
not provide a permanent immutable cache snapshot or authorization to install game files.

## UI decision remains open
Compare Slint and WinUI 3 on the same actual scan/list/input/task/log data. egui is an
optional additional sample. The small Windows control baseline supplies an immediately
buildable reference when new dependencies are unavailable; it is NOT WinUI 3 and must
not be represented as completing that comparison.

Slint 1.18.1 documentation was inspected. GPLv3 / Royalty-free / commercial choices
need a pinned dependency/license decision. Royalty-free 2.0 requires attribution and
restricts exposing Slint APIs; do not design third-party APIs around UI framework types.
No Slint dependency or license acceptance is part of this initial source.
References: https://slint.dev/agreements/slint-royalty-free-license.pdf and
https://learn.microsoft.com/en-us/windows/apps/winui/winui3/ .

Gate on Chinese composition/candidate placement, text selection/undo/clipboard,
Tab/focus/reader navigation, high contrast, 100/125/150/200% DPI and monitor transitions,
virtual lists, log backpressure, rendering/backend cost and clean-machine deployment.
Reject unusable input/accessibility even if package size is attractive.

## Next concrete slice
Dependency access and initial GitHub/CI delivery are now working through normal Git.
New-directory materialization and complete-stage recovery extend the earlier cache slice;
they do not provide updates to existing/shared game directories. Next: comparable UI samples,
auth with OS credential storage, Java selection and actual Vanilla launch in disposable
directories. Verify Java 8/modern Java and visible menus/worlds before scaling out loaders.
Server lifecycle feasibility also precedes full local polish. Stage B remains incomplete.

