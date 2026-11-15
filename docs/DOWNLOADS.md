# Download queue / 下载队列

This candidate addresses serial game/Java downloads and transient failures. It is
built and packaged only at the user's request: **no download tests, benchmarks,
application launch or installer execution were performed for this revision**.

- Game assets/libraries and official Java files use at most eight workers per batch.
  Identical digests share one download; conflicting sizes and over-budget manifests
  are rejected before their batches start. Forge client seed resources use the same
  queue. The Forge installer itself still controls any requests it performs.
- Each anonymous HTTPS worker reuses a WinHTTP session; request/connection handles
  are recreated. Account traffic keeps its separate implementation and timeouts.
  No credential/cookie forwarding, TLS bypass or arbitrary redirect support is added.
- Resolve/connect/send timeouts are 15 seconds; receive inactivity is 30 seconds.
  Cancellation is checked between blocking I/O operations and during backoff; an
  in-flight OS request may therefore take up to its timeout to return.
- Transient timeouts, DNS/connect/reset errors, HTTP 408/429/500/502/503/504 and short
  bodies receive at most three total attempts with 0.5/1-second backoff. Metadata
  reads retry transient network errors too. Certificate/authentication, disk, path,
  digest mismatch and other permanent errors are not retried automatically.
- Every successful file must match its original provider size and SHA-1/SHA-256
  before an atomic cache commit. A failed partial file is discarded; verified files
  remain available to a later installation attempt. **Partial-file HTTP Range resume
  is not implemented**, and no whole-install automatic retry or mirror is added.
- The UI shows verified file/byte totals, average network throughput and retries.
  Throughput counts received payload (including retried bytes), excludes cache hits,
  and is averaged over the current batch. It is not an end-to-end measured speedup.
  Failures identify the host and digest prefix, not query strings or credentials.

本轮改为最多 8 个并发任务、复用匿名下载会话，并对临时网络故障自动重试。
已校验文件保留在缓存中；重新安装同一版本可复用。未下载完整的单个文件会重新下载，
不是断点续传。界面显示校验量、平均网络速度和重试次数。真实速度、失败恢复、取消
以及不同网络环境的兼容性均待用户手动验收，未宣称已提速多少倍。

Implementation reference: [Microsoft WinHTTP handle/session guidance](https://learn.microsoft.com/en-us/windows/win32/winhttp/hinternet-handles-in-winhttp)
and [WinHTTP connection reuse guidance](https://learn.microsoft.com/en-us/gaming/gdk/docs/features/console/networking/web-requests/intro-winhttp).
