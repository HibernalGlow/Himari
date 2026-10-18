# §55.C 参考实现归类（9 仓，只读）

克隆位置 `_upstreams/`（shallow）。**许可证一栏由我本人 `head -1 LICENSE/COPYING` 复核，不是转述**；标注 ⚠️ 的两条是本篇最高影响结论。

## 0. 许可证门禁表（决定「能搬代码」还是「只能读设计」）

| 仓库 | 语言 / 构建 | 许可证（实测） | 能否取代码 |
|---|---|---|---|
| ⚠️ **SNIBypassGUI** | C++17/20 Win32 / CMake | **Proprietary License v3.0（2026-08-26，Racpast）** | **绝对不能**。原文：**not** an open-source license，源码「published for **transparency and security verification only**」 |
| ⚠️ **SpoofGUI** | C# .NET 10 WinUI3 + C++ launcher + Inno | **GPL-3.0** | **不能**（闭源 Himari 会继承 copyleft）；只读设计 |
| **GoodbyeDPI** | C / mingw + WinDivert | Apache-2.0 | 可，需 attribution + NOTICE |
| **ByeDPI** | C（`ciadpi`） | MIT | 可 |
| **darkware-zapret** | Swift 6 / `Package.swift` | MIT（内含 zapret 亦 MIT） | 可 |
| **SpoofDPI** | Go 1.26 | Apache-2.0 | 可 |
| **zapret-for-mac** | Go 1.26（仅 2 个依赖） | MIT | 可 |
| **SNISPF** | Python 3.8+ / asyncio | MIT | 可 |
| **sniproxy** | C / autotools + libev | BSD-2-Clause | 可 |

⇒ 需求书 §15/§43 的禁令**在法律上成立且有牙齿**（SNIBypassGUI 明确非开源）。§42 的台账必须把「可读不可搬」与「可搬需声明」分开记。

## 1. 五桶归类

| 桶 | 仓库 | 一句话理由 |
|---|---|---|
| **Integration reference** | SNIBypassGUI、(Github-direct 见 `05`) | SNIBypassGUI 是唯一已把「GUI + `sni-gate` 子进程 + TLS 终止代理 + DNS 协同」当**一个栈**编排好的项目 |
| **GUI reference** | SpoofGUI | Windows 桌面外壳最完整：先提权后进 UI、日志、诊断、扫描器、更新交接、托盘 |
| **Platform reference** | GoodbyeDPI（Win）、darkware-zapret（macOS） | 平台侧接管/服务/清理/卸载的产品化管路 |
| **Core reference** | SpoofDPI、zapret-for-mac、sniproxy | 数据面核心形状：可移植 packet 接口、listener/table 对象模型、热重载 |
| **Strategy reference** | ByeDPI、SNISPF | 策略抽象本体（ByeDPI）与**验证器**（SNISPF，别人都没有的那一半） |

**本篇最重要的单条结论：`zapret-for-mac` 是本组对 Himari 价值最高的仓库**，因为它把 §46/§48 想要的 core↔platform 切口**自己说出来并实现了**：核心是 transport-agnostic 的 desync 引擎，同一份策略文件同时驱动 packet 级与 socket 级通路，由 `Caps` 决定该通路能兑现哪些算子。

## 2. 各仓「值得学的机制」（只取行为，逐条带证据）

