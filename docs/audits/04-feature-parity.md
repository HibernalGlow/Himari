# §55.D 功能对照表 — Current upstream feature → Himari implementation → Status

目的（需求书原文）：**确保任何现有功能不会无意丢失**。
状态口径（四档，不许含糊）：

- `保留` — 上游已有，Himari 直接沿用（走 §2 的 upstream 同步，不重写）。
- `净增` — Himari 要新写（上游没有，或上游只有 UI 私有实现无法复用）。
- `缺口` — 上游有但**当前不可达/不可复用**，需先改上游或加 patch，否则就是隐性丢功能。
- `待定` — 需求书写了但与上游事实冲突，见 `00-index-and-open-decisions.md` 的 D 编号。

## A. sni-spoofing-rust 核心能力

| # | 现有功能 | 上游证据 | Himari 落点 | Status |
|---|---|---|---|---|
| A1 | 伪造 TLS ClientHello（517 B 模板） | `packet/tls.rs:29-81` | 直接依赖上游 `packet` | 保留 |
| A2 | 故意错误 TCP 序号 | `sniffer/mod.rs:79-80` | 同上 | 保留 |
| A3 | 第三 ACK 注入时机 + 1 ms sleep | `sniffer/mod.rs:286-305,:319` | 同上 | 保留 |
| A4 | FakeConfirmed / RST 判据 | `sniffer/mod.rs:342-357,:374-384` | 映射进 §35 runtime state | 保留 |
| A5 | TCP 转发（listener→connect） | `listener.rs` `handler.rs` `relay.rs` | 上游库调用 | 保留 |
| A6 | **`RawBackend` 抽象** | `sniffer/mod.rs:20-27` | 作为 §25/§26 `TrafficBackend` 的挂载点 | 保留 |
| A7 | macOS BPF backend | `sniffer/macos.rs`（365 L） | `platform/macos` 只做生命周期，**不动实现** | 保留 |
| A8 | Windows WinDivert backend | `sniffer/windows.rs`（121 L） | 同上 | 保留 |
| A9 | Linux AF_PACKET backend | `sniffer/linux.rs`（260 L） | 后续平台，不进核心分支 | 保留 |
| A10 | 帧类型数据化（Ethernet/Null/RawIp） | `packet/mod.rs:14-28` | 保留，禁止改成 cfg 分支 | 保留 |
| A11 | TCP keepalive（双向 11 s/2 s） | `handler.rs:181-188` | 保留 | 保留 |
| A12 | 优雅停机（`CancellationToken` + drain） | `proxy.rs:20-56` `listener.rs:108-112` | controller 直接复用 `RunningProxy` | 保留 |
| A13 | 进程内存活探测 `is_running()` | `proxy.rs:44-49`（`is_finished`） | §36 崩溃检测的第一手依据 | 保留 |
| A14 | 配置模型 + 校验 + 自环检测 | `config.rs:29-52,:85-93` | §8 的表单直接映射到它 | 保留 |
| A15 | 分享链接解析 vless/trojan/vmess/v2rayn | `xray.rs`（403 L，6 单测） | `core/config` 复用 | 保留 |
| A16 | 内置 647 个候选 SNI 清单 | `data/scan-snis.txt` | **不继承别人的数据**：Himari 自建扫描产出（§43） | 净增 |
| A17 | `releases/` 可 clone 取二进制（被墙镜像） | `releases/`（约 87 MB） | 保留**能力**（改走 GH Release + 校验下载），不保留**做法** | 净增 |
| **A18** | **`sniffer/proto/handler/listener/relay/shutdown` 是私有 mod** | `lib.rs:3,4,6,8,10,11` | 外部 crate **无法实现/选择 backend** | **缺口 → D-4** |
| **A19** | **`skip_checksum_on_send` 无人返回 true；Windows 双算 checksum** | `sniffer/mod.rs:24-26` `windows.rs:86-88,:115` | 休眠抽象；接新 backend 前要先定语义 | 缺口 |
| **A20** | **backend 选择无 `cfg(not(...))` 兜底**（其它 OS 编译失败） | `proxy.rs:85-95` | §46 要求 core 不留不可维护分支 | 缺口 |
| A21 | `server_isn` 写了不读（死状态） | `sniffer/mod.rs:31,:363` | 记为上游可清理项，不阻塞 | 上游债 |
| A22 | `panic=abort` + `tls.rs` 两处 `assert!` | `Cargo.toml:54` `tls.rs:30,:74-79` | §36 恢复要把「SNI 超长杀进程」挡在入口校验 | 净增 |

