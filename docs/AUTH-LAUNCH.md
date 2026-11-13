# Microsoft accounts, Java and Vanilla launch

This Stage B slice includes a Windows x64 core/CLI and an integrated native launcher
preview. See [native UI](NATIVE-UI.md) for graphical account, Java, installation and
launch controls. The original scan/comparison windows remain separate; final
framework acceptance remains open.

## Try it / 测试步骤

Use an existing, dedicated AmberL app directory and a complete **new-directory**
installation produced by `install-vanilla`. Do not pass an original game library.
For example, resolve, prepare and install Vanilla 26.3 using the earlier installation
commands; select a working x64 Java 25 yourself. Java is not downloaded automatically.

```text
amberl-inspect accounts <app-directory> login
amberl-inspect accounts <app-directory>
amberl-inspect accounts <app-directory> verify <account-id>
amberl-inspect java-list
amberl-inspect java <chosen-java.exe>
amberl-inspect check-launch <app-directory> <game-directory> <version> <chosen-java.exe> <account-id> <memory-MiB> <windows-os-version>
amberl-inspect launch-vanilla <app-directory> <game-directory> <version> <chosen-java.exe> <account-id> <memory-MiB> <windows-os-version>
amberl-inspect accounts <app-directory> remove <account-id>
```

登录时，在微软官方页面自行输入程序显示的设备码；不要向他人提供密码、令牌。
账户列表返回 Minecraft 角色与 ID。选择对应 ID、Java 路径、内存上限（例如 4096 MiB）
和实际 Windows 版本（例如 10.0.26100），先执行 `check-launch`，再执行 `launch-vanilla`。
Java 主版本必须匹配官方元数据，且为 x64；没有声明 Java 要求的版本暂不启动。
`java-list` 只列出标准位置的候选，不自动替换用户选择。选择目前通过每次 CLI 参数传入，
尚无持久化的实例 Java 偏好设置。Java 8/21/25 的解析不等于所有版本的游戏验收。

Close the game normally. Ctrl+C asks the owned visible game window to close and waits
for its exit, keeping instance/resource handles held. If cancellation precedes window
creation, the request is delivered when its visible window appears. An unresponsive or
windowless child can keep the CLI waiting; there is no automatic forced shutdown timer,
tray/detach policy, unrelated-Java termination or server supervision in this slice.

## Authentication and storage boundary

- Reuses only AMBER's public application ID/authority/scopes. No other product's
  account file, token, runtime, settings or library is imported.
- Device flow respects server polling intervals, pending/slow-down, cancellation,
  expiration and denial. Production credential requests use fixed Microsoft, Xbox
  and Minecraft endpoints with OS TLS; redirects, cookies and automatic logon are off.
- Xbox/XSTS and Minecraft login must succeed; entitlement and profile are checked
  before storing a new account and again before each game start. There is no offline
  ownership fallback or silently selected replacement account.
- `accounts-v1.dpapi` contains only DPAPI-encrypted refresh tokens and profile metadata.
  Protection is Windows-current-user scoped, with application-specific entropy and
  `CRYPTPROTECT_UI_FORBIDDEN`; no plaintext fallback. Access tokens stay in memory.
  Temporary save files contain ciphertext only. Account locks, pinned directories,
  link checks, flush and handle-based replacement protect updates. Broken/unknown
  records are preserved. OAuth refresh rotation is saved before downstream requests.
- DPAPI normally binds the data to the same Windows user/machine. It is not protection
  against another process running as that user or an administrator. Copying a portable
  launcher directory to another computer does not migrate the protected account.
- Remove forgets local login material; it does not revoke Microsoft's server-side grant,
  log out an already running game or change another product's account.
- Credentials are excluded from errors/progress/Debug. Argument buffers and main secret
  buffers are zeroized on drop where practical; this is not a complete process-memory
  erasure guarantee. Vanilla receives its access token in the standard process arguments,
  which privileged/local process inspection can see. No plaintext argument file is made.
  Captured output keeps a bounded tail, redacts the full game token across read boundaries,
  and omits oversized lines. The game itself owns its logs/crash files.

## Launch integrity and actual scope

The official catalog and metadata are resolved again before launch. Installed metadata
must equal the selected official metadata. Receipt hashes, file types, sizes, physical
directory identity and official resource hashes are rechecked. Immutable files and their
directories remain pinned through the owned child lifetime. The actual instance session
lock rejects a second AmberL session. Existing destinations are not adopted by name;
moved/copied installations require a future explicit migration/recovery feature.

Existing mutable game content is checked for reparse points/hardlinks. Ordinary new
options, logs and saves are allowed on subsequent runs. These checks do not sandbox
Minecraft or defeat a hostile concurrent same-user filesystem writer. External-library
attachment stays read-only; launching arbitrary existing/shared libraries is unsupported.

Arguments remain separate OS arguments. Unknown placeholders, unexpected Java major/
architecture, invalid memory and excessive Windows command length fail before game spawn.
Official optional `default-user-jvm` tuning is not applied; the explicit user memory and
required `jvm` arguments are used. Environment Java-option injection variables are removed.
Only standalone Vanilla Main is supported; loaders, servers and FRP remain out of scope.

`Started` proves process creation only. Core readiness remains `unknown`; human-observed
menu/world evidence is recorded separately. Exit reports the owned process ID/code and
bounded diagnostic tail. The prototype is not the final launcher or Stage B completion.

References: [Microsoft device flow](https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-device-code),
[Windows CryptProtectData](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata),
[CryptUnprotectData](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptunprotectdata).