### SNIBypassGUI（行为参考，禁代码）
- **子进程零参数启动，用 working directory 当配置通道**：把 `-p "<dir>"` 传给子进程会走 **ANSI argv**，非 ASCII 安装路径在往返 UTF-16 时被摧毁 ⇒ 进程在打开错误日志前就死（`services.cpp:490-497`）。**对 Himari 的 Rust↔sni-gate 边界是同一条真实陷阱**（我们已记录 sni-gate 的 `-c` 默认值相对 CWD）。
- **严格启动序 + 全有或全无回滚**：DNS 服务前置检查 → 端口冲突探测/清占 → nginx → sni-gate → 载规则并启动 → 热重载 → 刷解析缓存 → 装守护；任一步失败调 `StopLocked`（`services.cpp:703-804`）。
- **每个子进程放进自己的 job object**，孤儿清理**按完整镜像路径匹配**：手工启动的同名进程会被停，别人家的同名二进制不受牵连（`process.h:91-106`、`services.cpp:525-531`）。
- **CA 归 sni-gate 生成，GUI 只负责移除**，注释写得很直白：卸载后留一张受信任根 = 机器上一个没有软件为证的签名者（`services.cpp:845-851`）。移除按 **subject 或 issuer CN** 匹配，且**同时扫 LocalMachine 与 CurrentUser**，并处理「删除会使枚举失效」的循环（`:874-888`）。→ **这正是 `02-gate` §3 里 sni-gate 缺失的卸载能力的行为规格。**
- **活性 = 一次内核等待，不是轮询**；停止序：解除守护 → 递增 generation → 撤 DNS 策略 → 刷缓存 → 终止子进程 → 等端口释放（`redirector.cpp:306-317`、`services.cpp:534-555`）。
- **自动启动走 Task Scheduler COM 而非 `schtasks.exe`**：把路径拼进命令行会经控制台代码页丢非 ASCII；且托盘每次展开都要问「自启开了吗」，不能每次回答起一个进程（`autostart.h:20-40`）。
- 第三方许可**随包声明**的做法可学：`resources/payload/licenses/nginx/LICENSE` 与二进制放一起（**做法采纳，文件不搬**）。

### SpoofGUI（GPL，只读设计）
- **进 UI 之前先提权**：csproj 关掉生成的 main，好让 `IsRunningAsAdministrator → RelaunchAsAdministrator` 在 WinUI bootstrap 前跑；**同一个二进制还带 `--health-check` CLI 模式**（attach 父控制台打诊断）——这条对 Himari 极便宜且有用（`Program.cs:19-31,55-94`）。
- **子二进制先离线自检再启动**：`xray run -test -c <cfg>` 作为起进程前的 pre-flight（`XrayCoreService.cs:61-94`）。→ 直接对应 `02-gate` §2 里「sni-gate 没有 `--check-config`」的缺口：要么用等价试跑，要么向上游要这个开关。
- **系统代理归属判定**：区分「代理开着且是我的」与「代理开着但别人占着」（`SystemProxy.cs:77-104`）。§22 保存/恢复的前置条件。
- **就绪策略与 SNIBypassGUI 相反**：`WaitForListener(host,port,timeout)` 100 ms TCP 轮询。**两者不可调和，Himari 必须选一条并写明理由**（见 §4 裁定）。
- **WinDivert 不打包，按需下载并钉版本**（`2.2.2` / `2.2.2-A`，官方 release 取），安装脚本明确**排除**它（双用工具 + 临时解压被 AV 盯）。→ D-7 的一个已验证先例。
- ⚠️ **反面教材**：构建时抓 xray(MPL-2.0)/sing-box(Apache-2.0)/wintun/WinDivert(LGPL-2.1)，但脚本与 `.iss` 里**没有任何 NOTICE/第三方许可产出步骤**。§42 不能做成这样。

### GoodbyeDPI（Apache-2.0，Windows 平台参考）
- **两类句柄**：主动 intercept/modify 与被动 `WINDIVERT_FLAG_DROP` 分开（含专门的 QUIC 阻断过滤器）（`goodbyedpi.c:199-200,1132-1151`）。⇒ Himari 的 backend 抽象要把「观测/丢弃」与「拦截/改写」建成两种能力，而不是一个句柄。
- **过滤器是一个模板字符串**，由 CLI 开关增补子句、占位符最后一次性定型（`add_filter_str`/`repl_str`/`finalize_filter_strings`，`:202-266`）⇒ GUI 能展示「这一套会拦什么」，对 §29 Diagnostics 是刚需。
- **编号 profile 是原子开关的累积组合**（`-1..-9`，`case` 落穿；裸跑默认 `-9` 集，`:691-742`）⇒ §27「profile = 具名预设盖在正交原子上」的成熟形态。
- **TTL/跳数是真连接表**（只看 SYN/ACK，30 s 清扫，key 含 NUL 故自定义 strlen，`ttltrack.c:1-30`）。
- **服务包装复用同一个 `main()`**，且在 `StartServiceCtrlDispatcher` **之前**存 `argc/argv`（服务参数不来自命令行），停止时走与 Ctrl-C **同一条** `deinit_all()` ⇒ §37 网络恢复只有一处出口（`service.c:14-47,78-84`）。
- **构建加固当策略**：`-pie -fPIE -D_FORTIFY_SOURCE=2 -fstack-protector` + 极凶的警告集 + `--dynamicbase --nxcompat`；WinDivert 头/库来自外部相对路径 ⇒ **驱动版本是构建期钉**，必须记进台账。

