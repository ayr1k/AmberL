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
- `amberl-inspect`: independent CLI for reproducible inspection.
- `amberl-native-probe`: Win32 standard controls, virtual list, background worker.

Unregistered inspection uses canonical path + profile name. Registered scans use a GUID
plus profile name, and reopen/revalidate the stored volume/index/creation identity.
Registry scans keep non-delete-sharing handles to the directory and its ancestors.
All paths are checked for reparses before use; this conservative implementation can reject
otherwise legitimate linked libraries. Actual game contents are still read-only and are
not locked against external launchers. Deserialization does not authorize game mutations.
Win32 unsafe calls live in the UI and Windows registry adapters. No game writes/launches exist.

The registry holds a non-delete-sharing lock file with an exclusive OS lock; concurrent
writers fail fast. Updates write/sync a unique new file before same-directory replacement.
Uncommitted temporary files are ignored. Invalid/unknown schemas are preserved, not reset.
This is app configuration persistence, not a journal/backup engine for game files. Third-party
writers, hostile same-user processes and power-loss/storage failures are not fully modeled.
Only the local Windows filesystem used in the recorded tests is verified; network, ReFS,
removable filesystems and cross-machine directory identity need separate evaluation.

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
Resolve dependency access and GitHub delivery; compare UI samples. In parallel with UI
work, extend identity/transactions to actual game resources, then authenticated
Vanilla launch from a disposable fixture. Verify Java 8 and modern Java plus actual
visible menus/worlds before scaling out loaders. Auth and OS credential storage belong
early in the vertical slice. Server lifecycle feasibility also precedes full local polish.