## B. 上游 GUI 功能（§5「基本达到功能对等」的清单）

| # | 现有控件/功能 | 上游证据 | Himari 页面 | Status |
|---|---|---|---|---|
| B1 | 顶栏三状态胶囊 Proxy / Xray / IP | `ui.rs:1022-1074` | 首页 §30 | 净增（Iced 重写） |
| B2 | 自由文本 status line | 同上 | 保留 | 净增 |
| B3 | 明/暗主题勾选并持久化 | `:222-255,:1004` | §32 + 系统主题检测 | 净增 |
| B4 | GitHub 链接 | `:1068-1071` | About | 净增 |
| B5 | **`Start all-in-one`**（Xray+bypass+本地代理一条流程） | `:700-708` | 首页主按钮 | 保留能力/净增实现 |
| B6 | Stop all / Start·Stop proxy / Start·Stop Xray | `:1111-1188` | 首页 + Advanced | 同上 |
| B7 | Load / Save config | `:852-877` | Settings | 同上 |
| B8 | **Fetch my IP / Check IP**（§10） | `:669-693` | 首页 | 保留能力/净增实现 |
| B9 | `Use connect as scan target` | `:1179-1185` | Scanner | 同上 |
| B10 | Xray 4 字段（binary / http listen / socks listen / xray log） | `:1256-1274` | Advanced | 同上 |
| B11 | **`Local only` ↔ `Share on LAN`（127.0.0.1 ↔ 0.0.0.0）** | `:978-991` | §11 LAN Sharing | 保留能力/净增实现 |
| B12 | **Xray TUN mode 实验开关** | `:1287-1297,:2166-2184` | §12，必须标 Experimental | 保留能力/净增实现 |
| B13 | 多链接 ComboBox + 6 行多行文本存全部链接 | `:1311-1327` | Settings/Import | 同上 |
| B14 | Parse selected / Download Xray / Clear | `:1329-1360` | 同上 | 同上 |
| B15 | Proxy Config 10 字段 | `:1203-1241` | §8 普通用户只看 4 个 | 同上 |
| B16 | Scanner 三字段 + `Test fake_sni` + `Scan built-in list` | `:1386-1418` | §7 | 同上 |
| B17 | 扫描进度 `{done}/{total}, {ok} ok` + `elapsed` | `:1419-1431` | 同上 | 同上 |
| B18 | 结果 2 列表格、绿/红、`rev().take(120)`（缓冲 800） | `:1437-1460` | 同上 | 同上 |
| B19 | **日志环 + `tracing` MakeWriter 桥 + Export logs + Clear + Logging 开关** | `:123-164,:1466-1505` | §34（分 5 类 component） | 保留能力/净增实现 |
| B20 | 异步不卡 UI（mpsc + `try_recv` + 独立 tokio runtime + 120 ms repaint） | `:429-533,:2455-2467,:1014-1020` | §7 硬要求 | 净增（Iced `Command` 模型重写） |
| B21 | 每帧 `is_running()` 防假 Running | `:1009-1013` | **§24/§36 明令不许假 Running，必须保住** | 保留能力 |
| B22 | 单实例：**无** | — | §39 | **净增** |
| B23 | 托盘 / macOS 菜单栏：**无**（`rg tray\|menubar src/` 零命中） | — | §33 | **净增** |
| B24 | 通知 / 原生文件对话框：**无** | — | §4 | **净增** |
| **B25** | **多 listener：核心支持 N，UI 只能编辑 1** | `ui.rs:2410` vs `config.rs:34` | §5 可配置监听地址 | **缺口**（上游限制，Himari 必须补 UI 且别丢能力） |
| **B26** | **扫描结果 → 创建配置：不存在** | 表格无行交互；`-o` 只写 ok 文本 | §7 明确要求 | **净增** |

## C. 需求书要求但上游完全没有（净增清单）