### ByeDPI（MIT，策略参考本体）
- **策略是 `Vec<Part>` 不是 `enum`**：每个 part 自带 method/position/repeats/skip，一条连接内可以 split→fake→disorder 交替（`params.h:55-86`、`desync.c:587-601`）。
- **位置用语义锚点而非字节下标**：`OFFSET_SNI / HOST / MID / END / RAND / START`（`params.h:25-30`，CLI `1+s`/`+h`）。⇒ 「在 SNI 处切」换主机名还成立；「第 16 字节切」不成立。**本组最高单条价值。**
- **`tlsrec` 是记录层重写**：原地插入 5 字节重新切 TLS 记录边界，不动握手字节（`desync.c:485-517`）。
- **策略对象自带选择器与优先级**（hosts/ipset/端口范围/proto/detect/rounds + `id/bit/pri`），**一条命令行编码 N 个策略**，`-A/--auto` 触发分组（`params.h:94-174`、`main.c:597-613,854-865`）⇒ §27/§28 的 per-domain 策略分配就是这个形状。
- **检测驱动重试 + 可持久化缓存**：`fail_count`/`pri`/`cache_file`/`cache_ttl`（`params.h:130-134`）⇒ §28 的 `target/strategy/success/latency/timestamp` 表有现成先例（注意它**不上传**任何数据）。
- **能力缺失的开关不进 `--help`**（`FAKE_SUPPORT`/`TIMEOUT_SUPPORT` 编译期门，`params.h:21-24`）⇒ 纪律要学，但 Himari 用**运行时 `Caps`** 而非 `#ifdef`。

