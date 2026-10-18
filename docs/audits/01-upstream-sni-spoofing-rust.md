# §55.A 上游审计 — `therealaleph/sni-spoofing-rust` v1.0.2

只读克隆：`_upstreams/sni-spoofing-rust`，HEAD `d295602`。未做任何修改。
包名 `sni-spoof-rs`，edition 2021，**无 `rust-version` 声明**（代码用到 `LazyLock`/`is_multiple_of`，实际 MSRV ≥ 1.87）。
License **MIT**（`LICENSE:1`，`Copyright (c) 2025` 无主体名）。

## 1. 必须保留的部分（需求书 §1 的「不要推倒重写」）

| 能力 | 位置 | 为什么必须保留 |
|---|---|---|
| 伪造 TLS ClientHello | `src/packet/tls.rs:29-81` | 从真实 Chrome CH 模板拼接，固定 **517 字节**（`:8`），SNI 上限 219 字节（`:30`）。实测过 DPI 的字节形状，重写=重新踩坑。 |
| 故意错误的 TCP 序号 | `src/sniffer/mod.rs:79-80` | `fake_seq = isn+1-len(payload)`，让服务端收窗口丢弃该包而被动式 DPI 仍读到 SNI —— 这是整个 bypass 的核心机制。 |
| 注入时机判定 | `src/sniffer/mod.rs:286-305` | 抓 SYN 得 `isn`，在**握手第三个 ACK**、`payload_len==0` 时注入；`:319` 有一个 1 ms sleep。 |
| 成功/失败判据 | `src/sniffer/mod.rs:342-357`、`:374-384` | 注入后收到 `ack==isn+1` ⇒ `FakeConfirmed`；收到 `SYN|RST` ⇒ `Failed("RST from server")`。 |
| macOS BPF backend | `src/sniffer/macos.rs`（365 行） | `/dev/bpf0..99` 扫描（`:203-216`）、DLT→`FrameKind` 映射（`:91-102`，未知 DLT 直接报错）、多记录缓冲读取（`:133-185`）。需求书 §18/§24 点名「不要丢掉现有实现」。 |
| Windows WinDivert backend | `src/sniffer/windows.rs`（121 行） | 双 handle（收/发分离，`:42-56`）+ 专用泵线程 + `sync_channel(128)`；`set_outbound(true)` 与 `recalculate_checksums`。 |
| Linux AF_PACKET backend | `src/sniffer/linux.rs`（260 行） | 含硬编码 12 条 cBPF TCP 过滤器（`:164-260`）。 |
| **`RawBackend` trait** | `src/sniffer/mod.rs:20-27` | 已经是 §25/§26 想要的 `TrafficBackend` 形状：`recv_frame / send_frame / frame_kind / skip_checksum_on_send`。Himari 的策略/backend 抽象应挂在这里，而不是新造一层。 |
| 帧类型数据化 | `src/packet/mod.rs:14-28` | `FrameKind{Ethernet,NullLoopback,RawIp}` + `link_header_len()` —— 链路层差异是**数据不是 cfg**。 |
| 注册→connect 的时序 | `src/handler.rs:92-179` | 必须先注册四元组、再 `EINPROGRESS` 非阻塞 connect、再 `take_error()`；用 `socket2` 而非 `TcpStream::connect` 是**承重的**（SYN 发出前 sniffer 必须已知连接）。 |
| 配置模型 + 自环检测 | `src/config.rs:29-52`、`:85-93` | `real target / fake SNI / connect addr / listen addr / port` 已经齐了（正好是需求书 §8 要的字段）。 |
| 分享链接解析 | `src/xray.rs`（403 行，含 6 个单测） | vless / trojan / vmess(base64 JSON) / `v2rayn://` 包装；纯函数，可直接当库用。 |
| `RunningProxy` 生命周期 | `src/proxy.rs:20-56` | `request_stop / wait / stop / is_running`（`JoinHandle::is_finished`）+ `Drop` 自动 stop。**这是 §36 崩溃检测唯一真实存在的东西。** |

## 2. 现有 CLI（需求书 §51 Phase 1 要确认的「现有 CLI」）

**无 clap**，全手写 `std::env::args()`：

- `sni-spoof-rs [CONFIG]`（默认 `config.json`，`main.rs:21`）→ `run_proxy_until_signal`。
- `sni-spoof-rs scan [-t IP:PORT] [-c N] [--timeout S] [-l FILE] [-o FILE]`（`scan.rs:73-146`）。
- 未知首参数被**静默当成 CONFIG 路径**（`main.rs:17-21` catch-all）。
- **没有 `--version`**，没有 `--config`，没有 daemon/service 子命令。
- 日志只有 `RUST_LOG`，默认 **`warn`**（`main.rs:8`）。
- `scan` 解析失败 → 打印帮助 + `exit(2)`（`:57-64`）；`-o` 写失败 → `exit(1)`，**且这段在 `pub fn scan_all` 可达路径里**（`:247-250`）→ 库化时必须去掉进程退出。

