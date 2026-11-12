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
- `amberl-inspect`: independent CLI for reproducible inspection.
- `amberl-native-probe`: Win32 standard controls, virtual list, background worker.

Current identity uses canonical library path + profile name and is session-only.
Before persistence/mutation, add stable registry IDs, filesystem object identity and
handle-based revalidation. Deserialization is not permission to access a directory.
Win32 unsafe calls are confined to the UI adapter. No game writes or launches exist.

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
work, add persistent app-only library registry and safe file identity, then authenticated
Vanilla launch from a disposable fixture. Verify Java 8 and modern Java plus actual
visible menus/worlds before scaling out loaders. Auth and OS credential storage belong
early in the vertical slice. Server lifecycle feasibility also precedes full local polish.