| 条款 | 要求 | 现状 | 落点 |
|---|---|---|---|
| §9/§35 | 结构化 runtime state、禁止解析 stdout | **零 IPC**（两上游都已复核） | `core/runtime` 状态机 + §49 IPC |
| §4/§33 | 托盘 / 菜单栏 | 无 | `platform/*`，Iced 只管 UI |
| §20/§21 | GUI 不整体 root + privileged helper | 无（CLI 直接要权限；sni-gate 的 macOS 信任安装 shell out + root） | `platform/macos` helper + 一次性授权 |
| §22 | macOS 系统代理**保存/恢复**（HTTP/HTTPS/bypass/PAC/service） | 无 | `platform/macos` |
| §23 | 网络变化重检测接口（不许永久缓存 interface name） | 上游每次启动解析一次 | `platform/*` + 事件驱动 |
| §24 | BPF open/config/start/stop/close/reopen 生命周期 + fd 失效 | 上游有 open/使用，**无 reopen** | `platform/macos` |
| §27/§28 | Strategy 抽象 + Automatic + 结果缓存 | 上游只有 FakeSni 一条硬编码路径 | `core/strategy`，挂在 `RawBackend`/`fake_payload` 注入点 |
| §29 | Diagnostics 页面（✓/⚠/✕ + 解释原因） | 无 | `ui/pages/diagnostics` |
| §13 | `DnsBackend` 抽象 | sni-gate 已有 DoH/DoT/明文 resolver；**系统代理侧无** | `core/dns` + `platform/*` |
| §37 | 系统状态恢复（Proxy/DNS/PF/Routes/Helper/Services） | sni-gate **无卸载 API**（已复核） | `platform/*` 必须自带逆操作 |
| §38 | LaunchAgent/LaunchDaemon / 开机启动 / 重复实例防护 | 无 | `packaging` + `platform/*` |
| §40/§41 | `.app`+`.dmg`、portable+installer；签名/ hardened runtime / notarization / helper 与 bundle 内二进制签名关系 | 上游只有 tar.gz/zip，无签名 | `packaging/*` |
| §44/§45 | UI 明说 TCP/HTTPS ✓、QUIC/UDP △，不把「阻断 UDP」宣传成 QUIC bypass | 上游 README 有提示，UI 无 | 首页 + Advanced 文案 |
| §42 | license/version/attribution/redistributability 台账 | 无（二进制直接进 git） | `THIRD-PARTY.md` + CI 门禁 |
| §50 | 测试矩阵 | 上游 20 个测试，**核心伪造帧/relay/scanner/config 零测试** | `tests/` 分 5 层 |

## D. sni-gate 侧能力（§1 列的 11 项）与 Himari 的控制面

| sni-gate 能力 | 证据 | Himari controller 要做的事 |
|---|---|---|
| SNI routing | `router.rs:133-178` | 生成 `match_sni`，遵守 `exact>wildcard>suffix>regex` 语义 |
| Host routing | 同上（HTTP `Host` peek，`peek.rs`） | 同上，且 http 路由禁写 `[verify]`（`config.rs:3080-3125`） |
| TLS termination | `proxy.rs` + `main.rs:491-509`（4 种 ALPN） | 只管启停，不碰 |
| dynamic per-SNI cert | `resolver.rs:265-288,:385-470` | CA 路径注入 + 信任根安装/卸载（**卸载要 Himari 补**） |
| HTTP upstream | `RouteType::Http` | 模板/继承生成时别漏 `type` |
| raw TCP upstream | `RouteType::Raw`（不终止 TLS；禁 `cert_file`） | 同上 |
| ECH | `ech.rs`，`mode=doh\|static\|doh-with-fallback`，`require_ech` 默认 **true** | DoH bootstrap 需固定 IP（见 Github-direct 的 pinned DoH） |
| HTTP/2 | `Http2Config`（`enabled` 默认 false，**耦合开关**，raw 上显式 true = 错误） | UI 上别做成「入 h2 出 h1」 |
| IPv4/IPv6 racing | `address_family` + `6094aa2` dual-stack 竞速 + `nat64.rs` | 默认 `dual`，Advanced 暴露 |
| certificate cache | `[cache]` capacity 8192 / ttl 86400 + `store.rs` 原子落盘 | 只暴露，不重写 |
| multiple listeners | `[[listener]]` 必填非空；同址重复启动即报错 | **跨进程重复实例 §39 必须靠 Himari 自己的锁**（上游无 pidfile/lock） |

## E. 未决项映射（勿在本表里替用户定）

D-1 仓库身份 / D-2 应用层代理归属 / D-3 GUI 按重写估 / D-4 私有 mod（阻塞 Phase 2）/ D-5 IPC 零基础 / D-6 信任根无逆操作 / D-7 WinDivert 等再分发许可 / D-8 默认 `0.0.0.0`。
全部详见 `00-index-and-open-decisions.md`。**§51 Phase 2 开始前至少 D-1 与 D-4 必须有答案。**