`sni-spoof-rs-ui`（`required-features=["ui"]`）除 `-h` 外**无任何参数**，配置只走 `config.json` + `sni-spoof-rs-ui-state.json` + 5 个环境变量。

## 3. 现有 GUI（§5「功能对等」的基线，必须逐项对齐）

`eframe/egui 0.28`，renderer 强制 **Glow**（`ui.rs:48`），单文件 2496 行，窗口 980×860 / 最小 720×620。无 tab，用三个 `CollapsingHeader` + 上/下/中三面板。

功能清单（这就是对照表的输入，逐条见 `04-feature-parity.md`）：

- 顶栏 3 个状态胶囊：**Proxy / Xray / IP**（`:1022-1074`）+ 自由文本 status line + `Light` 主题勾选（持久化）+ GitHub 链接。
- 控制条 10 个按钮：`Start all-in-one`、`Stop all`、`Start/Stop proxy`、`Start/Stop Xray`、`Load`、`Save`、`Check IP`、`Use connect as scan target`（`:1111-1188`，各带启用门）。
- **Xray Import** 面板：4 个文本字段（xray 二进制路径 / http 代理监听 / socks 监听 / xray 日志参数）、`Local only` 与 `Share on LAN` 一键切 127.0.0.1↔0.0.0.0（`:978-991`）、**`Xray TUN mode` 实验开关**（`:1287-1297`）、多链接 ComboBox（`share_lines` 逐行 + label）、6 行多行文本框存全部链接、`Parse selected` / `Download Xray` / `Fetch my IP` / `Clear`。
- **Proxy Config** 面板：10 个字段（listen / connect / fake_sni / conn_timeout / handshake_timeout / keepalive_time / keepalive_interval / idle_timeout / buffer_size_kib / graceful_shutdown）——全部按字符串存。
  ⚠️ **对等陷阱**：`FormState::to_config` 硬写 `listeners: vec![listener]`（`:2410`）。核心支持 N 个 listener（`config.rs:34`、`proxy.rs:125-137`），**UI 只能编辑 1 个**。多 listener 目前是 CLI-only。
- **SNI Scanner** 面板：target / timeout / concurrency 三字段；`Test fake_sni`（只测当前 fake SNI）与 `Scan built-in list`；进度 `{done}/{total}, {ok} ok` + `elapsed`；结果 2 列表格（`rev().take(120)`，缓冲 `RESULT_MAX=800`）绿/红着色。
  ⚠️ 结果**纯展示**：不可选、不可排序/过滤、**不能一键变成配置**（需求书 §7「从 scanner 结果创建配置」在上游不存在）。
- **Logs** 面板（底部可拖高）：`Logging` 开关（关=清空环形缓冲）、`Clear`、`Export logs` 到 `<exe目录>/sni-spoof-rs-ui-logs-<unix_secs>.txt`；`stick_to_bottom`；**关键机制**：`tracing_subscriber` 的 `MakeWriter` 桥把全局 tracing 输出灌进 GUI 日志环（`:123-164`，UI 默认 filter `info`，比 CLI 的 `warn` 吵）→ §34「结构化日志」在这里已有雏形，但只有 `String` 行，没有 component 字段。
- 异步：**不卡 UI**，`std::sync::mpsc` + `try_recv` 轮询，三类消息 `ScanMsg / XrayMsg / IpCheckMsg`；扫描在命名线程 `"sni-scan"` 上跑**独立 tokio runtime**（`:2455-2467`）；有 `request_repaint_after(120ms)` 驱动。
- 主题：两套硬编码 13 槽 palette（浅=蓝灰 `:222-238`，深=红黑 `:239-255`）。
- 崩溃检测：`update()` 里每帧 `RunningProxy::is_running()`（`:1009-1013`）——**GUI 不会显示假 Running**，这点上游是对的，Himari 必须保住。
- Xray 进程：`Command` 起子进程，stdout/stderr 加 `"xray"` 前缀入日志环（`:638-643`）；停止是 **`child.kill()` = SIGKILL**（`:1631-1636`），无优雅关闭、无 reload。

## 4. 现有 core / backend 的真实分层程度（§46/§48 判断依据）

非测试 `cfg` 属性共 **26 处**：核心库 15 处、UI 11 处。核心里真正「选平台」的地方只有 3 处：

1. `src/sniffer/mod.rs:1,3,5` — backend 模块编译期门控；
2. `src/proxy.rs:85-95` — **唯一**的 backend 构造点（三段 `#[cfg]` 互斥 `let backend = ...`）；
3. `src/proxy.rs:207-222` — `platform_privilege_hint()` 四路字符串。

