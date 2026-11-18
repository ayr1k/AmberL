# FRP forwarding / FRP 转发

Open **联机 → FRP** and explicitly download the pinned official Windows x64
frpc 0.72.0 component. Enter the provider's host/control port, a PEM CA certificate
that authenticates that host, a token, the local Minecraft port and the public
port assigned by the provider. Start/stop controls this FRP process independently
of the game, server and invitation room. No provider account is created by AmberL.

节点证书必须与节点主机名匹配。令牌保存在当前 Windows 用户的 DPAPI 安全存储中；
再次启动时留空可沿用同一节点与证书的令牌。更换节点或证书必须重新输入令牌。
界面启动后清空令牌输入框；配置文件与命令行不包含明文令牌。

TLS certificate validation is mandatory. The saved encrypted credential binds the
endpoint and certificate digest; editing the ordinary configuration cannot redirect
the saved token to another endpoint. The selected certificate is copied into the
profile. It is not installed into Windows' trust store. The child receives the
token through an environment-rendered JSON template, not process arguments or
plaintext configuration. Output is suppressed because upstream diagnostics could
include credentials. Other same-user privileged software is outside this boundary.

Downloads verify the official ZIP SHA-256, frpc executable and original license.
The executable is rechecked/pinned before each run. AmberL does not run frps in
normal use or modify firewall, DNS, adapters or OS security settings. Only the
selected local 127.0.0.1 port is forwarded; ports must be in 1024–65535.

**进程运行中不等于外部可达。** frpc retries after node disconnection. The UI reports
the process and leaves external reachability unconfirmed. The provider must assign
the public port and friends must use that provider's external address. Public-node
compatibility, NAT and two-device gameplay require separate acceptance.

The developer `frp_smoke` fixture uses an official pinned frps on localhost, a
temporary test-only CA and a new owned Minecraft world. Actual testing verified a
Minecraft status query through FRP, node restart and automatic reconnection,
independent FRP stop with the Minecraft server still responding, and graceful
server save/exit. This does not establish an external-network connection.

Upstream: [frp 0.72.0](https://github.com/fatedier/frp/releases/tag/v0.72.0).
