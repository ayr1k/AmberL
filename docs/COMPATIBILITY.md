# Compatibility: explicit capabilities

## Current copy/ownership matrix

| Input/state | Inspect | Direct launch / modify source | Independent copy | Operations on the owned copy |
|---|---|---|---|---|
| Official/PCL/HMCL Vanilla, confirmed isolated directory | Yes | No | Known game-data files + verified official resources | Launch, content, naming/icons, removal, backup/restore |
| Same profiles, confirmed shared directory | Yes | No | Explicit shared-data acknowledgement required | Same; copied data is now independent |
| Unknown or conflicting directory evidence | Limited, with reason | No | Blocked | Not applicable |
| Modded/unknown loaders, custom aliases/patches | Inspect supported metadata | No | Not supported in this slice | Not applicable |
| AMBER-managed layout | Basic inspection | No | Not supported | Not applicable |
| Complete AmberL installation | Yes | Receipt/identity/hash/Java/account verification before launch | Full verified snapshot restoration | Supported within documented limits |

See [IMPORT.md](IMPORT.md) for the precise copied-file list, limits and synthetic
official/PCL/HMCL plus actual disposable-world evidence. No arbitrary external source
is adopted for writes. A copy does not establish modded-world compatibility.

以下保留早期只读样机的识别边界；当前副本导入能力以上表与 IMPORT.md 为准。

## Historical probe inspection matrix

These are the **initial probe's** capabilities, not the stable product contract.
External-library launch/update/restore remain unsupported. Separately, AmberL-created
Vanilla installations now support authenticated Windows x64 CLI launch; see AUTH-LAUNCH.md.

| Input | Inspect | Isolation evidence | Launch / install / modify / restore |
|---|---|---|---|
| AMBER `instances/<id>/instance.json` | Basic identity/name/game/loader | Recognized managed directory layout; ID must match | Not implemented |
| Official `.minecraft/versions` | Profile and bounded inheritance | Explicit matching profile gameDir; missing evidence unknown | Not implemented |
| PCL per-version `PCL/Setup.ini` | Same profile inspection | Recognized IndieV2 and legacy explicit values; duplicates rejected | Not implemented |
| HMCL per-version `hmclversion.cfg` | Same profile inspection, game patch label | Only explicit `usesGlobal:false` and known modes | Not implemented |
| Missing global PCL/HMCL config | Metadata where readable | Unknown; content folder names do not prove isolation | Not implemented |
| Conflicting configured directories | Metadata where readable | Conflict; no target selected | Not implemented |
| Mixed `instances` + `versions` root | Rejected as ambiguous | User decision needed | Not implemented |
| Forge/NeoForge/Fabric/Quilt coordinates | Loader label only | Independent of loader label | Not implemented |
| Custom entrypoint/unknown loader | Unknown label | Do not infer vanilla | Not implemented |
| Legacy game arguments with gameDir | Row reports unsupported | No guessing quoted legacy paths | Not implemented |
| Junction/symlink/reparse/parent traversal | Conservatively rejected | No external path adoption | Not implemented |
| Server/FRP library | Not implemented | Not implemented | Not implemented |

Limits: 8 MiB metadata files, 16 inheritance levels, 20,000 profile directories, bounded
JSON parsing. Scanning can be cancelled between filesystem operations, but a blocked
OS filesystem read is not promised to interrupt immediately. Windows app-only registration
supports up to 256 libraries; missing/replaced roots fail when opened, retaining registration.
The read-only path strategy does not guarantee isolation against hostile concurrent path
replacement. Registered scans additionally guard the root and its ancestors by handles;
individual game metadata remains read-only. External-library writes remain disabled; new isolated installation/session capabilities
do not authorize modifying an attached external library. Registry detach is not game deletion.

## Metadata planning only

Standalone Vanilla `net.minecraft.client.main.Main` metadata supports ordered Mojang
rules, explicit SHA-1/size download descriptors, current-platform libraries, legacy native
classifiers, modern argument arrays, unquoted legacy arguments and log4j2 descriptors.
Limits: 4,096 libraries/arguments, 32,768 bytes per argument, 100,000 asset objects.
The CLI uses Windows x64/Java `amd64` rules with an explicitly supplied OS version.
Unknown Java requirements remain unknown. Inheritance/loaders, quoted legacy arguments,
Maven-only libraries and custom entrypoints/extraction rules are explicitly unsupported.
Asset hash deduplication preserves logical names and virtual/resource layout flags.
No extraction, installation, argument substitution, credential handling or launch is implied.

只读计划支持原版描述与资源索引；实际安装、启动及第三方加载器兼容仍未验收。

## Stable matrix to populate

### New-directory CLI installation

Standalone Vanilla plans can now materialize verified cache resources into a new isolated
directory, including supported legacy native ZIP entries and virtual/resource asset copies.
No existing AMBER, PCL, HMCL or official-launcher library is adopted for mutation. Their
read-only rows above retain their previous boundaries. Installed metadata alone does not
supply isolation evidence to the generic external-library scanner or prove game readiness.
Unknown Java requirements remain unknown; the installer does not select or execute Java.
See INSTALLATION.md and the dated validation checkpoint for precise tested versions/limits.
Each evidence row must pin layout sample, Minecraft/loader version, isolation mode,
Java/OS/architecture and source revision. Separate columns: identify / launch / install /
content edit / component edit / recycle / backup restore. Use passed / limited /
unsupported / unverified, with exact reason and evidence. Test copies, never originals.

AMBER 1.7.1 has bounded external launch support; its loader recognition and prior test
results do not establish AmberL compatibility. Modern Forge/NeoForge module launchers
and old Java native-window behavior are explicit future test cases.


## Historical initial authenticated-launch cell

Vanilla 26.3 + explicit x64 Microsoft OpenJDK 25.0.4: actual Microsoft authorization,
DPAPI reopen, fresh entitlement/profile, visible menu, new world and normal save/exit
verified in a new AmberL installation. Java 8 and older game windows are still unverified.
Other Java majors must exactly match official metadata and need their own game acceptance.
Moved/copied instance adoption, loaders, shared/external directories and offline launch
are not supported by this path. See [launch boundaries](AUTH-LAUNCH.md).