结论：**核心平台分支并不烂**（§46 担心的「不可维护大条件分支」尚未发生）。Himari 需要的动作是把 (2) 换成 `TrafficBackend` 工厂 + 加 `#[cfg(not(any(...)))]` 兜底（现在没有，所以给 FreeBSD 编 `start_proxy` 会因 `backend` 未绑定而编译失败）。真正的债在：

- UI 的 11 处平台 cfg（Xray zip 解压、Wintun 安装、TUN 校验、appdata 路径）——正好是 §48 要下沉到 `platform/` 的东西；
- `get_interface_for()` 在 linux/macos **两份近乎重复的实现**（`linux.rs:123-162` vs `macos.rs:218-254`）；
- cBPF 过滤器 12 条指令在 linux/macos **两份重复字节数组**（`linux.rs:165-238` vs `macos.rs:266-339`）——过滤器程序可移植，attach 方式不可移植，应合成一个 `tcp_only_bpf()`；
- Windows 侧 checksum **算了两遍**（`windows.rs:115` `recalculate_checksums` + `skip_checksum_on_send()` 返回 `false`）；且 `skip_checksum_on_send` 这个 hook **目前没有任何实现返回 true** —— 休眠抽象。

## 5. Scanner 现状 vs 需求书 §7 的差

| §7 要求 | 上游有没有 | 证据 |
|---|---|---|
| bulk hostname scan | 有 | `scan.rs:188-255`，内置 647 个 SNI（`data/scan-snis.txt`，`include_str!`） |
| **DNS resolution** | **没有** | `--target` 必须是 `IP:PORT`，默认硬编码 Cloudflare `104.18.4.130:443`（`scan.rs:13`）；扫描器完全不解析域名 |
| **Cloudflare detection** | **没有代码** | 只有 README 让人肉眼认 `104.*/172.67.*/…` 段（`README.md:127,:392`） |
| TLS probe | 有 | `scan.rs:281-312`：真 socket 上写伪造 CH，读 5 字节判 `buf[0]==0x16` |
| connection test | 有 | `:265-279` |
| timeout | 有，但**一个标量复用三段** | 默认 6 s ⇒ 单个 probe 最坏 ≈18 s |
| result state | 有，7 个 | `ProbeOutcome{Ok,ConnectFailed,ConnectTimeout,HandshakeFailed,ReadTimeout,BadResponse(u8),EmptyResponse}` |
| **latency** | **没有** | `probe_sni` 里没有任何 `Instant`；只有 UI 的总 `elapsed`；数据模型无字段 |
| candidate fake-SNI targets | 有 | 内置清单 + `--list` |
| **结果创建配置** | **没有** | CLI `-o` 只写「ok 列表」纯文本；UI 表格无行交互 |
| 异步不卡 UI | 有 | 见 §3 |
| 限制并发 | 有 | `Semaphore::new(concurrency)`，默认 `DEFAULT_CONCURRENCY=10`（`scan.rs:15`）。⚠️ UI **另写了 100 行重复实现**（`ui.rs:2455-2492`）而不调 `scan_all`；`scan_all` 的返回值甚至被丢弃（`scan.rs:69 let _ =`）→ Himari 应统一成一个 `Stream` |

## 6. 测试与安全面

- `#[test]` 共 **20 个**：`xray.rs` 6、`bin/ui.rs` 7（纯函数）、`packet/tls.rs` 5、`packet/tcp.rs` 2。
- **`sniffer/mod.rs`（伪造帧数学核心）、`proxy/handler/listener/relay/config/scan` 零测试** → 需求书 §50「Core: packet rewrite / relay / scanner / config validation」基本是净增。
- `unsafe` 集中：`macos.rs` 21、`linux.rs` 10、`windows.rs` 1，其余 0。
- `panic = "abort"`（`Cargo.toml:54`）+ `tls.rs:30,74-79` 两个 `assert!` ⇒ SNI 超长/模板越界会**直接杀进程**，不是杀任务。§36「崩溃恢复」必须考虑这条。

## 7. 打包/分发问题（Himari 不要继承）

- `releases/` **提交了约 87 MB 预编译二进制**（含 Xray、WinDivert `.sys/.dll`、Wintun），README 说明它是给被墙用户的镜像 —— 「可 clone 拿到二进制」这个**能力要保留**，把二进制放进 git **这个做法不要保留**。
- 三个互不相同的持久化根：state/logs 在 **exe 同级**（`:1540-1551`）、Xray 在 appdata（`:1611-1617`）、`config.json` 在 **CWD**（`:349,853,869`）→ 对 `.app` bundle 和 Windows 安装包三者都不合法。§40/§41 必须重设。
- `docker/entrypoint.sh` 用 shell **第四次**重写分享链接解析（`:25-52`）——Himari 只留一份 Rust 实现。
- `Makefile` 用 `-x64`、CI/`releases/` 用 `-amd64`，命名不一致。