### zapret-for-mac（MIT，本组最高价值）
- **`Caps` × `Op.Requires()` + `OnUnsupported` 降级策略**（run / degrade / skip / fail）（`desync/types.go:20-50`）：`FullCaps()`（utun+BPF）与 `ProxyCaps()={Segment,TLSRec,DropOriginal}`（"byte-level tricks only"）。⇒ §25/§26/§27 要的抽象**这里已有实现范式**，且补上 ByeDPI `#ifdef` 做法缺的那一层。
- **pflog 是无损接管原语**（`DLT=117`，PF 在施加 block 判定**之前**记录），并记着一颗地雷：**新 clone 的 pflog 是 DOWN 的**——`tcpdump` 会顺带把捕获接口拉起，所以 shell 探针能过而直读 BPF 一个包都看不到（`transport/divert/pflog.go:16-58`）。**Himari 不读这段就会重新踩一遍。**
- **`DIOCNATLOOK` 需要自带 `pfvar`**：Apple 把 `<net/pfvar.h>` 从 SDK 删了，但 `/dev/pf` 仍应答；危险在于**字段偏移写错会返回「一个看起来合理的错 IP」**，代理会忠实地连到错的服务器。三道防线：ioctl 请求号里编码 `sizeof(struct)`（错则 `ENOTTY` 大声失败）、编译期 size/offset 断言、响应侧校验（族回显、端口非零、地址合法）（`internal/pfvar/pfvar_darwin.go:4-45`）。→ §24/§18 的 macOS 硬知识。
- **变更日志账本**：字节精确备份 + **fsync 的 Journal 先于任何改动落盘** → 候选配置在副本上离线校验（`pfctl -n -f`）→ 原子写保留 mode/owner → 回读验证，不符即恢复。**只删自己 marker 包围的块，绝不整份覆盖第三方改过的文件**（`netcfg/journal.go:1-25`）。⇒ **这就是 `07` §3 我设计的 write-ahead 账本的成熟先例，可直接对照收紧。**
- **死亡开关做成独立 launchd job**，因为守护进程看不见自己：用 daemon 的 **flock** 试探——抢不到说明有人拥有 anchor，什么都不做；抢到说明没人收尾，就 flush anchor 并释放 pf 引用。理由写得准：**`KeepAlive` 在「作业被用户在登录项里关掉」时无能为力，对「卡死而非退出」更是一点用都没有**（`cmd/zapretd/guard.go:18-38`）。
- **launchd 现况事实**：Ventura 起**每个 LaunchDaemon 都出现在系统设置 ▸ 登录项**，按签名身份显示；ad-hoc 签名就显示为无名/程序名；**默认是启用的**（是披露不是审批门），但用户可关掉且**该开关跨重启存活 ⇒ 「神秘不再自启」的第一检查点**。`load/unload` 已废弃，正确写法 `bootstrap system` / `bootout system/<label>`，且**会大声失败**（`internal/launchd/launchd.go:1-41`）。→ §19/§38 直接引用。
- **签名口径一段注释钉死**：全程无需 Apple 开发者账号，链接器 ad-hoc 签名即满足 arm64；**不需要关 SIP、不需要 kext、不需要 NetworkExtension entitlement、不需要重启**（`Makefile:1-6`）。→ §41 的「最小可运行基线」应作为发布门禁测试固定下来。
- **注入要预留头部余量**：所有重发封包自己写、直投链路层，内核**不会替分片**；`injectHeadroom=24` 覆盖 frag(8)+2×HBH(16)，`minTunMTU=1280`（`transport/divert/divert.go:49-62`）。§24 的 low-overhead path 细节。
- **装之前先探测**：`probe` 按子系统四个独立探针（bpf/net/pf/utun）+ `doctor/selftest/autopick` ⇒ Himari 应有 `himari probe`。
- **用户可编辑清单与生成清单分文件**（`list-*-user.txt` vs 生成件）⇒ 再生成永不摧毁用户输入（§43 自建数据集的存放纪律）。

### darkware-zapret（MIT，macOS 产品化参考）
- **权限模型**：安装期一次性授权，之后靠 `sudoers.d` 白名单免密；提权用 `NSAppleScript ... with administrator privileges`。⚠️ **它的 `ALL ALL=(ALL) NOPASSWD:` 比需要范围宽**，且 NSAppleScript 提权不适合沙盒/MAS 分发 ⇒ Himari 应改走 `SMAppService`/`AuthorizationServices`。
- **App Translocation/隔离属性**的真实绕行（拷进临时目录再执行 + `xattr -d com.apple.quarantine`）⇒ 未签名分发必踩，§41 要提前设计。
- **PF 锚点注入的可重入写法**：把 anchor 行插在对应 `com.apple/*` 行**之前**（抢在 Apple 规则前），一次 `pfctl -f` 重载，失败就打印用户需要手加的两行；删除只删那三个自己那套 pattern（`common/pf.sh:15-63`）。
- **macOS 回环重定向陷阱已有解**：`rdr → 127.0.0.1` 后必须加 `from !127.0.0.0/8` 防自匹配，并靠 `route-to (lo0 …) user { >root }` 只把**代理自己**的非 root 出网包导出去；排除项用 PF table 不是规则串（`pf.sh:116-138`）。
- **停止序**：先撤防火墙**再**停守护（`:19-22`），避免流量打到已死的重定向目标。
- **策略 UI**：一个 enum 两引擎，两套配置习语（zapret 文本 blob vs argv 数组），加自定义策略 escape hatch 与「从暴破结果导入为可选策略」 ⇒ 正是 Himari「FakeSni vs sni-gate」的 UI 难题现成形状。
- **反面**：`RunAtLoad` 却**没有** `KeepAlive`/守护；`pkill -f` 当生命周期管理；`/tmp` 存状态与日志；`chmod 777`/`666`；**一个 1742 行的 `main.swift`**（违反我们的 <1000 行纪律）。

