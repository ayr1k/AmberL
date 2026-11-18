# Release discovery / 版本检查

About → Check stable releases performs an explicit anonymous HTTPS request to
`https://api.github.com/repos/ayr1k/AmberL/releases?per_page=100`. It never contacts
AMBER/AmberE update feeds, sends account/game data, polls in the background, downloads
an installer or starts one. A valid result enables the official release-page button.

Only non-draft, non-prerelease `vMAJOR.MINOR.PATCH` (or unprefixed) releases newer
than the installed version are considered. A stable version also succeeds its matching
development version. Require an uploaded `AmberL-MAJOR.MINOR.PATCH-windows-x64.exe`
asset, exact AmberL repository/tag URL, positive size at most 512 MiB and a GitHub-reported
SHA-256 digest. The UI shows version, size and digest. This validates metadata, not
installer bytes; download/installation remain explicit user actions. Network errors
never become “up to date”. The initial empty official list was checked live.

在“关于”页面手动检查正式版本，仅查询 AmberL 自己的官方仓库。草稿、预发行、
旧版本、其他产品地址以及缺失校验值的发行文件不作为可用更新。不会自动下载或
执行安装包。当前仍为开发预览，尚无正式版本时会明确显示暂无新正式版本。

Formal-release packaging must use the asset naming above and publish its matching
SHA-256 sidecar. Release publication, signing and stable installer identity are handled
only after explicit release authorization. The current Preview installer/channel stays
independent. App-local Microsoft runtimes must be serviced with future package updates.

Reference: https://docs.github.com/en/rest/releases/releases#list-releases