### SpoofDPI（Apache-2.0，核心参考）
- **一个 `Handle` 接口 + 按 build tag 分 OS 实现**（afpacket vs pcap），并把过滤器统一成 BPF 指令 ⇒ §46「平台差异封在 adapter/backend，core 里不许 `if linux…`」的正例。
- **按链路类型手搓 SYN/ACK BPF**（ethernet 与 loopback 两个 baseOffset 分支）⇒ 学路径 TTL 的最小开销捕法。
- **跳数学习明确署名来自 GoodbyeDPI** + 按目的地 LRU 缓存 ⇒ **这是「可追溯借鉴」的示范做法**，§42 台账该长这样。
- **darwin TUN 用 `FindSafeCIDR()` 搜不冲突的 /30**，而不是硬编 10.x（`tun_net_darwin.go:34-49`）⇒ §12 routing loop / VPN 冲突的正解形态。
- **规则匹配是 patricia trie**（`go.mod` 里 `kentik/patricia`）⇒ 域名/IP 规则集必须是前缀树，不是 HashSet。
- 配置层有独立的 **`warn.go`**：把「非致命建议」与「错误」分开 ⇒ Himari Settings 页要区分校验的这两级。

### sniproxy（BSD-2，核心参考）
- **对象模型**：`Listener → Table → Backend*`，`Connection` 从 Backend 选出；**Table 与 Listener 分离**才能让一张表同时服务 80(http) 与 443(tls)（`ARCHITECTURE.md:1-27`）。
- **协议是 4 字段 vtable**（name / default_port / parse_packet / abort_message）⇒ 解析策略是数据不是 switch。
- **九态连接机**，「both closed」显式存在；**靠翻转 `EV_READ` watcher 做背压**，不靠阻塞（`connection.h:38-49`、`connection.c:148,254`）。
- **`SO_REUSEPORT`** 让 N 个进程共享端口（且在不支持时优雅降级）⇒ **Himari「重启不断流」的可行路径**。
- **引用计数式热重载**：`listeners_reload(existing,new,…)` 差分，未变的 socket **复用不重绑**（`listener.h:39-67`）⇒ 对照 `02-gate`：sni-gate **没有热加载**，Himari 若要 reload 不中断，这层必须在控制器里做（或向上游要）。
- **绑定失败退避定时器而非退出**（`backoff_timer`）⇒ 用户登录时 ICS/Docker 占着 443 的正确行为。
- **先绑特权端口再 `setgid/setuid` 降权**；对自己的 DNS 弱点**坦白**（单 socket、只有 16 bit query ID、无源端口随机化、不缓存）⇒ §13/§20 的诚实文案范例。

### SNISPF（MIT，策略参考 + 唯一带验证器的）
- **`BypassStrategy.apply(client, server, fake_sni, first_data)`**，文档点明调用时机：「TCP 已连上但真实数据尚未转发之前」⇒ 策略接缝被一句话定死。
- **窗口外伪造四步，且以 ack 为确认**：抓 SYN 记 ISN → 抓第三个 ACK → 注入 `seq = ISN+1-len(fake)` → **等服务端 ack == ISN+1**，据此确认伪造被忽略、流没被破坏。第 4 步别人都省略，Himari 必须有（`raw_injector.py:1-13`）——与上游 `sniffer/mod.rs:342-357` 的 `FakeConfirmed` 是**同一机制的两侧印证**。
- **组合策略里写死一条理由**：「`prefix_fake` **不**用在真实 TCP 流上，因为它会破坏 TLS 握手」（`combined.py:1-15`），注入器是**注入进来的可选协作者**不是全局 ⇒ 「helper 在场/不在场」的降级形状。
- **验证器是它独有的贡献**：`DomainChecker` 用 DNS + ASN(13335/209242) + published CIDR + TLS 握手 + HTTP 校验，判断哪些域名适合当 fake-SNI，可 CLI 批量重验（`scanner/domain_checker.py:1-50`）⇒ **这就是替代 SNIBypassGUI 冻结 `supported_sites.dat` 的正确机制**（§43）。Cloudflare 段**刻意内联并注明「小且极稳定，官方发布新段时更新」**——这个注释纪律值得抄。
- 反面：`config.json` 11 个无类型扁平键、无 schema 无枚举，且默认 `LISTEN_HOST="0.0.0.0"` ⇒ 与 D-8 同类错误，不要继承。

## 3. 隔离清单（§15/§43 合规证据）

`SNIBypassGUI/resources/payload/data/`：**16 MB / 实测 56 个文件**（子代理报 47，我已更正为 `find -type f` 计数）。含 `sni-gate.exe` 9,736,192 B、`nginx.exe` 6,674,432 B、`supported_sites.dat` 7,714 B、`dns_hosts.txt`、他们自己的 `sni-gate.toml`、`conf/sites/` 42 个逐站 nginx 配置、`conf/shared/upstream/{akamai,cloudfront,fastly,bunny}` 等。

**处置**：不复制、不改写成 Rust、不据此生成域名表、不「借鉴」其 nginx 配置结构。建议把 `_upstreams/SNIBypassGUI/resources/payload/**` 在 Himari 可达路径上做**忽略级隔离**，因为按设计它可被 grep 到（本次审计在查证书处理时**已顺带命中 `paths.ini` 的 5 行**：`[Uninstall] Remove=` 清单与一处提及 sni-gate `[ca] common_name` 的注释；未复现内容，仅如实登记）。

另：`_upstreams/Github-direct` 的隔离清单见 `05-sni-gate-controller.md` §3（含 28 MB 二进制、708 行 sni-gate 源码补丁、`profiles.json` 语料及其 nginx 衍生痕迹）。

## 4. 跨仓裁定（必须选一条并说明理由的冲突）

1. **就绪判定**：SNIBypassGUI「绝不用时钟猜，等内核对象」vs SpoofGUI「100 ms TCP 轮询」。
   **裁定**：采纳 SNIBypassGUI 的原则（不猜窗口）；SpoofGUI 的显式 `WaitForListener` 只作**控制器之外的 UX 兜底**，并且配 `07` 的阳性对照测试。
2. **平台对称表**（Himari 该建的）：WinDivert 主动 vs `FLAG_DROP` 被动 ⇄ pflog 先记录后阻断；NRPT 逐名路由 ⇄ PF `rdr`+`route-to`（含 `lo0 from !127.0.0.0/8` / `user { >root }` 逃生）；`HKCU Internet Settings` + 归属判定 ⇄ `networksetup` 先查再设；job object 终止 ⇄ `flock` + guard + anchor flush；Task Scheduler COM ⇄ `launchctl bootstrap/bootout`。
3. **策略模型合并**：ByeDPI 的 `Vec<Part>` + 语义锚点 + per-group 选择器/优先级 × zapret-for-mac 的 `Op/Caps/Registry` + `OnUnsupported` 降级 × SNISPF 的验证器（**绝不用冻结 `.dat`**）× zapret-for-mac 的 `strategies/*.toml` 预设 + user/generated 分文件 × darkware 的 enum→生成配置 UI + `himari probe`。
4. **许可证台账范式**：SpoofGUI 的缺陷（构建期抓第三方却不产出 NOTICE）是**反面基线**；SNIBypassGUI 的「许可随包同放」与 SpoofDPI 的「机制注明出处」是**正面范式**。

## 5. 无人覆盖的缺口

九仓里**没有任何一个**做过：「**Iced/Rust GUI 以子进程方式运行 sni-gate，并做能力协商式交接**」。Himari 仍是第一个把 SNIBypassGUI 的集成纪律、zapret-for-mac 的 core/transport 切口、sniproxy 的 listener/引用计数模型合到同一对二进制里的项目。⇒ §54「不是另一个 CLI wrapper」在这份归类里有实证支撑。
