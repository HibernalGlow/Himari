## Gate A′ — 上游测试基线（实测）✅

+
+```
+cargo test --release        （未开 `ui` feature）
+test result: ok. 13 passed; 0 failed; 0 ignored
+```
+
+**这 13 这个数本身就是证据**：审计按文件统计的「无 UI 时可跑的测试」= `xray` 6 + `packet/tls` 5 + `packet/tcp` 2 = **13**，另 7 个在 `src/bin/ui.rs`（需 `--features ui`）。实测数与逐项统计吻合 ⇒
+
+- `config.rs` / `sniffer/*` / `proxy.rs` / `handler.rs` / `listener.rs` / `relay.rs` / `scan.rs` **确实零测试**（不是估算，是可复核的计数）。
+- §50 的 Core 测试清单里，**伪造帧数学、注入时序、relay、config 校验、scanner 全部是净增**。
+- Himari 包装上游前应先给这些补上回归基线（尤其 `build_fake_frame` 的 `fake_seq` 与 `CLIENT_HELLO_SIZE=517` 不变量），否则后续任何上游合并都没有参照物。
+
 ## CLI 合同实测（`--help` 原文）
 
 `sni-spoof-rs [CONFIG]`（默认 `config.json`）/ `sni-spoof-rs scan [OPTS]`；

## Gate B — iced 0.14 在本机编译 + 链接 ✅


`cargo build` for 一个真实窗口程序，**依赖树全绿**：`iced_core/iced_runtime/iced_widget 0.14.2/iced_wgpu/iced_renderer/iced_winit 0.14.1` + `wgpu 27` + `wgpu-core-deps-apple` + `metal 0.32` + `objc2-app-kit` + `cosmic-text` + `swash`。

```
SPIKE_RC=0
target/debug/spike-iced  56,027,672 bytes
```

噪音两条（不阻塞，但 §40 打包时要处理）：
- 一个 `mismatched_lifetime_syntaxes` 警告：0.14 的 `view` 返回类型是 `Element<'_, Message>`，**必须显式写 `'_`**。
- future-incompat：传递依赖 `block v0.1.6` 「code will be rejected by a future version of Rust」。记进 §42 依赖台账的待观察项。

## Gate C — neobrutalism 的机制在 0.14 里**可实现**（编译级已证）


风格要求「粗边框 + 粗偏移阴影 + 按压时阴影消失」。实测可行路径：

| 需要的东西 | 0.14 的真实落点 | 证据 |
|---|---|---|
| 描边 | `button::Style.border: Border{ color, width, radius }` | `iced_widget-0.14.2/src/button.rs:487-497`、`iced_core-0.14.0/src/border.rs:6-14` |
| 偏移阴影 | `button::Style.shadow: Shadow{ color, offset: Vector, blur_radius }` | `iced_core-0.14.0/src/shadow.rs:7-13` |
| 按压态 | `button::Status` 是 **unit 变体**：`Active / Hovered / Pressed / Disabled` | `iced_widget-0.14.2/src/button.rs:471-480` |
| 锐角 | `Border.radius` 是 `Radius`，**未从 `iced::` 根导出** ⇒ 用 `..Default::default()` | `border.rs:81` |
| 对比可读性 | `Extended` 每个角色是 `Pair{ color, text }`，文档明写 **`text` 保证在 `color` 上可读** | `iced_core-0.14.0/src/theme/palette.rs:426-434` |

⇒ `docs/design-language.md` 里那条「card 面色 ≠ 窗口底色」的门禁，在 0.14 有**免费助力**：`Pair.text` 由库保证可读，但仍需**自己断言面色差**，因为库不保证「两个不同面色不撞」。

## 0.14 API 事实表（把「egui→Iced 是重写」这件事量化）

上游用的是 eframe/egui 0.28（即时模式、无 Message）。以下每一条都是**这次编译报错实测出来的**，不是文档摘抄：

| 0.12/0.13 时代的写法（会直接编不过） | 0.14 的真实写法 |
|---|---|
| `trait Application { fn new/update/view/title }` | **trait 已不存在**；`iced::application(boot, update, view)` 返回 builder，`.theme(f).run()` |
| `Command<Message>` | **`Task<Message>`**（`iced::Task`，根导出确认存在；`iced::Command` 不存在） |
| `fn new(Flags) -> (Self, Command)` | `BootFn` 接受 **`fn() -> State`**（试 `fn(&mut State) -> Task` 报 E0593） |
| `update(&mut S, M) -> Command` | `update(&mut S, M) -> Task` |
| `container(...).center_x().center_y()` | **`center_x(width: impl Into<Length>)` / `center_y(height)`** —— 现在要传尺寸 |
| `button.style(\|theme, &status\|)` | `style(impl Fn(&Theme, Status) -> Style)` —— **`Status` 按值**，不是引用 |
| `palette.primary.color` | **`palette.primary.base.color` / `.base.text`**（角色是 `Pair` 集合） |
| `Theme::Custom` 手搓 `Extended` | `Theme::custom(name, Palette)`（6 色）或 `custom_with_fn(.., generate)` 自定义扩展集 |

> 结论：**§51 Phase 3 按「新写」估是对的**，且比预想更彻底——不只是框架换掉，0.14 自身相对常见的旧教程写法也已不兼容。**任何照 0.12/0.13 博客抄的代码在这台机器上都编不过。**

## 尚未验证（诚实边界）

1. **视觉正确性**：编译级只证明「写得出来」，不证明「好看/符合风格」。**需要眼睛**。本机 agent 不起 GUI，故列为交付项，见下。
2. **按压「位移」**：风格要求「阴影消失 **且** 元素位移」。`Style` 里**没有平移字段**，所以位移要靠 padding/offset 的 widget 技巧达成 —— **未验证**。
3. **Gate D（未开始）：托盘/菜单栏 × iced 事件循环**。`tray-icon`/`muda` 与 winit 都要主线程/事件循环所有权，这是 Phase 3 最大的结构性风险，**独立成一个 spike**，别和 Gate B 混在一个失败域里。
4. **上游运行时**：BPF 需 root，`--help`/`scan`/`cargo test` 正在后台跑，结果未回填。

## 交给用户的验收清单（按编号回报即可）

在 `/Users/glow/Base/Code/Prometheus/.hibernal-experiment/spike-iced` 下：

```
cargo run
```

1. 窗口是否出现，按钮是否有 **2px 硬边框 + 3px 右下偏移实心阴影**（无模糊、无圆角）。
2. **按住**按钮时：阴影是否**完全消失**。
3. cream 底 (#F4F1EA) 与按钮面色之间是否**一眼分层**（这就是 card≠window 那条门禁）。
4. 点 `Toggle Theme` 切到 dark：分层与边框是否仍然成立。
5. 若有任何一条不符，请指到具体编号 —— 我会把它当成风格 token 的约束写回 `design-language.md`，而不是调参数到看起来过。

## Gate E — 上游 scanner 在本机**真实跑通**（非编译级）✅

+
+```
+./target/release/sni-spoof-rs scan -l mini-sni.txt -c 3 --timeout 4
+SCAN_RC=0
+stdout:  www.cloudflare.com / security.vercel.com          ← 只输出 ok 的 SNI
+stderr:  scanning 3 SNIs against 104.18.4.130:443 (concurrency=3, timeout=4s)
+         progress: 3/3 (2 ok)
+         === summary === total: 3  ok: 2  failed: 1
+```
+
+这次实测把 `01-upstream` 里四条**文档断言**升级为**行为证据**：
+
+1. **无需 root**：scanner 只是对外发伪造 ClientHello，普通用户权限即可跑（BPF 才需要特权）⇒ §7 的 Scanner 页面可以在**不装 helper 的情况下先做出来**，这是 Phase 3 的一条捷径。
+2. **stdout/stderr 严格分离**（working SNIs→stdout，进度与摘要→stderr），与设计一致。
+3. **输出里没有任何时延字段**，摘要只有 `total/ok/failed` ⇒ §7 要求的 `latency` 确实是**净增**，且要改数据模型（`ProbeResult{sni, outcome}` 无字段可放）。
+4. **不做 DNS**：`www.cloudflare.com` 作为 SNI 字符串送出，目标仍是硬编码 IP `104.18.4.130:443` ⇒ §7 的「DNS resolution」同样是净增。
+
+**判据有效性**：同一次运行同时产出 `ok` 与 `failed` ⇒ 这把尺**能区分**，不是恒真断言。`api.github.com` 在本机网络下失败这一事实本身也应进 Himari 自建数据集的候选验证记录（§43：自主扫描→自主测试→自主验证）。
+
+**同时暴露一个必须修的坑**：`-t/--target` 只接受 `IP:port`，**默认值是单个硬编码 Cloudflare IP**。若该 IP 不可达，整批扫描会全红且与 SNI 无关 ⇒ Himari 的 Scanner 必须支持**多 target + 失败归因**（区分「target 不可达」与「SNI 被 DPI 拦」），否则结果不可解释，违反 §29「解释为什么失败」。
+
+## CLI 合同实测（`--help` 原文）
+
+`sni-spoof-rs [CONFIG]`（默认 `config.json`）/ `sni-spoof-rs scan [OPTS]`；
+`scan` 选项：`-t/--target`（默认 `104.18.4.130:443`）、`--timeout`（默认 6）、`-c/--concurrency`（默认 10）、`-l/--list`、`-o/--output`、`-h`。
+**确认无 `--version`、无 `--config`、无 daemon/service 子命令** —— 与 `01-upstream` §2 一致。
+
 ## 尚未验证（诚实边界）
 
 1. **视觉正确性**：编译级只证明「写得出来」，不证明「好看/符合风格」。**需要眼睛**。本机 agent 不起 GUI，故列为交付项，见下。

## Gate F — 渲染的配置被**真 sni-gate** 接受 ✅（`himari-app`）

+
+产品工作区已建在 `/Users/glow/Base/Code/Prometheus/himari-app`（crate：`himari-core` / `himari-gate` / `himari-ui`）。全工作区 **49 tests，rc=0**：core 10、renderer 7、真进程 5、supervisor 12、UI 15。
+
+```
+HIMARI_SNI_GATE_BIN=Himari/target/release/sni-gate cargo test -p himari-gate
+test positive_control_the_harness_detects_a_rejected_config ... ok
+test sni_gate_accepts_the_rendered_config ... ok
+test sni_gate_accepts_a_rendered_config_with_every_route_kind ... ok
+test a_settled_gate_stops_cleanly_and_a_hard_kill_reads_as_a_signal_death ... ok
+test poll_exit_does_not_report_a_live_child_as_dead ... ok
+```
+
+这五条的含义：
+
+1. **阳性对照先行**：往渲染结果里塞一个未知键，harness 必须检出（真 sni-gate 立即退出且 stderr 含 `configuration error`）。这条先过，「进程还活着」才算接受证据。
+2. 我渲染的 TOML **被上游加载器接受**，单路由版与 `tls/ech/http/raw` 四型齐备版都是。自洽 ≠ 合法，这条补上了后者。
+3. **SIGTERM 的语义按启动时机分岔**（详见 `02` 的更正与 `05` 的停止行）：settle 0 s → 信号 15 死亡；settle ≥1 s → exit 0。⇒ 控制器的 graceful stop 必须先过就绪门，否则把用户停止误判成崩溃。
+4. 硬杀被分类为 `Exit::Signal`（`panic = "abort"` 的崩溃就是这个形状，只看 exit code 永远看不见）。
+5. `poll_exit()` 不许把活着的进程报成死的；reap 之后 `has_child()` 必须转 false（这条测试当场逮到一个真实缺陷：`stop()` 原先没清 `self.child`）。
+
+

## Gate G — UI 行为在无窗口下可测 ✅

+
+`himari-ui` 的全部行为在纯函数 `App::reduce(Message) -> Vec<Intent>` 里，15 条测试覆盖 §50 的 UI 行，包括三条「UI 不许说谎」的守卫：
+
+- 控制器若推 `Started` 而状态机判非法，UI **不会**变 Running（并配「合法两步确实到 Running」的对照）。
+- `stub::feedback` 的每条事件序列必须**全部合法**且**永不出现 Running**——骨架不自证健康。
+- 非法表单（端口非数字 / 端口 0 / 空监听地址）**不产生任何 Intent**，错误进 `form_error` 显示。
+
+一处由此逼出的语义决定：`Stop` 打在已 `Stopped` 的组件上必须是 **NoOp**，而不是拒绝、也不是先进「Stopping」——首页 Turn Off 永远可点，拒绝会让 UI 显示假的进行中状态；该决定配了对照测试（运行中的组件被 Stop 必须真变 Stopping 并加 generation）。
+
 ## 尚未验证（诚实边界）
 
 1. **视觉正确性**：编译级只证明「写得出来」，不证明「好看/符合风格」。**需要眼睛**。本机 agent 不起 GUI，故列为交付项，见下。

## Gate H — 上游核心以 **Cargo git 依赖**接入并编译通过 ✅（§2 同步模型实测）

+
+```
+sni-spoof-rs = { git = ".../therealaleph/sni-spoofing-rust",
+                 rev = "d2956025c31d96f0f0a341af4f1a8eda204857c7" }   # v1.0.2-1-gd295602
+Compiling sni-spoof-rs v1.0.2 (https://github.com/... #d2956025)
+```
+
+钉**不可变 commit**而非移动 tag（M11）。`crates/himari-runtime` 全绿，工作区合计 **61 tests，rc=0**。
+
+公开面复核结果（**含对我自己上一条判断的更正**）：
+
+- `scan_all` / `probe_sni` **确实是 `pub async fn`**（`scan.rs:188`、`:257`）。我先前依据 `^pub fn` 模式匹配判定它们私有，是**错的**；扫描器可以当库用。真正的限制只剩 `scan::run()` 内部 `process::exit`（`:63/:85/:249`）与 `DEFAULT_SNIS` 属上游语料（§43 不用）。
+- `Config` / `ListenerConfig` **字段全 pub** ⇒ Himari 直接构造，`config::validate` 可复用。
+- `ProbeOutcome` **未实现 `PartialEq`** ⇒ 只能 `matches!`（第一次编译就报出来了）。
+- `ProxyError` 五个变体可区分 ⇒ 映射成 §29 的失败阶段：`config / local-ip / packet-backend / runtime / thread`，并**始终带上上游的 `platform_privilege_hint()`**。
+
+新增能力（上游没有的两项，§7）：**每探针 latency**、**`Started{in_flight}/Completed` 事件流**。并发上界因此可测而非只写在文档里：`concurrency=2 / 6 个候选`，断言 `gauge.peak() <= 2` **且**必须观察到过 2（否则该断言是空的）。
+
+

## Gate I — 本机 DNS 是 fake-ip，"解析失败"不能当测试夹具 ⚠️

+
+实测（`socket.getaddrinfo`）：
+
+```
+'256.256.256.256' '443' -> OK first=198.18.0.222
+'thishost.invalid' '443' -> OK first=198.18.0.223
+'http://1.2.3.4'  '443' -> OK first=198.18.0.224
+'127.0.0.1' 'notaport'  -> gaierror
+'1.2.3.4'   '99999'     -> gaierror
+```
+
+⇒ 这台机上**任何主机名都能解析**（198.18.0.0/15 是 fake-ip 段）。我最初两条用 `.invalid` / `256.256.x` 当失败夹具的测试**都是假红**，真红的原因是环境不是代码。唯一确定性的解析失败来自**端口语法非法**（getaddrinfo 在查 DNS 之前就拒）。
+
+这条对 §13 DNS 与 §29 Diagnostics 有直接后果：**Himari 不能靠"解析出错了"判断网络状态**，也不能靠 `.invalid` 类域名验证污染/拦截；DNS 判据必须用**已知答案的名字做回读比对**（返回 IP 是否落在预期段），否则在 fake-ip 环境下所有 DNS 断言都会恒真。
+
 ## 尚未验证（诚实边界）
 
 1. **视觉正确性**：编译级只证明「写得出来」，不证明「好看/符合风格」。**需要眼睛**。本机 agent 不起 GUI，故列为交付项，见下。

## Gate J — §49 结构化 IPC 落地 ✅

+
+`crates/himari-ipc`。工作区 **74 tests / rc=0**，clippy 无警告。
+
+**逻辑接口跨平台、transport 才分平台**：codec 写成对任意 `Read + Write` 泛型，所以 macOS 的 UDS 与 Windows 的 named pipe 共用同一套编解码；`proto.rs` 里**没有任何平台类型**（无 `HANDLE`、无 `fd`）——这条是 §49 尾句的可检查形式。
+
+`[4 字节 BE 长度][JSON]`，envelope 带 `schema_version`：
+
+| 守卫 | 断言 |
+|---|---|
+| 版本不兼容 | 收到 `SCHEMA_VERSION+999` 必须 `VersionMismatch`，**不许猜**（猜错的状态会直接画到首页） |
+| 长度上界 | 声明 `0xFFFFFFFF` 必须在分配前被 `TooLarge` 拒（4 MiB 上限） |
+| 帧内截断 ≠ 干净关闭 | 少 3 字节的流必须 `Truncated`；配「同一份完整字节读得通」的对照，否则这条断言是空的 |
+| 坏 JSON | 报 `Json`，不与 EOF 混淆 |
+| 方法名 | §49 的 12 个名字逐个从字面 JSON 反序列化（`get_status`/`get_sni_gate_status`/`test_strategy`…），改名即红 |
+| `Accepted` ≠ `Succeeded` | 受理与成功是两个类型，`Response::Accepted` 不许等于 `Status{running}` |
+| 失败原因分档 | `NeedsPrivilege` 与 `PortInUse` 必须是不同答案，不能塌成一个 "failed" |
+
+**§39 单实例**用进程持有的 `flock(LOCK_EX\|LOCK_NB)`，**不是**「锁文件存在吗」——存在性判断分不出活体占用和崩溃残留。三条测试：第二个持有者被拒 + 释放后可再取（对照）；**残留 socket 文件可清**而**活 socket 不可**（std 不会为 `UnixListener` unlink，正是崩溃留下的东西）；**非 socket 的同名文件永不被当残留删掉**。
+
+Windows 侧未实现，`acquire()` **返回 `Unsupported` 而不是 Ok**——返回成功就等于宣称 §39 已满足，而实际什么都没守。
+
+两处编译期撞出来的细节（记下来避免重犯）：`serde` 内部 tag `event` 与 §34 要求的日志字段 `event` 冲突（tag 改 `kind`，字段保住）；`is_socket()` 在 `FileTypeExt` 上，不在 std 的 `FileType` 上。
+
 ## 尚未验证（诚实边界）
 
 1. **视觉正确性**：编译级只证明「写得出来」，不证明「好看/符合风格」。**需要眼睛**。本机 agent 不起 GUI，故列为交付项，见下。

## Gate K — 后台运行时服务（`himari-daemon`）跑通真 socket ✅

+
+工作区 **79 tests / rc=0**，clippy 零警告。第 6 个 crate `himari-daemon` 是 §33「GUI 主窗口和后台服务生命周期分离」与 §48 分层的物理落点：GUI 只会发 §49 的方法，包引擎只被 daemon 触碰。
+
+**库 + 薄 main** 的拆法让整条请求/事件路径能在真 unix socket 上测，不需要开窗口：
+
+| 测项 | 结论 |
+|---|---|
+| 全新 daemon | 五个组件全部 `stopped`，headline `off` —— 没启动任何东西就**不许**显示任何运行态 |
+| 无特权 Start | 收到 `Error{NeedsPrivilege,…}` 且详情带上游 `platform_privilege_hint()`；**随后 GetStatus 必须是 `failed`**（应答与状态视图一致，这正是 §35「GUI 只渲染状态」的跨线检验） |
+| §39 单实例 | 第二个 daemon 在**绑定任何东西之前**被拒（`AlreadyRunning`）；释放后可再取（对照） |
+| Stop 之后 | 全部回到 `stopped`，不留 `failed` 尾巴 |
+
+**真二进制实测**（`target/release/himari-daemon`）：
+
+```
+第二个实例 rc=2  「another Himari already holds .../himari.lock」
+SIGKILL 后（rc=137）残留 himari.sock + himari.lock
+重启成功「listening on ...」          ← clear_stale_socket 在真进程上生效
+再起一个 rc=2                          ← 陈旧自愈没有把互斥弄坏
+```
+
+两处只有跑起来才会暴露的问题，都已改掉并验证：
+
+1. **daemon 的一个真 bug**：`start()` 只给 `Proxy` 发了 `Start`，漏了 `SniSpoof`。于是紧随的 `StartFailed` 从 `Stopped` 出发是**非法迁移**，被状态机正确拒收，结果 `GetStatus` 显示 `stopped` 而真相是「启动失败」。这条被 e2e 断言当场逮到（`left: Some("stopped") right: Some("failed")`），也证明 §1.2 那张迁移表不是纸面纪律。
+2. **`himari.lock` 默认 0644**（`ls -l` 量出来的），等于把属主 PID 泄露给本机任何用户。改为 0600 后，二进制侧复量：lock `-rw-------`、socket `srw-------`、运行目录 `drwx------`。守卫测试断言 mode 精确等于 `0o600`。
+
+顺带两条编译期事实（别再凭记忆写）：跨 crate 给外部类型写 inherent `impl` 编不过（`EventKind::matches` 改成自由函数）；`Sender::is_disconnected` 在本工具链仍不稳定，改用 `send()` 的返回值判连接已断。还有一个**测试自身的坑**：push 与 response 共用一条流、到达顺序不固定，只收「response 之前」的帧会让断言变成时间掷骰——必须事后排空。
+
+**尚未解决（诚实边界）**：daemon 没有 SIGTERM 处理，被信号杀死时**不会**执行 `shutdown()`。当前只留下可自愈的陈旧 socket（已验证能自愈），但若将来挂着 sni-gate / Xray 子进程，信号死亡会**留下孤儿**——这正是 §36 的 guardian 与 §37 的恢复账本必须补的理由，不是可以跳过的美化。
+
 ## 尚未验证（诚实边界）
 
 1. **视觉正确性**：编译级只证明「写得出来」，不证明「好看/符合风格」。**需要眼睛**。本机 agent 不起 GUI，故列为交付项，见下。

## Gate L — UI 经 IPC 打到真服务，并且**不信任状态字符串** ✅

+
+工作区 **88 tests / rc=0**。`himari-ui` 不再走 `stub`（该模块已删除），改为：`Intent` → `bridge::to_request` → `client::ask` → 真 socket → 服务端。
+
+核心设计取舍：**服务端推的是「状态字符串」，而 UI 的镜像只接受「事件」**。若允许 UI 直接覆写状态，§35/§36 里「控制器不能伪造 Running」那条守卫当场作废。因此：
+
+- `Push::StateChanged` 除状态外**带上它观测到的事件名**（`Event::encode()`，`start_failed` 连阶段一起走：`start_failed:packet-backend`）
+- 客户端 `replay()` 把事件重放进**本地迁移表**，再核对落点是否与推送声称的一致；不一致记为 `protocol_problems` 并在首页显示
+- 非法迁移的推送被拒收并报告，绝不应用
+
+守卫（`bridge_replay.rs` + `ui_reduce.rs`）：
+
+| 断言 | 意义 |
+|---|---|
+| 诚实推送重放零问题 | 正常路径不制造噪音 |
+| `stopped → started` 伪造被拒 | 镜像保持 `stopped`；对照：合法两步确实到 `running` |
+| 合法事件但声称更高状态 | 记 1 条 `mirror` 不匹配；首页停在 `Transitioning`，**永远不许读成 Protected** |
+| 未知组件名 / 未知事件名 | 各报一条，`applied == 0` |
+| 阶段名走线 | `packet-backend` 存活；未知阶段落进 `unknown-stage` |
+
+那条 `unknown-stage` 是**安全而非整洁**问题：初版我用 `Box::leak` 把任意 wire 字符串变成 `&'static str`，等于让一个行为异常的 runtime service 每条状态推送永久占用堆。改成对闭合集合查表。
+
+`client_live.rs` 是**不开窗能做到的最强集成证明**：起真 daemon，走 UI 自己那套 client 代码——
+
+- `client::socket_path()` 与 `daemon.socket_path()` **必须相等**（路径规则只写一处，客户端和服务端不可能各自漂移）
+- `Intent::Start` 经 bridge 出请求 → 收到的推送重放后**零协议问题**、`applied ≥ 2`
+- `Proxy` 与 `SniSpoof` 是同一次动作的两半，镜像里**必须一致**（分岔即 bug）
+- 无特权那侧断言 `Failed` 且摘要含 `NeedsPrivilege`/`bpf`（daemon 侧 e2e 已锁死这条分支为实测事实）
+- 同一目录再 `Daemon::bind` 仍被拒（§39 走客户端路径复现）
+
+另：`bridge` 有一条**覆盖率守卫**——遍历所有 `Intent`，断言「路由 / 显式不路由」计数都不低于阈值且总和等于全集。以后新增按钮忘了接线，这里会红，而不是线上静默失灵。
+
+`RunScan(CurrentFake)` 的解析位置也挪了：现在在 `reduce` 里就按表单展开成 `Hosts([fake_sni])`，所以**进度条显示的总数就是真发出去的总数**，bridge 不必替 UI 编一个目标。
+
 ## 尚未验证（诚实边界）
 
 1. **视觉正确性**：编译级只证明「写得出来」，不证明「好看/符合风格」。**需要眼睛**。本机 agent 不起 GUI，故列为交付项，见下。

## Gate M — §37 恢复账本 + 服务自身的 SIGTERM 路径 ✅

+
+第 7 个 crate `himari-platform`（`journal.rs`）。工作区 **99 tests / rc=0**。
+
+写序按 `03-reference-map.md` 学来的四条：**字节精确备份 + fsync 的 journal 行先落盘，然后才动手**；**绝不把备份盖回第三方改过的文件**；**只删自己 marker 包围的块**；**靠回读验证，不信返回值**。
+
+9 条守卫全部按磁盘字节断言（不是按返回码）：
+
+| 守卫 | 钉住的东西 |
+|---|---|
+| 记录先于改动 | `begin_file_edit` 之后目标文件**仍是原字节**、journal 有一条 `done=false`；commit 才翻（正向对照） |
+| 已提交的改动能逐字节还原 | undo 后回到用户原内容，条目退役 |
+| **第三方改动绝不覆盖** | 别人在我们之后改过 ⇒ 报 `Conflict` **且文件保持对方的字节**；备份不落地 |
+| 崩溃只回滚未完成的那条 | `undo(true)` 不动已完成项，撤销被打断项 |
+| marker 作用域 | 只取回我们的块，`apple/banana` 邻居行必须在 |
+| marker 被人整片抹掉 | 报 `MarkerMissing`，**不算成功**，也不碰现有内容 |
+| 我们创建的文件 | 删除；再 undo 一次是 no-op（Turn Off 两次不许失败） |
+| journal 行损坏 | 报出行号，而不是跳过（跳过= silently 丢一条待撤销记录） |
+
+一处我自己写出的类型错（`Undo` derive 了 `Copy` 却带 `String` 字段）与一处缺失的 `From<serde_json::Error>` 已在编译期暴露并修掉。
+
+**daemon 侧**：`terminate()` 成为唯一真正的停止路径，顺序是 §37 要求的——**先静核心 → 再撤销账本 → 最后释放 socket**，并把 `Report` 交回调用方：**有冲突就不许宣称关干净**（返回 FAILURE 并打印问题）。启动时先跑 `recover_after_crash()`（只撤销上一世未完成的），再对外服务。
+
+真二进制复量（同一台机、同一份代码，对比上一轮）：
+
+```
+SIGTERM rc=0   （上一轮：143 = 被信号杀死）
+TERM 后目录只剩 himari.lock + journal/   ← himari.sock 已被释放
+SIGINT rc=0
+```
+
+信号处理器只置一个 `AtomicBool`（信号上下文里唯一安全的事），主循环轮询该标志后走 `terminate()`；非 Unix 平台不伪装支持，走 `park`。
+
 ## 尚未验证（诚实边界）
 
 1. **视觉正确性**：编译级只证明「写得出来」，不证明「好看/符合风格」。**需要眼睛**。本机 agent 不起 GUI，故列为交付项，见下。

## Gate N — sni-gate 控制器（§14）跑在真网关上 ✅

+
+`himari-gate/src/controller.rs`：渲染器与 supervisor 之上的一层编排。工作区 **110 tests / rc=0**。
+
+**那条实测规则现在是代码，不是注释**：`ready` 只在通过就绪门后置真；`stop()` 只有 `ready == true` 才发 SIGTERM，否则硬杀并且**不冒充优雅关闭**。实测端到端：真 sni-gate 起来 → 就绪 → `stop()` 得 `Exit::Clean`（若上游哪天改了 handler 安装时机，这条断言会先红）。
+
+就绪门的定义是**两条同时成立**：TCP 能连上我们写进配置的地址 **且** 子进程仍归我们所有。只用「端口能连」正是参考审计里被标为设计债的那招；只用「进程在」又证明不了它在服务。端口由 `pick_free_port()` 先要一个再放手，**因为 sni-gate 从不回报它的临时端口**（`02` §2）——这带来一个 release→bind 的竞态，代码里写明了，并说明为何可接受：真占用时网关会退出，`NotReady{exit}` 把退出状态带回来，不会被读成「太慢」。
+
+一次**测试失败挖出的设计洞**（值得记，因为两处都错）：我原先在崩溃检测里清掉 `published_generation`，于是 `decide` 把「崩过」当成「从未发布过」判成 `StartFresh`；而若不清，用户主动 Stop 之后 supervisor 会立刻把它救活。⇒ 判定入参补 `enabled`，并新增 `Action::Stop`：
+
+- `!enabled && alive` → `Stop`（"user wants it down"）
+- `!enabled && !alive` → `Reuse`（"kept down"）
+- 对照：只翻 `enabled` 就必须重新变成 `Restart`；且 **fuse 优先于一切**（保险丝开着时连「放下」的请求也不越权）
+
+崩溃路径实测：`kill -9` → `poll()` 报 `Exit::Signal`、`ready` 转假、失败计数 +1，且**第二次 poll 不重复计数**（只报一次）。
+
+其余：`write_atomic`（tmp + fsync + rename，断言不留 `.tmp` 残骸）；`version()` 走 `-V`——它**在配置非法时也能答**，所以能当起进程前的 pre-flight；`tail()` 每个日志文件封顶 200 行（sni-gate 的 stdout 与 stderr 都得收，配置错误发生在 logger 之前）；还有一条**故意把上游指到 TEST-NET-3** 的测试：listener 照样服务、`is_ready` 照样为真 ⇒ 证明「在服务」≠「路由可用」，M3 的逐路由真握手必须**继续独立存在**，不能被就绪门替代。
+
 ## 尚未验证（诚实边界）
 
 1. **视觉正确性**：编译级只证明「写得出来」，不证明「好看/符合风格」。**需要眼睛**。本机 agent 不起 GUI，故列为交付项，见下。

## Gate O — §7 扫描与 §14 网关都从 IPC 另一头跑通 ✅

+
+工作区 **115 tests / rc=0**。daemon 拆出 `hub.rs` / `jobs.rs`（都不碰请求处理路径，所以 accept loop 与窗口永不等探针）。
+
+**扫描（§7）现在是真链路**：`Request::Scan` → `Response::ScanStarted{job_id,total}` 立刻返回 → 探针在自己的线程 + 自己的 tokio 运行时上跑 → 每完成一条推 `ScanProgress{done,total,usable,latency_ms,outcome}` → 收尾推 `ScanFinished{cancelled}`。测试从 socket 另一侧断言：
+
+- 3 个候选 ⇒ **恰好 3 条进度帧**，`done` 严格 `1,2,3` 单调
+- 掺一个 220+ 字节的非法候选 ⇒ 批次里**必须有不可用项**，且 `ok < 3`（否则「全绿」不证明尺能看见失败）
+- 每条进度都带非空 `outcome`（没解释的行对 §29 无用）
+- `job_id` 不串台
+
+**日志（§34）可读且按 component 过滤**：`GetLogs{component:"core"}` 拿到的行全部以 `[core] ` 开头且含扫描摘要；`component:"xray"` **不许借用别的 component 的行**。
+
+**取消不撒谎**：没有扫描在跑时 `Stop{target:"scan"}` 返回 `Error{Busy,"no scan is running"}`，而不是一个 `Accepted`；有扫描时也只是**停止上报**，在飞的 TLS 探针不会假装被中断（注释里写明这条边界）。
+
+**网关（§14）接进 daemon**，且失败面与成功面都测：
+
+| 情形 | 应答 | 之后 `GetSniGateStatus` |
+|---|---|---|
+| 未配二进制 | `Error{BackendUnavailable}`，detail 里带** remedy**（含 `SNI_GATE_BIN`） | `sni_gate=failed`，stage `gateway-binary-missing` |
+| 配了真二进制 | `Accepted`（**只在就绪门通过后**才推 `Started`） | `sni_gate=running` + `sni_gate_version` 含 `sni-gate` |
+| 随后 Stop | `Accepted` | `sni_gate=stopped` |
+
+`terminate()` 现在**先停网关再撤账本**，停不动就写进日志而不是静默。
+
+两处诚实性/卫生修正：`Options` 改为**注入**（`bind_with_options`），测试不再抢改进程环境变量；`Push::StateChanged` 带事件名的同时，`jobs.rs` 的计数改成 `AtomicUsize`，因为 `himari-runtime::scan` 要求回调是 `Fn` 而非 `FnMut`（第一次编译就报出来了）。
+
+本轮我 own 的两个失误，都记下来避免重犯：(1) 用 python 脚本批量改测试字符串，把文件改出语法断裂和 `assert_eq_` 这种残骸——**结论：文件级重写比脚本改字符串可靠**；(2) 新测试忘了先消费 `ScanStarted` 响应就等推送，导致自己的测试红（不是服务红）。
+
 ## 尚未验证（诚实边界）
 
 1. **视觉正确性**：编译级只证明「写得出来」，不证明「好看/符合风格」。**需要眼睛**。本机 agent 不起 GUI，故列为交付项，见下。

## Gate P — §22 macOS 系统代理「先存后还」✅（128 passed / clippy clean）

+
+这一步的主要产出是**把上一版里编造的命令语法全部抓出来**。全部对照 `networksetup -usage` 与本机只读实跑：
+
+| 上一版写的 | 真机事实 |
+| --- | --- |
+| `-setwebproxy <svc> State off` | 状态是**独立命令** `-setwebproxystate <svc> <on|off>`；`-setwebproxy` 只吃 `<domain> <port> [<authenticated> <user> <pass>]`，并且**它自己会把代理打开**（usage 原文 "Turns proxy on"） |
+| `-setwebproxyAuthenticatedProxyEnabled` | 不存在。认证只能靠 `-setwebproxy` 的可选参数写，而 `-getwebproxy` **读不回密码** |
+| 过滤 `"There are no"` | 空 bypass 的原文是 `There aren't any bypass domains set on Wi-Fi.`（旧过滤器会把这句话当成一条排除域名） |
+| bypass 输出=括号逗号一行 | **一行一条**（与实测的 `-getdnsservers` 同族），空列表才是一句话 |
+| `-setautoproxyurl <svc> <url> <on|off>` | 只有 `<svc> <url>` 两参且**总是带 enable**；要关必须再发 `-setautoproxystate <svc> off` |
+| — | `-getwebproxystate` **不是命令**（回整页 usage + `** Error: The command is not recognized.`），开关状态只能从 `-getwebproxy` 的 `Enabled:` 读 |
+| — | 未知服务 rc=**8**，`Unable to find item in network database.` 只在 **stderr**，stdout 为空 |
+
+**没有拿真机当测试夹具。** `crates/himari-platform/tests/networksetup_stub.sh` 是假 `networksetup`：只接受上面实测过的形式（所以编造的 flag 会直接 rc=1 让测试变红）、状态存文件、每次调用记进 `calls.log`；它的 `-getwebproxy` 输出与真机现状逐字节相同（`Enabled: No / Server: 127.0.0.1 / Port: 7890 / Authenticated Proxy Enabled: 0`）。两个故意弄坏工具的开关 `fail_on`（拒绝执行）和 `swallow`（返回 0 但不执行）专门用来测「命令被接受却没落地」。
+
+落地的三条设计判断（都有对应守卫）：
+
+1. **接管前先判断能不能还回去。** 认证代理在用（密码读不回）、或 PAC 处于开启（macOS 优先用 PAC，Himari 会显示"已启动"却什么都不走）→ `Occupied` 直接拒绝，且 `calls.log` 里一条 `-set` 都没有。你机器的 Wi-Fi 正好就是"配了 127.0.0.1:7890 但关着"这种状态，被当成基准夹具。
+2. **还原只发我们改过的东西。** HTTP/HTTPS 各自按"原本开着 / 原本关着 / 从没配过"三分支出命令；PAC 与 bypass 列表**一条写都不发**（用 `calls.log` 断言），所以"关 Himari 把用户的排除列表洗掉"这类事故没有代码路径可走。
+3. **journal 是唯一还原通道。** 新增 `Ledger::undo_kind(kind)`（关掉一个功能不许顺手回滚用户还想要的其它变更），并删掉了重复的 `NetworkSetup::restore`。`point_at` 在**第一条写之前**就把还原序列落盘（`fail_on` 测试证明：HTTP 已改、HTTPS 未改的半成品状态，`undo(true)` 能干净还原）。
+
+**一个没能证明、已经写进代码假设的事实**：非特权用户执行 `-setwebproxystate "Wi-Fi" off`（值本来就是 off）返回 rc=0，而 `preferences.plist` 的 mtime/大小前后完全一致（1791011164 / 10943），所以**"普通用户能不能落地系统代理写入"无法在不改动你实际配置的前提下判定**。处理方式是不去猜：`point_at` 之后强制读回，读不回就报 `NotApplied`，daemon 把它映射成 `NeedsPrivilege` —— §20 那个特权 helper 的接缝就长在这个错误码上，而不是长在一个 `if` 假设上。
+
+**未覆盖项（不装作覆盖了）**：`the_system_proxy_follows_the_listener_either_way` 在本机走的是反向分支（无特权 → packet-backend 先失败 → 断言假工具一条都没被调用、journal 为空、状态是 `failed`）。正向分支（监听起来→真的改代理→关掉→还原并读回）在这台机器上跑不到，其语义由适配器层 11 条守卫承担。
+
+跑全套（网关生命周期那条守卫**故意不静默跳过**，缺环境变量就 panic）：
+
+```
+cd /Users/glow/Base/Code/Prometheus/himari-app
+HIMARI_SNI_GATE_BIN=/Users/glow/Base/Code/Prometheus/Himari/target/release/sni-gate \
+  cargo test --workspace        # 128 passed / 0 failed
+cargo clippy --workspace --all-targets   # 只剩 block v0.1.6 的第三方 future-incompat 提示
+```
+
 ## 交给用户的验收清单（按编号回报即可）
 
 在 `/Users/glow/Base/Code/Prometheus/.hibernal-experiment/spike-iced` 下：

## Gate Q — §42 第三方许可台账：从"抄一份清单"变成门禁 ✅（132 passed / clippy clean）

+
+台账落在 `himari-app/THIRD-PARTY.md`，但**它本身不是产物**，产物是 `crates/himari-daemon/tests/third_party_ledger.rs` 的四条断言：许可证一律从磁盘上真实的 crate manifest 里读（不采信 README、不采信记忆），直接依赖必须在台账里有行，上游 commit 钉死必须与 lock/台账一致。
+
+这一步真正挖出来的两件事：
+
+1. **"整串精确匹配 allowlist" 是错的。** 第一版把 `MIT OR Apache-2.0` 这类**字符串**当许可项，实跑立刻错判 **62 个包**：`MIT/Apache-2.0`、`Apache-2.0/MIT`、`Zlib OR Apache-2.0 OR MIT`、`(MIT OR Apache-2.0) AND Unicode-3.0`、`Unlicense OR MIT`、`Apache-2.0 WITH LLVM-exception OR ...` 全是同一许可的不同写法，而 `Apache-2.0 OR GPL-2.0-only` 应该**选 Apache 那条分支**。改成按 SPDX id  tokenize、只要存在一条全许可 OR 分支即通过。分类器自己带正/负对照（`LGPL-3.0-or-later`/`AGPL-3.0`/`LicenseRef-proprietary` 必须红），否则"全绿"可能只是因为什么都判绿。
+2. **WinDivert 已经在 `Cargo.lock` 里**：`windivert 0.7.0-beta.4` 与 `windivert-sys 0.11.0-beta.2`，manifest 实测 `LGPL-3.0-or-later`。它们不参与 macOS 构建，但 D-7 因此**不再是"以后再说"**。当前处理＝具名豁免 + 断言其许可证仍是 LGPL-3.0 + 断言 `THIRD-PARTY.md` 仍写着 D-7 —— 豁免不能替一个猜测背书，Windows 阶段真要用就得回来把这条改成结论（含"绝不连带再分发上游那 87 MB 预编译二进制"）。
+
+另外三个实测细节：`sni-spoofing-rust` 的 LICENSE 是 MIT 但版权行是 `Copyright (c) 2025`（**作者名是空的**），所以署名必须按仓库 URL + commit 写；本仓（sni-gate fork）是 `MIT OR Apache-2.0` 双证。Xray / Wintun **尚未引入**，台账里就明确写"不适用/待定"，没有为了让表看起来完整而填值。
+
+**一个方法层教训（已写进长期记忆）**：本机 cargo registry 是跨项目共享的，`calloop 0.12`、`windivert` 这类"盘上有"完全不等于"这个构建会编译"。所以豁免必须按 **name+version** 精确匹配，并且双向断言：豁免项若在盘上出现就要求重新判定（第一版按 name 断言，直接被 `calloop 0.14.5` / `redox_syscall 0.9.4` 打了脸——两条互相矛盾的结论同时出现，才暴露出这是个版本问题而不是存在性问题）。
+
 ## 交给用户的验收清单（按编号回报即可）
 
 在 `/Users/glow/Base/Code/Prometheus/.hibernal-experiment/spike-iced` 下：

## Gate R — §36 守护者：真进程被杀之后 ✅（135 passed / clippy clean）

+
+补上的是之前唯一缺的一环：状态机有 `Crash/RecoveryAttempt/RecoveryGaveUp`，控制器有 `poll()`/`decide()` 和保险丝，但**没有任何东西去发现进程没了**。现在 `Daemon::guard_tick(now_ms)` 就是那一轮"发现→恢复→上报"，`spawn_guardian(every)` 按间隔驱动它（§34 反对的是激进轮询，不是反对定时心跳；尝试次数由控制器自己的保险丝封顶，烧了就一直报 `failed`，不再无限重启装作在服务）。
+
+三条守卫（`crates/himari-daemon/tests/guardian_guards.rs`，连跑三次 0.45–0.77s 全绿）：
+
+1. **`a_gateway_that_is_killed_comes_back_with_its_state_following`** —— 用真 sni-gate 二进制：起网关 → 读 pid → `kill -9` → 有界窗口内必须报 `§36 gateway exited`；之后要么 `running` 且 **pid 变了**（同 pid 回来等于什么都没重启），要么 `failed` 且日志写明恢复失败。两种结局都断言，唯独不允许"还是 running"。
+2. **`a_tick_with_nothing_claiming_to_run_resurrects_nothing`** —— 全新 daemon 一次 tick 必须**空报告**、四个组件仍读 `stopped`（守护者不许跟用户主动的 Stop 打架）。
+3. **`a_deliberately_stopped_gateway_stays_stopped_across_ticks`** —— 起→停→连打三 tick 必须都空，且 pid 已交还。
+
+**顺带量到的事实**：`kill -9` 之后**第一个** tick 抓不到死亡——`try_wait()` 在那一瞬间仍报"还在跑"，进程要先被回收。所以测试按"真实间隔会给它几次 tick"来写（有界重试 + 窗口内必须发现），而不是假装第一拍就该命中；这也是 §36 的实现要跑在**循环**里、而不是"事件来了才看一次"的实证理由。
+
+**顺手补的一个洞**：`Daemon::apply` 之前把状态机拒绝的事件**静默丢掉**（`state.apply(...)` 返回值被扔了）。非法转移正是"UI 显示 running 但进程已死"最常见的来路，所以现在被拒绝时写一条 `[core] <组件> refused <事件>: 现在是 <状态>` 的日志。另外许可台账的门禁也补严了：只认 `[package]` 段里的 `license`，别处的 `license = "..."`（依赖声明里的）不再被当成本包许可证。
+
+**当前未做（不装作做了）**：守护者还没有 Windows/Xray 的活性源（Xray 在本 build 里按 `xray-not-wired` 明确报失败），也没有接到 Iced 首页的可见提示——那是 §29 诊断页和阶段 4 的活。
+
 ## 交给用户的验收清单（按编号回报即可）
 
 在 `/Users/glow/Base/Code/Prometheus/.hibernal-experiment/spike-iced` 下：

## Gate S — §29 诊断页的判定核心 ✅（149 passed / clippy clean）

+
+`crates/himari-ui/src/diagnostics.rs` + `tests/diagnostics_guards.rs`（14 条），`view::diagnostics` 现在只是把判定逐行渲染出来。九行覆盖 §29 要求的 Network / Proxy / DNS / Packet backend / SNI / Strategy / Xray / sni-gate，外加一行 **Runtime service**——因为"没能问到"和"它是关的"是两件事，混在一起就是这页最容易撒的谎。
+
+判定只有五种：`✓ 实测通过` / `⚠ 实测但要注意` / `✕ 实测失败` / `— 明确关着（既不算失败也不算通过）` / `? 没测过`。**新增 `?` 这一档是这页的核心设计**：任何拿不到证据的行必须停在 `?` 并写出「要怎样才能测」，而不是顺着用户期待滑到 `✓`。
+
+几条按"证据从哪来"写死的规则，每条都有断言：
+
+- **Proxy 行是假 Running 的捕手**：状态 `Running` 但没有 `observations.proxy_bound_addr` → 判 `✕`（"说自己在跑，却从未观测到绑定地址"）。配了另一半阳性对照：同一行在证据齐备时**必须**能变 `✓`，否则上一条断言只是无脑报红。
+- **`App::default()` 一行 ✓ 都不许有**（开窗口≠测过任何东西）。
+- 特权被拒要说清去处：`StartFailed{packet-backend}` → `✕` + remedy 明确指向 **helper**（且断言文案里有 helper/root），因为 §20 禁止把整个 app 跑成 root；`gateway-binary-missing` 与 `gateway-not-ready` 是**两种不同的失败**，一条说"没二进制（HIMARI_SNI_GATE_BIN）"，一条说"起来了但从未 accept"（并提示去读它的日志，因为配置错误写在 tracing 初始化之前的 stderr）。
+- **不替别人背书**：`EgressState::Reported(ip)`（§12 要的"证明流量走了 Himari"）是**某个外部服务回的**，所以 Network 行仍然停在 `?`，测试直接断言这行的文案里不许出现那个 IP。
+- Strategy 行只认"策略 + 结果"成对出现的日志；DNS 行只认扫描里确实带 `dns`/`resolv` 的那一条，其余一律 `?`。
+- LAN 共享开着时，Proxy 行连"关着"都要复述一遍（§11 的意外暴露必须看得见）。
+
+**顺带清掉的**：`view.rs` 里 `word2` 变成死代码后直接删了（不留 `_` 前缀糊弄编译器）。
+
+**未做（不装作做了）**：Network 行的接口/路由探测、DNS 行的系统解析器实况，仍需要 §23 的网络变更重探与平台适配器接线；`Strategy` 行要等 §27/§28 的策略缓存。这些在页面上现在就是 `?` + 一句"要怎么测"，而不是空白。
+
 ## 交给用户的验收清单（按编号回报即可）
 
 在 `/Users/glow/Base/Code/Prometheus/.hibernal-experiment/spike-iced` 下：

## Gate T — §23 网络服务每次现问，不许记 ✅（158 passed）

+
+§23 要求"网络变了要重探""不能永久缓存接口名"，而此前代码里**根本没有**"现在到底哪个服务在带流量"这个概念——代理服务名是从环境变量拿的，等于人肉缓存。现在有了：
+
+**基准是抄下来的真机输出**（就在实现这一段的当口跑的）：
+
+```
+$ /sbin/route -n get default        $ networksetup -listnetworkserviceorder
+  interface: en0                    (1) Thunderbolt Bridge / Device: bridge0
+                                    (2) Wi-Fi / Device: en0
+                                    (3) Tailscale / Device:        ← 空！
+```
+
+这台机装了 Tailscale，而它的 `Device:` 是**空的**（系统扩展型 VPN 没有 BSD 接口），所以 `resolve()` 遇到 `utun3` 这类路由设备必须落在 `UnknownDevice` 而不是"挑第一个/挑刚才那个"——**这一条就是 §23 最容易做错的地方，且有断言**（`a_vpn_that_holds_the_route_is_reported_as_unfollowable_instead_of_guessing`）。
+
+七种判定：`Matched{service,device}` / `NoRoute` / `UnknownDevice{device}` / `Disabled{service,device}`（`-listallnetworkservices` 的 `*` 前缀=禁用，实测 header 原话）+ 三个纯解析函数（服务序 / 默认路由 / 禁用列表）。7 条纯解析守卫直接吃上面那三段原文（`tests/service_resolution_guards.rs`）。
+
+**守护进程侧（`Daemon::current_network_service()`）**：每次调用都重问 `route` + `networksetup`，中间不落任何记忆。守卫 `the_current_service_is_re_asked_on_every_look_and_never_remembered` 用**同一个 Daemon 实例、不重启**，只把假工具的路由状态从 `en0` 改到 `en5` 再改到 `utun3`，断言答案依次是 `Wi-Fi → Ethernet → UnknownDevice`（起始态还没有默认路由时必须是 `NoRoute`，不允许凭空冒出一个服务）。这是"缓存"最直接的证伪法。
+
+`guard_network()`（挂在 Gate R 的 `guard_tick` 上，排在最前）按 §22/§23 的联合口径行动：路由**换了服务**→ 先把原服务的设置原样还回去、再跟到新服务；路由落到**无法指向**的状态（无默认路由 / 隧道设备 / 服务被禁用）→ **交还原服务**并说明原因，而不是继续挂在一个已经不带流量的指向上。问不到答案时**保持现状并报告**——不能证明它错了，就不动手。
+
+**未做（不装作做了）**：诊断页的 Network 行现在仍是 `?`。§49 的方法表里没有"当前服务"这一项，把 §23 的答案送进 UI 需要先定它是走 `get_status` 扩字段还是新增一条只读方法——这是契约决定，不该由我在实现里偷偷加。另外 `guard_network` 的**行动**分支在本机跑不到（无特权时监听起不来，`proxy_prior` 因此一直是 `None`），语义由假工具侧的判定守卫 + 上面的证伪守卫承担，与 Gate P 的同一限制并列记录。
+
 ## 交给用户的验收清单（按编号回报即可）
 
 在 `/Users/glow/Base/Code/Prometheus/.hibernal-experiment/spike-iced` 下：

## Gate U — §20/§21 特权 helper 的第一块：方法白名单与鉴权 ✅（167 passed / clippy clean）

+
+新 crate `crates/himari-helper`（workspace 第 8 个成员）。这一片只放**安全边界**，不放执行——8 条守卫全部不需要 root、也不碰真机：
+
+1. **客户端不能命名程序。** `Request` 是闭合枚举 + 显式字段表白名单（`allowed_fields`），测了六种走私写法（`{"method":"run","program":"rm",…}`、在合法方法上追加 `program`/`args`、把 `service` 写成对象、未知方法名、没有 method）全部拒收。另一条断言把性质钉在类型上而非 serde 上：序列化回来的请求里**不存在** `program`/`args` 字段可去。
+2. **只对允许的 uid 办事**，且带阳性对照（同一请求换 uid 就 `Execute`）。
+3. **重放与回退**：同 generation → `AlreadyApplied`（不是第二次写）；更小的 generation → `StaleGeneration`（照办就会把用户后来的改动倒回去）；`note_applied` 取 max，水位不许倒退；只读方法（`current_service`）无 generation，问三次跑三次。
+4. **per-service 的键空间**：`restore_system_proxy:Ethernet` 不许被 Wi-Fi 上见过的 generation 吞掉（§22 的回还是按服务的）。
+
+**两条实测把设计改掉了**（不是实现细节，是能力边界）：
+
+- serde **不支持**在内部标签枚举及其 variant 上用 `deny_unknown_fields`（编译期直接报 `unknown serde variant attribute`）。所以"拒收未知字段"改成显式字段表校验——顺带更好审：白名单是一张能读能 diff 的表，不是一个属性开关。
+- macOS 的 `LOCAL_PEERCRED` 返回 **`xucred`：只有 uid + 组，没有 pid**（读 `libc` 的 Apple 定义得证；Linux 的 `sockpeercred` 才有 pid）。因此**helper 无法识别"是哪个进程连的"**，`Peer.pid` 恒为 `None` 且已配断言（将来变 Some 即平台行为变更，测试会红）。可验证的是**权限身份**；**程序身份必须落到代码签名**（§41 的 `SecCodeCheckValidity`），本 crate 还没引入，已写进 `docs/privileged-helper.md` 的更正段。
+
+**顺带证明 §42 门禁有牙**：加 `libc` 依赖的**当轮**，`every_direct_dependency_has_a_ledger_row` 就把它拦下来（`add these to THIRD-PARTY.md: ["libc"]`），并把整轮 `cargo test --workspace` 打到 rc=101 中止——不是警告，是失败。补了实测行（`MIT OR Apache-2.0`，从磁盘 manifest 读）后 **167 passed / 0 failed / 0 ignored**。
+
+**这一片明确没有的东西**（别把"有 helper crate"读成"§20 完成"）：没有监听循环、没有 `main`、没有安装/一次性提权（`SMAppService` 还是待实测）、没有代码签名校验、daemon 也还没有把 `NeedsPrivilege` 转成对 helper 的调用。下一步就是从 `peer_credentials` + `authorize` 往外接服务端，并按 §37 让每次执行先落 `Ledger`。
+
 ## 交给用户的验收清单（按编号回报即可）
 
 在 `/Users/glow/Base/Code/Prometheus/.hibernal-experiment/spike-iced` 下：

## Gate V — helper 回路 + §14 就绪门的一个**真实缺陷**（先证伪后修）✅（175 passed / clippy clean）

+
+### 1) `himari-helper` 的服务端与客户端通了
+
+`server.rs`（`serve_connection`/`accept_one`/`bind`）+ `client.rs`（`Client::call/point_system_proxy/restore_system_proxy/current_service`），复用 `himari-ipc` 的同一套 framing（§49 的"同一逻辑接口"）。特权动作本身收在 `Execute` trait 后面，测试用计数器实现，所以**不需 root 就能断言"效果发生了几次"**：
+
+- 允许的 uid：point→restore 各跑一次，helper 自己数出 `applied=2`；
+- 重放同一 generation → `Refused(already carried out)`，**效果列表不变**、`applied=1`；
+- 回退的 generation → `Refused(behind the applied)`，新意图不被旧请求覆盖；
+- `current_service` 问三次跑三次，效果列表**为空**（读不许变成写，§23）；
+- 线上直接塞 `{"method":"run","program":"rm","args":["-rf","/"]}` → 会话结束、什么都没跑（不是"回一个错误继续解析"——无法解码的流不能猜边界）；
+- 日志逐条带 `applied:` / `refused:` 前缀（§29 诊断页要引用它）。
+
+helper crate 合计 **15 条**（9 条鉴权 + 6 条回路），全绿。
+
+### 2) 兄弟会话那句「端口能连≠网关在跑」在我自己的代码里成立
+
+我原来的 `wait_until_serving` 判定是 **TCP connect 成功 + 子进程没退出**，注释还写着"port-only 是参考审计点名的设计债，所以两者都要"。写了攻击性测试 `crates/himari-gate/tests/readiness_impostor.rs`：一个脚本子进程活着但从不绑定，另一个 `TcpListener` 占着配置端口——
+
+```
+the gate called a foreign listener on port 57879 our gateway serving: Ok(())
+```
+
+**红得确凿**：两个条件都满足时，网关根本没在跑，而 §36 明令禁止的那个状态（显示在跑、进程没在服务）会通过健康检查到达。
+
+修法是把第三个条件加进闸门：**该端口的监听 socket 必须属于我们那个子进程**。macOS 侧 `lsof -nP -iTCP:<port> -sTCP:LISTEN -Fpc`，按 `p<pid>` 字段行取 pid（不是按列切，不解析它的表格排版）；探针程序可注入（`with_port_owner_probe`），并且**探针跑不起来时报 `OwnerProbeUnavailable` 而不是跳过**——证明不了归属就不许说 serving。新增两个错误变体：`ImpostorListener{port, owner_pids, waited_ms}`（连上了但不是我们）与 `NotReady`（压根没连上）分开，因为建议不同。
+
+两条守卫一起验，缺一条这个修复就毫无意义：
+
+- `a_port_owned_by_somebody_else_is_not_reported_as_serving` → 拒，并且 `is_ready()` 仍为 false（否则 stop 会当自己是优雅关闭）；
+- `the_same_gate_still_passes_when_the_real_gateway_owns_the_port` → **真 sni-gate 在自由端口上仍然通过**（阳性对照；缺 `HIMARI_SNI_GATE_BIN` 时该测试故意 panic 而不是跳过，因为"永远说不"的门同样能过第一条）。
+
 ## 交给用户的验收清单（按编号回报即可）
 
 在 `/Users/glow/Base/Code/Prometheus/.hibernal-experiment/spike-iced` 下：

## Gate W — helper 有了自己的二进制、启动自检与真实端到端 ✅（189 passed / clippy clean）

+
+`himari-helper` 现在有 `main.rs` + `startup.rs`，crate 内合计 **29 条守卫**（9 鉴权 + 6 回路 + 8 效果 + 6 启动/端到端），其中端到端那组是**直接跑 `CARGO_BIN_EXE_himari-helper` 这个真二进制**打真 unix socket、效果落到假 `networksetup` 上——单元测试全绿也证明不了 `main` 真的把参数解析、目录自检、bind、服务循环和执行器接在了一起，所以这一组是唯一有意义的证据。
+
+**启动即自检，且默认拒绝**：
+
+- 没有 `--allow-uid` 就**不启动**（`MissingAllowedUid`，退出码 2）。不给"默认允许任何人"留任何代码路径。
+- 未知 flag / 缺值 / `--allow-uid root` 这类拼错都归 `BadArgument`——安装脚本写错必须炸，不许"安静地做了别的事"。
+- journal 目录与 socket 所在目录必须**属主等于本进程 euid 且不含 group/other 位**；判据读的是真 metadata。两个不同 uid 共用一份 journal = 谁先到谁决定谁能改写对方的还原记录，所以这条在任何 bind 之前完成。守卫里带正/负对照：0700+自己 → 通过；chmod 0755 → `LedgerDirTooOpen{mode:0o755}`；期望 uid 填 +1 → `LedgerDirOwnedByOther`。
+- 检查失败时**先不 bind**：`assert!(!socket.exists())` 断言"拒绝启动就不该留下监听套接字"。
+
+**两次被实测纠正的记录**（都是我先按想当然写、跑起来才发现）：
+
+1. `--once` 模式下**探测连接会把唯一那次会话用掉**：helper 服务完一个空会话就退出，真客户端再来就是 `Broken pipe`。判据从"先探一下再连"改成"重试到连上为止，第一次成功连接就是正式会话"。
+2. generation 表的实际作用域是**进程生命周期**（声明在 accept 循环之外），不是每连接一份。所以断言按代码真实语义写成：客户端崩了重连同一条 generation 仍然被 `Refused(already carried out)`；跨重启的防重放交给 §37 账本，不在这层承诺。
+
+端到端连跑 3 次稳定（1.05–1.29s）。§37 的启动恢复也接上了：helper 在服务任何请求**之前**先 `ledger.undo(true)`，回滚不掉的就 stderr 逐条报出来。
+
+**还差什么（阶段 4 未完）**：`--once`/自检是普通用户身份跑的，**真正的 root 安装路径没跑过**；launchd 标签与一次性提权（§21）、代码签名校验（§41，`xucred` 给不出进程身份）、BPF 方法与 §24 生命周期、以及把 daemon 的 `NeedsPrivilege` 改走 `client::Client`（下一步：先定 helper 账本目录的属主与路径，两个 uid 不能共用一份 journal）。
+
 ## 交给用户的验收清单（按编号回报即可）
 
 在 `/Users/glow/Base/Code/Prometheus/.hibernal-experiment/spike-iced` 下：

## Gate X — daemon 真的走 helper 了：§20 的通路闭合（191 passed / clippy clean）

+
+`SystemProxy` 增加 `helper_socket`（环境 `HIMARI_HELPER_SOCKET`）。**只要配了 helper，daemon 就不再自己写系统代理**——这不是保守，是 §36 的要求：非特权写"被接受却没落地"恰好制造"显示在跑、实际不带流量"的状态。
+
+三条新事实各有守卫：
+
+1. **不越权写**（`himari-daemon`）：配一个不存在的 helper socket → `apply_system_proxy` 报 `HelperUnreachable`（映射成 `NeedsPrivilege`），并且假工具的 `calls.log` 里**一条 `-set` 都没有**、daemon 自己的 journal **是空的**。也就是说"没有回落到非特权直写"是被断言的，不是被注释承诺的。
+2. **还原的属主分离**：helper 模式下回还原走 `client.restore_system_proxy`，daemon **不**去 replay 自己的账本（那份是普通用户的，helper 那份是 root 的）。这正是 §20"两个 uid 不能共用一份 journal"的落点：`--ledger-dir` 单独给 helper，启动时还强制"属主=euid 且无 group/other 位"。
+3. **崩溃后由下一个进程擦屁股**（§37，跑真二进制）：用 `fail_on=-setsecurewebproxy` 让第一次 point 只改了一半就失败 → 断言机器上确实坏着（`Enabled: Yes / 40443`）→ 撤掉开关、**换一个 helper 进程**起来 → 它在服务任何请求之前把未完成项回滚，断言回到 `No / 7890`。这条不需要 root，测的就是启动恢复的顺序。
+
+词汇表也统一了：helper 的 `Refused` → `PlatformError::Occupied`（§22 同一判断）、`Transport` → 新增的 `HelperUnreachable`、`Failed` → `Unavailable`；daemon 侧 `proxy_error_code` 把 `NotApplied` 与 `HelperUnreachable` 都归到 `NeedsPrivilege`，所以 UI 的"该去装 helper"这条建议不取决于当次走的是哪条路径。generation 由 daemon 的单调计数器提供（`Arc<AtomicU64>`），helper 侧的重放守卫据此拒第二次写。
+
+**这一层测试的边界（写清楚，别让 191 这个数字骗人）**：`himari-daemon` 的测试拿不到 `CARGO_BIN_EXE_himari-helper`（跨 package 不提供），所以 daemon 侧只能证明**选路正确**（不直写、不记账），而"二进制真能把效果落地"由 `himari-helper` 自己的 7 条启动/端到端守卫证明。两边合起来才覆盖 §20 的链路，任何一边单独绿都不算。
+
+**仍然没有的**：真正的 root 安装与一次性提权（§21）、代码签名校验（§41，`xucred` 给不出进程身份）、launchd 启动项（§38，方法在表里但明确回 `not wired`）、BPF 生命周期（§24）、Windows 侧（含 D-7 WinDivert 许可决定）。
+
 ## 交给用户的验收清单（按编号回报即可）
 
 在 `/Users/glow/Base/Code/Prometheus/.hibernal-experiment/spike-iced` 下：

## Gate Y — §27 策略引擎 + §28 自动模式缓存（207 passed / clippy clean）

+
+新 crate `himari-strategy`（第 9 个成员）。放独立 crate 而不是塞进 `himari-core`，是为了守住"core 零外部依赖"这个既有性质（§35/§36 的状态机里不该出现 serde），也让两条 UI 路线都能共用。**纯逻辑 + 一个调用方指定路径的文件读写，没有任何网络类型**。
+
+§27 的五个策略都在表里（`FakeSni`/`SniGate` 可用，`Fragmentation`/`Disorder`/`TlsRecord` 是保留位），保留位必须自述为什么不可用——理由文本就是 §29 要显示的话（指向 D-4：上游把 sniffer/proto 留成私有，不是"我们懒得做"）。
+
+16 条守卫锁的是"看起来合理但会说谎"的六种情形：
+
+1. **没测过的目标不许有"最佳策略"**：只能得到 `TryUnmeasured`（探第一个可用策略），且 `has_evidence == false`。§29 的问题和引擎的问题被刻意分成两个 API——"我该试哪个"和"我们有证据吗"不是同一件事，合成一个就必然有一边说谎。
+2. **缓存不许凭空造出能力**：塞一条 `fragmentation` 的成功记录进去，`select` 仍然只能提议可用策略；出现 `Reuse(Fragmentation)` 即测试失败。
+3. **证据会过期**（24h 窗口，边界内侧仍算、外侧变回未知），且"过期"不许被说成"坏了"。
+4. **排序不许靠假装数字**：延迟缺失排在有测量值**之后**，而不是当成 0 抢赢 900ms。
+5. **失败策略在其它选项未试前不许被复活**（防抖），而全都失败时明确返回 `Exhausted{tried}`——"自动模式没找到"是答案，不是悄悄重试第一个。
+6. **§28 的隐私口径按结构断言，不按声明断言**：把 JSON 走一遍，收集所有**字段名**（`targets` 下面的键按数据处理，因为它们是 SNI 值），断言字段集合 ⊆ `{version, targets, strategy, success, latency_ms, at_ms}`；并配对照：`success` 必须在集合里（否则这个检查是在空集上自娱自乐）、`private.example` 不许出现在字段集合里。另断言文本不含 cookie/authorization/url/host/user-agent/referer。
+
+存储侧的每条都有对应守卫：按 target 限长 12（丢最旧、保最新）、`record_replacing` 保证"一个策略反复失败"不能挤占整个窗口（否则探索会被悄悄关掉）、空白 target 不入账（否则变成公共垃圾桶）、写入 tmp+fsync+rename 且不留 `.tmp`、版本不符→清空但**出声报告**（静默重置会被读成"从来没成过"）、坏 JSON→报错而不是当成空历史。
+
+**没接的东西（别让 207 骗人）**：这个引擎还没有任何调用方——daemon 没有把它接进启动/扫描流程，§29 诊断页的 Strategy 行仍然只看日志（所以多半仍是 `?`），把它变成实测需要先定 D-11（缓存经哪个方法进 UI）。也不涉及 §26 的实际执行切换：现在没有任何一条路径会因为 `select` 的结果而改变发包方式。**这一步交付的是判据和存储，不是自动模式能用了。**
+
 ## 交给用户的验收清单（按编号回报即可）
 
 在 `/Users/glow/Base/Code/Prometheus/.hibernal-experiment/spike-iced` 下：

## Gate Z — §28 自动模式接上：测量→记录→选择→起对应组件（214 passed / clippy clean）

+
+三处接线，每处都断言行为而不是措辞：
+
+1. **记录**（`himari-daemon/src/jobs.rs`）：扫描每完成一个候选就 `record_replacing(target, FakeSni{success, latency_ms, now})`，**先落盘再广播**进度；写不进去就记一条 `[core] the scan result for X was not saved for automatic mode: …`——持久化失败与测量失败是两件事，不许合并成"没事"。归属诚实：扫描器探测的是假 SNI 握手，所以结果只记在 `FakeSni` 名下；记到别的策略上就是给没走过的路记功。
+2. **选择并路由**（`Daemon::start_auto`）：`Selection` 决定**启动哪个组件**——`SniGate` 走 `gate_start()`，`FakeSni` 走监听器那条。守卫不看日志看行为：故意**不配**网关二进制，于是网关分支回它自己的 `HIMARI_SNI_GATE_BIN` 拒绝——这个拒绝就是路由到网关的证据；监听器分支断言 `SniSpoof` 离开 stopped（本机无特权最终 Failed，但进了哪条路是确定的）且另一组件仍 stopped。
+3. **三种诚实的拒绝**：没有 `HIMARI_AUTO_TARGET` → 报"需要目标，什么都没启动"，并断言三个组件都仍 stopped（拒绝不是嘴上说说）；可用策略全在窗口内失败 → `every available strategy failed …: fake SNI, sni-gate`，绝不重试已知失败；引擎若真给出不可用策略 → 报内部矛盾，而不是悄悄降级到别的路径。
+
+时钟口径也定了：缓存过期用**墙钟毫秒**（`now_ms()`），因为 §28 的 `timestamp` 必须跨重启可比；用单调时钟会让每次开机把窗口清零，旧失败于是永远看起来"新鲜"。版本不符/坏 JSON 在启动时经 `[core]` 日志出声报告（`StrategyLedger::note`），不静默当成"从来没测过"。
+
+7 条新守卫里两条跨重启：一条 terminate 后重新 bind，断言 `select` 仍然 `Reuse` 同一策略（证明缓存在磁盘上起作用，而不是进程内对象）；一条跑真扫描，按 target 断言每个被探测过的候选都留下记录。合计 `himari-daemon` 的 `tests/automatic_mode.rs`。
+
+**这轮仍未做的**：自动模式只会"在已有两条通路里选一条"，不会因选择而改变发包方式（§26/§27 的 fragmentation、disorder、TLS record 分段仍是保留位，受 D-4 挡）；§29 的 Strategy 行还没读这个缓存（卡在 D-11：经哪个方法进 UI）；`Selection` 的 `why` 目前只进日志，还没变成给用户看的那一行。
+
 ## 交给用户的验收清单（按编号回报即可）
 
 在 `/Users/glow/Base/Code/Prometheus/.hibernal-experiment/spike-iced` 下：

## Gate AA — §40/§41 macOS 打包与签名的可复现部分 ✅（219 passed）

+
+`scripts/build-macos-app.sh` + `packaging/macos/{Info.plist,himari.entitlements,notices.txt}`。跑通的是：**产物树组装 → 嵌套优先签名 → `codesign --verify --deep --strict` → hdiutil 出 dmg**，实测输出：
+
+```
+./Himari.app: valid on disk
+./Himari.app: satisfies its Designated Requirement
+wrote ./Himari.app and ./Himari.dmg
+```
+
+（`--verify --deep --strict` 是本机唯一有意义的判据：这台机关了 Gatekeeper，`open` 成功不说明任何事。）
+
+**§42 的"必须随包带许可"这条被写成了会失败的构建，而不是注释**：`notices.txt` 只列**名字**，版本从 `Cargo.lock` 现场解析、文件从 cargo registry/git checkout 现场找，任何一条解析不到或文件不存在就 **构建中止**。这一步当场抓出两个真问题：
+
+1. `gsub(/.*"|".*/, "")` 的贪婪匹配把整行吞掉，导致版本永远解析为空——所有 registry 许可都是"UNRESOLVED"，而我第一次跑只看到"3 个非 registry 项成功"就以为通了。加存在性检查后才现形。
+2. **产品仓里根本没有自己的 LICENSE 文件**：workspace 声明 `MIT OR Apache-2.0` 却没有文本，包里带不出我们的许可。已补 `LICENSE-MIT`/`LICENSE-APACHE`；MIT 的版权行我写的是 `Copyright (c) 2026 Himari contributors`（上游 sni-gate 的 LICENSE-MIT 连版权行都没有，无法继承），**这一行需要你来确认署名口径**。
+
+5 条契约守卫（`tests/macos_bundle.rs`）不重建包，只钉漂移：每条具名许可必须解析到真实文件、两个 plist 必须过 `plutil -lint`、`Info.plist` 的版本必须等于**包内那个二进制**的版本（workspace 无统一 version，读 `himari-ui` 自己的）、被签名的三个二进制必须都是真 workspace 成员且包含 helper、以及脚本必须确实使用嵌套优先与 `--verify --deep --strict`。
+
+**这轮没做也别说成做了**：公证（notarization）需要 Developer ID 与 App Store Connect API key，本机没有凭据，脚本只用 ad-hoc 签名（`HIMARI_SIGN_IDENTITY` 未设时为 `-`）；hardened runtime 未开，entitlements 是**空字典**并写了为什么（不写 sandbox、不写 network.* 这些只在沙盒里才有意义的键）；launchd 标签与 helper 的**安装**（§21 一次性提权）还没有；`.dmg` 里没有拖到 Applications 的窗口布局；Windows 侧（§41 portable+installer）完全未开始。
+
 ## 交给用户的验收清单（按编号回报即可）
 
 在 `/Users/glow/Base/Code/Prometheus/.hibernal-experiment/spike-iced` 下：

## Gate AB — §38 登录项：用户级 LaunchAgent，装得真、关得死（233 passed / clippy clean）


`himari-platform/src/launch_item.rs` + `Daemon::set_login_item`。**分工先纠正一处设计直觉**：登录项不该由 root helper 写——它在用户自己的 `~/Library/LaunchAgents`，让 root 去写普通用户的文件会把 §20 的"特权只做非做不可的事"反过来用，还会留下属主混乱。所以非特权服务自己写，helper 那两个字方法仍如实回 `not wired`。

14 条守卫（平台 10 + 服务 4），跑的是一个假 `launchctl`（记调用、`print` 的返回值由状态文件决定），所以既没碰真 launchd 也没碰用户的登录项：

- **plist 内容**：`RunAtLoad` + `KeepAlive` 只配 `SuccessfulExit=false`。这条是 §33/§36 的交叉点——崩了要拉起，用户主动退出**不许**被复活；断言同时拒 `KeepAlive` 写成无条件 `<true/>`。参数里的 `&` 必须被转义成 `&amp;`。
- **写前记账**：文件不存在走 `RemoveCreated`（undo 是删除，不是清空），已存在走 `RestoreFile`（undo 逐字节还给别人——守卫用一份"别人的 plist"验了这一点）。bootstrap 失败时记录停在 *未完成* 状态，`undo(true)` 能把半截安装收掉。
- **幂等**：第二次安装不写文件、不再 bootstrap、不新增账本记录。launchd 的"already bootstrapped"错误被当成达到目标而不是失败——**判据是状态而不是退出码**（与 Gate P/V 同一口径）。
- **关得死**：`remove` = bootout + 删文件 + `Ledger::discard_kind`。之后 `undo(false)`（关机路径）和 `undo(true)`（崩溃恢复）都拿不回它。

**这一轮真正抓到的是一处 §37/§38 的冲突**：`terminate()` 原本无条件 `undo(false)`，也就是**退出时会把用户要的登录项一起回滚掉**。修法是 `Ledger::undo_except(&["launch_item"])`——系统代理必须还原（§22），启动项必须保留（§38），同一个"退出时还原一切"的调用不能同时满足两者。守卫直接断言 terminate 之后的报告的 outcomes 里没有 `launch_item`，并且文件还在。

顺带得到一条意外的 §39 证据：测试里想在上一个 Daemon 还活着时再 bind 同一目录，直接被实例锁拒成 `AlreadyRunning`——锁的作用域（活到 Daemon drop，`terminate` 不释放）就是"不能有两个实例"的实现，测试作用域因此不是装饰。

**没做的**：`--check-notices` 之外没有 CI 接线；helper 的 `install_launch_item`/`remove_launch_item` 仍如实报未接线（root 侧 LaunchDaemon 还没设计完）；`.app` 内的 daemon 与 `Agent::current()` 的 current-exe 关系只在测试里验到"plist 里有当前可执行文件名"，没验 bundle 里的真实路径；Windows 的"登录自启"（任务计划/注册表）完全未开始。

## 交给用户的验收清单（按编号回报即可）

在 `/Users/glow/Base/Code/Prometheus/.hibernal-experiment/spike-iced` 下：

```
cargo run
```

1. 窗口是否出现，按钮是否有 **2px 硬边框 + 3px 右下偏移实心阴影**（无模糊、无圆角）。
2. **按住**按钮时：阴影是否**完全消失**。
3. cream 底 (#F4F1EA) 与按钮面色之间是否**一眼分层**（这就是 card≠window 那条门禁）。
4. 点 `Toggle Theme` 切到 dark：分层与边框是否仍然成立。
5. 若有任何一条不符，请指到具体编号 —— 我会把它当成风格 token 的约束写回 `design-language.md`，而不是调参数到看起来过。

§22 这边还有一条**只有你能定**的（我不会自己动手，因为它会瞬间改变你的出网路径）：

6. 允许我把 Wi-Fi 的 HTTP 代理开关真的 `on` 一下再立刻还原吗？现在能证明的只有"非特权调用返回 rc=0"，而 `preferences.plist` 纹丝不动，所以**「普通用户的 networksetup 写入到底落不落地」还是个未知**。你点头我就用你自己那份 127.0.0.1:7890 配置做往返验（前后各读一次并打印原文）；你摇头我就把它留作 §20 helper 的立项理由，实现上继续按"读不回就报 NeedsPrivilege"处理。

Shell cwd was reset to /Users/glow/Base/Code/Prometheus/Himari

## Gate AC — §29 的取数通道：`get_diagnostics`，诊断页的每个 ✓ 必须有出处（242 passed / clippy clean）

+
+**为什么现在动 §49 的方法表**：那一节写的是"至少包含"，所以加一个**只读**方法在授权范围内（D-11 我按这个口径先做了，不认同就回退，改动集中在 3 个文件）。真正的动机是诊断页里有几行**结构上不可能**由 UI 判断：当前网络服务在路由表里、策略证据在服务的本地缓存里、登录项在用户 LaunchAgents 目录里——按 §47/§48，UI 不许自己去碰 `networksetup` 或 BPF，于是那些行永远只能写 "?"。看起来诚实，其实是把已测到的东西挡在门外。
+
+落了什么：
+
+- `crates/himari-ipc/src/proto.rs`：`Request::GetDiagnostics`、`Response::Diagnostics{rows}`、`DiagnosticRow{area,verdict,why,remedy}`，`Verdict` 是**闭合的 5 值** snake_case 枚举（`ok/warn/fail/idle/unknown`）。
+- `Daemon::diagnostics()`（`crates/himari-daemon/src/lib.rs`）固定 8 行：runtime service / packet backend / sni-gate / Xray / system proxy / network service / strategy / login item。组件行由 `row_for` 从状态机 + `last_error_stage` 生成，`remedy_for(stage)` 把闭集阶段名翻成人能做的事；新增注入点 `Options::login_item`（`HIMARI_LAUNCH_AGENT_HOME` + `HIMARI_LAUNCHCTL`），**不注入就如实报 Unknown**，绝不去猜用户的主目录。
+- UI：`Intent::RunDiagnostics` 原本发的是 `GetStatus`（占位，等于什么都没问），现在发 `GetDiagnostics`；`Reply.rows → App.daemon_rows → overlay_measured`。
+
+**判定权的划分是这一轮的设计核心**：`runtime service` 这一行**不由服务覆盖**——它回答"后台服务答没答上"，而这件事客户端比服务自己知道得更准（回复要么到达了这个窗口，要么没有），服务的 `I'm answering` 不能替客户端的连接事实说话。其余 7 个区域服务侧的措辞**替换**本地那行 "?"，因为对面是观测、本地只是承认没测。`measured_area` 的映射表**故意不收** `"runtime service"`，并把这条豁免写进守卫。
+
+**过期即清**：任何一条不带 rows 的回复（例如随后的 `get_status`）都会把 overlay 整体换掉，`Unreachable` 直接清空。这是按需取数页面的默认失败模式——留着上一次量出来的 ✓，就是让一个已经没人复核的绿勾永久住在页面上。
+
+9 条新守卫（服务 6 + UI 3），每条都带阳性对照：
+
+- 全新服务：8 个区域**一个不少也不重复**，而且 `verdict == Ok` 的集合**恰好等于** `["runtime service"]`；`strategy`/`login item` 必须带 remedy（只说 "?" 而不说怎么填，等于没写这一行）。
+- 策略行：没证据时 `Unknown` 且点名目标；`strategy().record(FakeSni success)` 之后变 `Ok` 并把 `fake SNI` 写进 `why`——这把尺看得见"测到了"。
+- 组件失败行：`sni-gate` 无二进制时 `Fail` + `why` 含 `gateway-binary-missing` + remedy 含 `HIMARI_SNI_GATE_BIN`（阶段名→建议这条映射是被断言的，不是碰巧同名字串）。
+- 登录项：**阳性对照就是加一个文件**——同一个 daemon、同一个 label，plist 不存在时 `Idle`，把 plist 写进沙盒目录后必须离开 `Idle`；测的是沙盒 `home`，没碰用户真的 LaunchAgents。
+- 反伪造：`{"verdict":"measured_good"}`、`"OK"`、`null`、整个字段缺失**四种都必须拒**（`is_err`），5 个合法值必须收。这条是前一条的尺——如果闭合枚举其实宽松，"只有 runtime service 能变绿"就是装饰。
+- 线上名：`{"method":"get_diagnostics"}` 必须解析；`verdict` 序列化必须是 `"warn"`；帧头包一层 `ServerFrame` 也要能读回来（`codec_guards` 的方法清单同步 15→16）。
+- 跨仓漂移守卫放在 **UI** 侧并且**不抄名字表**：它起一个真 daemon、拿真 rows，逐条断言 `measured_area()` 认得（除豁免那一行）。把区域名誊进测试只会得到一个过期还能绿的清单。
+
+**没做也别说成做了**：诊断页的**视觉**仍未用眼睛验收（§30/§31），本轮只保证证据能走到 `assess()`；`Xray` 行永远只能是 Idle/Fail（§6 未实现）；`packet backend` 的"BPF 打开了其实没在跑"（§24 的 reopen 语义）仍无实测；`Dns`（§13）与 `SNI`（§43）两行还是 UI 本地判断，服务侧还没有对应行；没有加任何轮询——只有点 `Run diagnostics` 时取一次（§52 明令禁止激进轮询）；`Options::login_item` 在真实 `.app` 里由谁注入还没接（目前只有 `from_env`）。
+
 ## 交给用户的验收清单（按编号回报即可）
 
 在 `/Users/glow/Base/Code/Prometheus/.hibernal-experiment/spike-iced` 下：

## Gate AD — §9/§10/§11 可编辑端点：`set_config` 真的设了值，且被拒的写入不留半截（249 passed / clippy clean）

+
+`Request::SetConfig` 之前一律回 `not implemented`，`GetConfig` 返回的是一份**写死的默认值**——也就是说 GUI 的地址/端口/fake-SNI 表单从来没进过服务。这一轮把它变成"唯一真源"：`Daemon.endpoint` 存在服务里，`start()`、崩溃恢复、§23 的代理跟随、扫描的上游**全部改读 `self.endpoint()`**，`get_config` 也读同一个值；`lan_share` **不落字段**，由 listen 地址是否 unspecified 推导（存的 flag 和存的地址可以互相矛盾，那时另一边就是谎话）。
+
+校验规则（闭集字段名，未知键直接点名拒绝——被忽略的键会以"已应用"的口吻回读）：`listen_addr` 必须是地址字面量（不接域名）、`listen_port` 必须是 1-65535 的数字、`connect` 必须 `address:port`、`fake_sni` 走一条严格的主机名校验（ASCII、必须带点、标签 1-63、不许连字符端点、不许空格/斜杠/冒号/下划线）。SNI 会被写进 ClientHello，而引擎对坏值**静默不理**，所以宁可在这里拒。
+
+**§11 的暴露口径**：只有"从不可曝光变成曝光"才要 `confirm_lan: true`——已经曝光过的端点再做一次无关编辑不再重复索要确认（那是在惩罚服务自己记得的状态），回到 loopback 不需要确认。守卫的正向对照是：同一份 `lan_share: true` 加了确认之后必须真的变成 `0.0.0.0:40443`，否则上面那条拒绝守的是一个不存在的功能。
+
+7 条守卫（6 集成 + 1 单元），重点在**证据走到哪儿**：
+
+- `an_edit_that_reaches_the_service_also_reaches_the_scanner`：设 `connect=192.0.2.9:4431` 后起扫描，断言作业日志里出现 `probes 1 candidate(s) through 192.0.2.9:4431`。这条是给"存了但没人读"准备的——一个只进存储的配置能过掉本文件其余所有断言。为此 `jobs.rs` 在请求被消费前打一行上游名（§34 本来就该有）。
+- `one_refused_key_in_a_batch_blocks_the_whole_write`：好字段 + 坏字段同批到达时，整批不落（表单是一次提交的）。
+- `every_rejected_field_leaves_the_running_configuration_untouched`：12 个非法载荷逐个拒绝，**每个都回读整份 config 与改前逐字节相等**；punycode 那条单独验必须收（ASCII 规则不能误伤合法名字）。
+- `get_config_answers_with_the_values_the_service_would_start_from`：把 `get_config` 的答案和 `daemon.endpoint()` 读两次比对，钉死"只有一个真源"。
+- 单元 `the_listener_gate_opens_only_for_a_live_listener`：`listener_is_live` 这个闸门在 Running 时必须为真、Starting/Stopped/Failed 时为假。**为什么它是单元而不是端到端**：真正的监听器起不来（无特权），所以"运行中拒改"这一整条端到端分支在本机无法验证，只有判据本身被证过——这条限制写在这里，不写成已完成。
+
+**没做也别说成做了**：端点配置**只在内存**，重启即回默认值（§10/§53 的持久化还没做，落盘要先过同一套校验，不能让一份旧文件静默把 0.0.0.0 带回来）；GUI **没有 `confirm_lan` 控件**，所以在界面上填 `0.0.0.0` 只会被拒、无路可走（IPC/命令行能走通）；`Intent::ApplyLanShare` 仍未接线；§9 的路由列表/多监听器编辑没有实现（网关那份配置仍是单路由模板）；扫描日志里的上游不等于 §12 的"证明流量走过 Himari"，那还需要一条真的经监听器出网的证据。
+
 ## 交给用户的验收清单（按编号回报即可）
 
 在 `/Users/glow/Base/Code/Prometheus/.hibernal-experiment/spike-iced` 下：

## Gate AE — §10/§53 端点落盘：下一次开机用的就是上一次确认过的值（254 passed / clippy clean）

+
+`runtime_dir/endpoint.json`，由 `apply_config` 在**改内存之前**写（write→flush→sync→rename，半截文件不可能被读到）。顺序是刻意的：先换内存再落盘的话，"已保存"在读到磁盘之前只是一句许诺；写盘失败就整笔拒绝，返回 `Internal` 并说明没应用。
+
+读回来的路径过**同一套校验**（`persisted_endpoint`：字段名闭集、地址/端口/`fake_sni` 全走实校验、缺字段即拒），因为这个文件是唯一可以绕过 GUI 和服务校验抵达这里的输入——手改、崩溃截断、旧版本写的都算。校验不过就**明说并退回内置默认**（日志一行点名 `endpoint.json was rejected (…)`），服务照常起：一份坏设置不该变成用户今天没法绕过的理由。
+
+§11 的口径在这里有个真实张力，选了"照办但必须再说一遍"：确认发生在上一轮，而曝光的后果每一轮开机都在重演，文件里不带那次点击。所以恢复出的 `0.0.0.0` 会被**照用**，同时日志再喊一次 `restored from endpoint.json: the listener is on every interface (…)`。另一种做法（每次开机重新索要确认）会把 §53 的"重启自动恢复"变成"重启自动失效"。
+
+5 条守卫：
+
+- 往返验：写入→terminate→drop→同目录 rebind，`get_config` 必须等于上一轮确认的值，并**断言磁盘上那个文件存在**（只回读服务的话，"持久化"和"运气好的全局状态"分不出来）。
+- 被拒的写入不落盘：先存一个合法端口，再发一次未确认的 `lan_share`，文件里必须仍是那个端口，rebind 后仍是 loopback。
+- 阳性对照（曝光恢复）：`lan_share+confirm_lan` → rebind → `listen == 0.0.0.0:40443` **且**日志里有 `every interface`。少了这条，"必须再说一遍"就退化成一个永远不会红的断言。
+- 6 份畸形文件逐个投（非 JSON、地址不合法、端口 0、缺 `connect`、fake_sni 非法、多一个 `helper_socket` 键）：每一份都必须**既退回默认、又在日志里点名被拒**——"悄悄丢掉"和"宣布后丢掉"是两件事。
+- 阴性对照：全新目录 rebind 不得出现任何 `endpoint.json` 字样，也不得凭空造出一个文件（否则上一条的"宣布"就是噪声）。
+
+**没做也别说成做了**：这个文件不在 §37 的写前账本里（它是用户自己的设置，不是系统状态，`undo` 不应该把用户的 fake-SNI 还成默认值——但要真想清楚"卸载时删不删它"得连着 §26 的卸载流程一起定）；文件权限没做额外断言（runtime dir 已 0700）；GUI 侧仍没有能触发它的确认控件；跨版本字段迁移没有策略，一个未来加的字段今天会被当"未知键"拒掉——那是有意的严格，但也意味着**升级需要写迁移**。
+
 ## 交给用户的验收清单（按编号回报即可）
 
 在 `/Users/glow/Base/Code/Prometheus/.hibernal-experiment/spike-iced` 下：

## Gate AF — §34 结构化日志 + §49 `subscribe` 的过滤器（259 passed / clippy clean）

 
+需求书点了名的东西，之前有两件是假的。**一**：`Push::LogLine{ts_ms,component,event,error}` 在协议里定义了、`EventKind::LogLine` 在过滤器里也认，但**没有任何一条路径广播它**——环里存的是裸 `String`，§34 要的 `timestamp/component/event/domain/strategy/result/latency/error` 九个字段一个都不存在。**二**：`hub.add(Vec::new())` 在连接建立时就写死了"收全部"，`Request::Subscribe{events}` 被 `Accepted` 之后**直接丢掉**——客户端以为自己在订阅一类事件，实际什么都收。
+
+改完的样子：
+- 新的 `himari_daemon::log_ring`：`Class` 是闭集五类（`core/sni-gate/xray/platform/ui`，并额外认 `sni_gate` 这个下划线拼法，因为状态推送里的组件名就是那个），`Record` 带 §34 全部字段，`tail()` 渲染成 `[component] event domain=… strategy=… result=… latency_ms=… error=…`（保留前缀是为了终端里按眼过滤）。存一条 = 也广播一条 `Push::LogLine`。
+- 每条扫描完成都落成**字段**而不是句子：`domain/strategy/result/latency_ms` 由 `jobs.rs` 真填（§34 的字段只有在正确的槽位里才算存在）。
+- 入口只有一个的**脱敏**：`authorization`/`cookie`/`set-cookie`/`proxy-authorization`/`bearer `/`basic `/`token=`/`api_key=`/`password=`/`secret=` 命中即**从该点整段截断**并留 `[redacted]`；长度上限 400 字符再标 `…[truncated]`。截断是故意的：部分打码要猜秘密到哪里结束，猜错就是把秘密打出来。匹配用 `char_indices` 逐字符小写比较——`to_ascii_lowercase()` 整串会改变字节长度，拿它的偏移去切原串会切在多字节字符中间。
+
+两条守卫（各带反向对照）：`Authorization: Bearer super-secret-token-value` 与 `Cookie: sid=leakme-please` 必须既不进环也不进广播，而**同一条不含秘密的句子必须逐字节原样留下**（含 `Wi-Fi → en0` 这种非 ASCII），否则脱敏器只是个捣乱器；5000 字符的正文必须被截且**声明被截**。
+
+过滤器这条守卫是**双向**的：订阅 `[log_line]` 的连接必须收到配置变更那一行、并且**收不到**紧随其后的状态变化；另开一条只订 `[state_changed]` 的连接必须收到 `sni_gate` 的状态推送。单向断言（"过滤后什么都没来"）与"过滤器把一切都静音了"无法区分，所以两条一起写。
+
+**这轮真正值得记的是暴露顺序**：`LogLine` 一开始广播，`scan_guards` 那条老守卫就红了（"unexpected push"）——不是它变脆，是它一直靠"过滤器没生效"才能存在。把它改成显式订阅 `[scan_progress]` 之后又红第二次，才照出**第二个**缺陷：`kind_matches` 里 `ScanFinished` 不归 `ScanProgress`，于是按进度过滤的客户端**永远收不到终止帧**，只能靠超时区分"跑完了"和"没人喂了"——正是 §52 禁止的那件事。终止帧属于同一条流，现在两个一起匹配。改完全绿才说明这两处都是真修，不是把断言调松。
+
+**没做也别说成做了**：`xray` 与 `ui` 两类目前只有分类能力、没有真实生产者（§6 未实现；GUI 侧写进同一个 `Class` 的调用点还没有）；日志没有落盘（§45 的"logs 目录"与 GUI 的日志页仍走 IPC 内存环）；`Push::LogLine` 没带 `domain/strategy/latency`（协议里就只有那四个字段，扩它要过一次 §49 的口径）；订阅过滤器一旦设置就覆盖整条连接，没有"同一条连接换过滤器"的语义验证；脱敏是模式匹配，不是协议解析——一个不像 `key=value` 的秘密照样能穿过，这条限制写在这里而不是假装没有。
+
 在 `/Users/glow/Base/Code/Prometheus/.hibernal-experiment/spike-iced` 下：
 
 ```

## Gate AG — §13 `DnsBackend`：解析是组件不是调用点（263 passed / clippy clean）


需求书点名的抽象里 `DnsBackend` 之前**零实现**：`scan.rs` 直接 `lookup_host(&request.target)`。这条改动做两件事，第二件才是 §13 的重点。

**接缝**：`himari-runtime::dns` 定义 `DnsBackend`（对象安全靠返回 `Pin<Box<dyn Future>>`，不引 `async-trait`——多一个依赖要多过一次 §42 台账）＋ `DnsError::{NoAnswer, Resolver}`（"解析不出"与"解析器坏了"在 §29 是两个答案）＋ `SystemDns`（机器自己的解析器）＋ `FixedDns`（`new/failing/empty` 三种夹具）。`scan_with(request, gauge, on_event, dns)` 是真路径，`scan(...)` 只是"默认后端"的一行皮；`Options::dns` 让服务在启动时**选一次**，`JobBoard` 持有 `SharedDns` 并在 spawn 之前 clone 进去——探针不能自己决定用哪个解析器。

**安全侧**：`nothing_in_the_workspace_writes_the_system_dns_configuration` 把整棵 `crates/**.rs` 走一遍，禁止 `setdnsservers`／`Set Dnsservers`／`flushcache`／`/etc/resolv.conf`／`set dns server`。这把尺有两个必须写清楚的细节：
- **阳性对照**：同一次遍历必须能找到 `setwebproxy`（我们真的调它）。找不到就说明遍历是空的，"零命中"就什么都不是。这条断言挡住的正是"扫描器什么都没读到却报告干净"。
- **禁的是绝对路径而不是文件名**：`journal_guards.rs` 里有一个 `dir.join("resolv.conf")` 的沙盒夹具，用来证明写前记账的顺序。第一版按文件名禁，把它判成违规了——那是夹具不是系统配置。§13 禁的是碰**真的那一份**。

其余三条守卫：地址字面量必须**从字符串直接回答**（不走查询，否则"后端决定了地址"和"解析器今天心情好"分不开）；`localhost:443` 必须回读到环回地址（本机解析器几乎什么都答，光断 `is_ok()` 是废尺）；`Counting` 包装器断言 `queries == 1`（证明扫描问的是**注入的那份**），并用文档段 `198.51.100.7` 做反向对照——出现 usable 就说明注入没生效、流量真出网了；`FixedDns::failing` 必须产 `TargetUnresolved` 且 `reason` 里带上后端名 `fixed`，`empty()` 必须产 `NoAnswer`（两种失败不许合并）。

**没做也别说成做了**：`Options::dns` 只能由代码注入，没有环境变量口径（`HIMARI_DNS_*` 未定）；扫描器内部对**候选 SNI** 的名字仍不做解析（候选是字面量列表，符合 §43 的自建语料方向，但不是所有场景）；Xray 的 DNS 设置（§6 的 `DnsBackend` 另一半）没有实现；`Push::LogLine` 里没有后端名；"DNS 失败不许打断系统解析"目前是靠**没有代码路径**＋源码扫描保证的，不是靠一次真实的故障注入验过系统状态未变。

## 交给用户的验收清单（按编号回报即可）

## Gate AH — §34/§37/§40 日志落盘：磁盘不许是脱敏的绕行路（267 passed / clippy clean）


`runtime_dir/logs/himari.log`，一条记录一行 NDJSON（八个字段与环里那套同名）。为什么值得落：§37 说崩溃恢复要有历史，§40 说卸载要删得掉——两者都要求"有一个固定路径的文件"，而不是内存里那个 1000 条的环。开在 `bind_with_options` 里（`enable_file`），失败就**明写**日志文件没启用，而不是让日志页以为背后有文件。

三条口径：
- **磁盘不许绕过脱敏**。上一轮的门禁只保护环，那正好留下最常见的旁路：内存干净、文件里全是被禁的东西。守卫读**原始字节**，断言 token 和 cookie 都不在文件里，且两条都带 `[redacted]`；同时非秘密行必须原样在（否则这就是个只会红的尺）。
- **有界**。超过 1 MiB 轮转一次成 `himari.log.1`，只留一份备份；`write_line` 打开失败就把文件**关掉并说明**（不是逐行重试——盘拒你一次就会永远拒你，而一路追加抱怨的环会把真正的行挤掉）。那条"禁用"记录直接进环、不走 `record()`，否则写失败会递归地去碰同一个文件。
- **没开就是没开**。`LogRing::default()` 的环照样存记录，但目录里不许出现 `logs/`——这条阴性对照保证"落盘"是被 enable 出来的，不是构造时偷偷做的。

守卫共 4 条（`structured_logs.rs` 现在 9 条）：字段级回读（JSON 解析后逐字段比对，含 `ts_ms > 0`）、脱敏旁路（如上）、有界（写到触发轮转，断言 `.1` 存在且活动文件不超过上限 + 一行余量）、**服务级**：真 bind 之后 `runtime_dir/logs/himari.log` 必须存在、必须含 `config applied` 那行、每行都得是能解析的 NDJSON（证明产品路径而不是单元自嗨）。

**没做也别说成做了**：写入不逐行 fsync（长会话的性能代价换来的后果是崩溃可能丢最后几行；系统状态的持久性由 §37 的写前账本负责，不是由这份日志负责）；只留一份备份，没有按天/按数量轮转；GUI 侧没有"打开日志目录"的按钮（§45 的目录已存在但入口未接）；`Push::LogLine` 仍只带四个字段，文件里反而更详细——两者口径不同，读日志页的人看不到字段；日志文件属主/权限只受 runtime dir 0700 保护，没有单独断言。
MD
python3 - <<'PY'
import re,io
p="docs/audits/00-index-and-open-decisions.md"
s=open(p,encoding="utf-8").read()
s=s.replace("**263 passed / 0 failed / 0 ignored**","**267 passed / 0 failed / 0 ignored**")
s=s.replace("AA–AG（§40–§42、§38、§29 取数通道、§9–§11 可编辑端点、§10/§53 端点落盘、§34 结构化日志与 §49 订阅过滤、§13 `DnsBackend`）",
            "AA–AH（§40–§42、§38、§29 取数通道、§9–§11 可编辑端点、§10/§53 端点落盘、§34 结构化日志与 §49 订阅过滤、§13 `DnsBackend`、§34/§37/§40 日志落盘）")
open(p,"w",encoding="utf-8").write(s)
print("267" in s, "AA–AH" in s)
PY
rg -n "^

## Gate AI — §47/§48/§52 层次禁令变成门禁（270 passed / clippy clean，且证过"违规必红"）


`himari-ui` 的 Cargo 描述里写着"never touches BPF, PF or WinDivert (brief §48)"，但那是一句**散文**：第一个为了方便而加进按钮处理函数的 `std::process::Command` 就能让它变成假话，而且没有任何检查会响。这一节把它变成机器检查。

`crates/himari-ui/tests/layering_guards.rs`，三条：

- **禁止项**：UI 的 `src/**.rs` 里不许出现**代码形状**的越权词——`himari_platform::`、`himari_runtime::`、`sni_spoof_rs`、`SpoofGuard::`、`NetworkSetup::`、`launch_item::`、`Ledger::open`、`std::process::Command`；`Cargo.toml` 里不许声明 `himari-platform`/`himari-runtime`/`sni-spoof-rs`/`windivert` 这四条边（依赖边是让调用**能链接**的东西，比调用点更难藏）。选词只取代码形状是有原因的：本 crate 的正当文案里确实会写"BPF/WinDivert 没打开"来解释某行为何是 `?`，按词面禁会在下一次写好听的 tooltip 时被改成假门禁。
- **阳性对照（同一次扫描）**：同一批禁止词拿去扫 `himari-daemon/src`，`himari_platform::`／`SpoofGuard::`／`NetworkSetup::`／`launch_item::` **必须都命中**；`std::process::Command` 必须在 `himari-gate/src` 命中；依赖扫描必须在 daemon 的 manifest 里看见 `himari-platform`。没有这一条，"零命中"可能只是扫描器什么都没读到。
- **正向半边**：UI 的 `src` 必须真的出现 `himari_ipc::` 与 `Request::`——证明"去掉了直连"不等于"按钮什么都不做"。

**这把尺自己被证伪过**：临时放一个含 `std::process::Command` 的 `.rs` 进 UI 的 `src`，守卫立刻红并点名文件与词（`…zz_layering_falsification.rs: std::process::Command`），删掉后复跑三条全绿；临时文件已确认不在工作树里（`git status` 只剩本轮之前就存在的改动）。门禁只有"违规必须红"被演示过，才不是装饰。

**没做也别说成做了**：它只看源码文本，不看编译期的实际可达性（`pub use` 转发出来的别名、宏拼接出的路径都能绕过形状匹配）；没有覆盖 `flutter/` 那棵并行工程（D-10 的另一条路线，本会话不碰）；UI 里"进程/权限"类能力目前也确实没有替代入口（§4 的托盘/提权适配器等 helper 接线做完才有地方放）；正向半边只断言"出现了 IPC 词"，不是"每个按钮都走 IPC"。
MD
python3 - <<'PY'
p="docs/audits/00-index-and-open-decisions.md"
s=open(p,encoding="utf-8").read()
s=s.replace("**267 passed / 0 failed / 0 ignored**","**270 passed / 0 failed / 0 ignored**")
s=s.replace("AA–AH（","AA–AI（")
s=s.replace("§13 `DnsBackend`、§34/§37/§40 日志落盘）","§13 `DnsBackend`、§34/§37/§40 日志落盘、§47/§48 层次门禁）")
open(p,"w",encoding="utf-8").write(s)
print("270" in s, "AA–AI" in s)
PY
rg -n "^

## Gate AJ — §11 的 LAN 共享开关：按下必须真的移动监听器（274 passed / clippy clean）


`Intent::ApplyLanShare(_)` 之前和 `ApplySystemProxy` 一起躺在"未接线 ⇒ `None`"的清单里。也就是说**页面上那个开关是个装饰品**：它翻动 `app.lan_share`、诊断页据此多写一句"LAN 共享已开"，但运行时收到的请求数是 **0**。上一轮把服务端 §11 的确认门做完之后，这条缺口就从"待办"变成了"自相矛盾"——门在，门铃没接。

接法与理由：`ApplyLanShare(on)` → `SetConfig{"lan_share": on, "confirm_lan": on}`。把**翻开关本身**当作那次刻意动作（警告文字就长在开关上），关掉则不需要确认。手工在地址框里敲 `0.0.0.0` 仍然被服务拒——**强制在服务侧，不在前端**（§48）。

4 条守卫（`crates/himari-ui/tests/lan_share_wiring.rs`），每条都堵一种"看起来接好了"的假：
- **端到端**：`App::reduce(LanShareToggled(true))` → 断言意图恰是 `ApplyLanShare(true)` → `to_request` 必须是 `Some` → 交给**真 daemon** → 回读的 config 必须是 `0.0.0.0:40443` 且 `lan_share: true`；再按一次关，必须回到 `127.0.0.1:40443`。消息只改本地状态、或请求根本没发出去，都在第一条就断。
- **唯一入口**：同一份 daemon 上，`{"listen_addr":"0.0.0.0"}`（无确认）必须被拒且 config 原样不动；同样的曝光经开关必须被接受。只有"拒"没有"接受"的话，上一轮那条拒绝守卫就只是在守一个不存在的功能。
- **反向对照**：桥里必须**仍有**未接线的意图（`ApplySystemProxy`／`SetAutostart` ⇒ `None`），否则"开关发出了请求"这句话毫无信息量——它可能只是全都连上了。
- **不许顺手 latch 状态**：连按两次之后 `get_status` 里所有组件必须仍然 `stopped`，并先断言 `components` 非空（空表上"全都 stopped"是假的绿）。

**没做也别说成做了**：这是**逻辑接通**，按钮/开关在窗口里的**视觉位置仍未用眼睛验收**（§30/§31 的稿子还没定）；`ApplySystemProxy` 与 `SetAutostart` **仍未接线**（§22 的开关、§38 的登录项在 GUI 上按了没用，服务侧能力已具备但缺 IPC 方法——加方法要过一次 §49 口径，我没擅自扩）；曝光的确认只在这一个开关上，`ApplyEndpoint` 表单**不带** `lan_share`，所以"改端口"与"要不要保持曝光"的交互仍靠服务记住上一次的值；§23 的运行时重探测会在路由变化时把代理跟着走，但它与这个开关的相互关系没有专门的守卫（目前只有 daemon 侧 §23 那组测试覆盖）。
MD
python3 - <<'PY'
p="docs/audits/00-index-and-open-decisions.md"
s=open(p,encoding="utf-8").read()
s=s.replace("**270 passed / 0 failed / 0 ignored**","**274 passed / 0 failed / 0 ignored**")
s=s.replace("AA–AI（","AA–AJ（")
s=s.replace("§13 `DnsBackend`、§34/§37/§40 日志落盘、§47/§48 层次门禁）","§13 `DnsBackend`、§34/§37/§40 日志落盘、§47/§48 层次门禁、§11 LAN 开关接通）")
open(p,"w",encoding="utf-8").write(s)
print("274" in s, "AA–AJ" in s)
PY
rg -n "^

## Gate AK"`），别把工具回执当留存；要注进文本的变量先证非空，并且"跑测取数"与"写进文档"分两条命令。

48	
49	三问的现状：
50	
51	- **D-9 仍未决**：普通用户执行 `networksetup -setwebproxystate … off` 返回 rc=0，但 `preferences.plist` 的 mtime/大小前后一致——**"非特权写入是否落地"未被证明**（证明它需要改动用户真实的网络配置，故未做）。代码按"写完必须读回；读不回报 `NotApplied`→`NeedsPrivilege`"处理，等 §20 特权 helper 收口。
52	- **D-10 已由用户拍板**（见 [[himari-flutter-frb-bridge]]）：走 Flutter+FRB，需求文档 §3"必须 Iced"作废、Iced 那套并行保留。本会话只在 `crates/himari-*` 落实现，不碰 `himari-app/flutter/`。
53	- **D-11 我按"§49 是下限（原文写"至少包含"）"的口径先做了**：新增只读方法 `get_diagnostics`，没有扩 `get_status` 字段（那会把状态帧变成杂物间）。这条已在 `docs/audits/00` 标注为可整体回退。
54	
55	**还有一件等用户答的事（问过两次都没答，别当成许可）**：`crates/himari-{daemon,platform,strategy,helper}` 与 `ipc/proto.rs`、`ipc/lock.rs`、UI 的 `bridge/client/diagnostics` 等**从未被跟踪过**——也就是说这几百条测试整个都在未提交的工作区里。第一次提交涉及"提交到 `main` 还是兄弟会话的 `flutter-frb`"，而 Butler 未提交桶是全树共享的（见 [[himari-flutter-hook-and-shared-bucket-traps]]），所以我**没有自行发起**，只在每轮末尾复述一次。用户回一句"继续干活"不等于回答这个问题。
56	
57	**Why:** 这三条都是"能测但不该我替用户决定"或"要动用户真实状态才能测"的事，硬猜会直接违反需求文档 §22（不许弄坏用户网络配置）与 §52（不许为省事而过度抽象/绕过契约）。
58	
59	**§20/§21 特权 helper 的契约与边界已单独成文**：`Himari/docs/privileged-helper.md`（2026-10-04 写）。要点：helper 只装"OS 不肯让普通用户做的写操作"（系统代理 / BPF / launchd 标签 / PF-WinDivert），**不做策略决定、不缓存 §23 的服务名**；`NotApplied → NeedsPrivilege` 是它现在唯一的入口；它的任何持久变更必须先过 `Ledger`（`undo_kind`），卸载时先恢复一切、最后删自己。sni-gate 的 CA **没有卸载 API** 且指纹跨平台不一致（Windows SHA-1 / 其余 SHA-256），所以信任锚清理不能拿指纹当键 —— 这条是显式记账的未解项，别在实现时才发现。
60	
61	**How to apply:** 接续时先读 Gate 台账和这三问，**不要重跑已记录的实测**（除非环境变了）；任何新能力都要同时带来由该 Gate 承担的证据 + 至少一条证伪守卫。三问未答时的默认路线：把 helper 当**必需项**开工（因为 §53 的"打开即用"没有别的路径），Flutter 目录不碰。§29 的诊断行不再由我停在 `?`：`get_diagnostics` 会把服务侧实测覆盖进 UI（`runtime service` 那一行故意**不**被覆盖——能不能连上由客户端判定更准；任何不带 rows 的回复或 Unreachable 都会清空覆盖层，未注入的输入仍如实报 `Unknown`）。
62	
63	**§11 曝光 vs §53 自动恢复的张力，已定的口径是"照办、但每次再说一遍"**：`endpoint.json` 里存过 `0.0.0.0` 的端点重启后**照用**（确认发生在上一轮，文件里不带那次点击），但恢复时日志必须再喊一次"on every interface"。反面做法（每次开机重新索要确认）会把 §53 的"重启自动恢复"变成"重启自动失效"。设置文件**不进** §37 写前账本：它是用户自己的设置，`undo` 不该把用户选的 fake-SNI 还成默认值；"卸载时删不删它"要连着 §26 卸载流程一起定，尚未定。与 [[feedback-testable-unknowns-must-be-measured]]、[[feedback-cask-claims-must-be-true-not-green]]、[[feedback-gauges-need-positive-controls]]、[[ref-macos-network-service-and-default-route]] 一起用。
64

## Gate AL — §22 的系统代理开关：设置而不是"现在就改"，并实测了本机的权限边界（281 passed / clippy clean）


先量了一件一直被我当已知、其实没证过的事：**这台机器起不起监听器**。一次性探针（跑完即删）给出：

```
PROBE start=Error { code: NeedsPrivilege, detail: "failed to open raw socket: no available /dev/bpf device — Run with sudo so the BPF device can be opened." }
PROBE proxy_state=failed
```

这条实测把两处东西钉死了：`the_system_proxy_follows_the_listener_either_way` 在本机走的是**左支**（失败 ⇒ 不许碰系统代理），它的右支（"Yes/40443/账本一条"）在有权限的机器上才会执行；§24 那句"BPF 打开成功但其实没在跑"在本机**没有验证条件**。探针顺带照出一个小口子：Start 失败时日志环是空的——失败原因只在响应与状态阶段里，不在 `[platform]` 日志里（诊断行有 stage＋建议，所以不是丢信息，但"日志里读不到失败"这点值得记）。

据此把 §22 那个开关接成**设置**而不是立即改写：`set_system_proxy{enabled}` 翻 `proxy_allowed`，`apply_system_proxy` 在写之前问它（纯函数 `may_point_system_proxy(configured, allowed)` 有真值表：两个输入各自都能单独翻）。语义理由：把系统代理指向一个没起来的监听器正是 §22 要防的坏法；而**关掉会立刻还原**（只还原真取过的前值快照）。没注入服务名的服务**拒**并点名 `HIMARI_SYSTEM_PROXY_SERVICE`。回读用 `get_config` 的 `system_proxy` 字段——它现在由"配了服务 ∧ 开关开着"推导，不是第三个存储值。

3 条集成守卫的关键是**让被读者自己作证**（不需要特权监听器）：
- 开着时直接调 `apply_system_proxy(endpoint(40991))` ⇒ 假 networksetup 的 `calls.log` 里必须出现 `40991`；
- 关掉后**同一个调用**打另一个端口 `40992` ⇒ 调用序列必须与关之前**逐条相等**，且不得出现 `40992`；
- 再打开后用第三个没人试过的端口 `40993` ⇒ 写入必须回来（避免"看起来生效"其实是上一段的残留）。
- 反向对照：`turning_the_switch_off_never_invents_a_write`——关开关本身不许对 networksetup 发出任何 `-set…`（没有前值快照就没有还原的资格，§22 的"先存后还"是双向的）。
- 无服务名 ⇒ 拒，且 `get_config` 的字段仍是 false。

**没做也别说成做了**：真机右支（监听器起来 ⇒ 系统代理被指过来、账本记一条、Stop 后还原）在本机**不可执行**，需要 sudo 或 helper；`proxy_allowed` 不持久化（`endpoint.json` 里没有它，重启回默认"开"）；GUI 侧仍只是把意图变成一次 IPC 调用，**开关在窗口里的位置没验**；`ApplyLanShare`/`SetAutostart`/`ApplySystemProxy` 三个开关现在都会真发消息，其余（`InstallTrustAnchor`/`PromoteScanResult`/`SetXrayBinary`/`TestStrategy`/`RunScan(Builtin)`）仍在"如实报未接线"的清单上。
MD
python3 - <<'PY'
import io
p="docs/audits/00-index-and-open-decisions.md"
s=open(p,encoding="utf-8").read()
s=s.replace("**277 passed / 0 failed / 0 ignored**","**281 passed / 0 failed / 0 ignored**")
s=s.replace("AA–AK（","AA–AL（")
s=s.replace("§11 LAN 开关接通、§38 自启开关接通）","§11 LAN 开关接通、§38 自启开关接通、§22 系统代理开关（含本机 BPF 权限边界的实测））")
open(p,"w",encoding="utf-8").write(s)

q="docs/audits/09-session-handover-2026-10-04.md"
h=open(q,encoding="utf-8").read()
h=h.replace("cd /Users/glow/Base/Code/Prometheus/himari-app","cd /Users/glow/Base/Code/Prometheus/himari-app")
h=h.replace("  cargo test --workspace          # 277 passed / 0 failed / 0 ignored","  cargo test --workspace          # 281 passed / 0 failed / 0 ignored")
h+="""
## 补充（Gate AL，同日稍后）

- **实测边界**：本机 `/dev/bpf` 打不开 ⇒ `SpoofGuard::start` 必失败（`NeedsPrivilege`）。因此 `the_system_proxy_follows_the_listener_either_way` 在本机只走左支；右支与 §24 的 BPF 生命周期**需要 sudo 或 helper 才有验证条件**。这条也解释了为什么 §22 的 GUI 开关被实现成"设置 + 关掉即还原"，而不是"立即改写系统代理"。
- **`cargo test --workspace` 必须带 `HIMARI_SNI_GATE_BIN`**：我本会话漏了两次，漏了就是 2 条故意 panic 的红（不是代码坏）。
- §22 / §11 / §38 三个 GUI 开关现在都会真发 IPC；清单上剩下"按了没用"的是 `InstallTrustAnchor`、`PromoteScanResult`、`SetXrayBinary`、`TestStrategy`、`RunScan(Builtin)`，它们对应的服务侧能力本来就未实现。
- 下一步优先级改为：① §12 诚实版 `check_egress`（现在更明确了：无 live 监听器在本机是**必然**状态，所以"报错而不是回 via_himari:false"这条守卫可以完全验）；② `proxy_allowed` 持久化；③ D-9。
"""
open(q,"w",encoding="utf-8").write(h)
print("281 in 00:", "281 passed" in s, "| AL:", "AA–AL" in s, "| handover:", "Gate AL" in h)
PY
rg -c "^## Gate " docs/audits/08-phase3-gates-evidence.md; rg -n "^

## Gate AM — §40/§45 卸载清理：删自己造的东西，先还原再删账本（285 passed / clippy clean）


新增 `crates/himari-daemon/src/cleanup.rs` + `Daemon::uninstall()` + `himari-daemon --uninstall`（§45 的"卸载清理"到这才有个能跑的入口）。

**清单是名字表，不是"清空目录"**：`himari.lock`／`himari.sock`／`strategy-cache.json`／`endpoint.json`／`journal`／`logs`／`gate`。runtime dir 是 `0700` 的用户目录，人完全可能往里放自己的文件；§45 要的是清理，不是抹掉。守卫里就摆了一个 `my-notes.txt`：它必须**不在**删除表上、卸载后**还在**、且出现在 `report.kept` 里（报了才叫可核对）。

**顺序是这条的主要风险**：journal 是唯一记得"用户原来指哪儿"的东西，先删它就等于"卸载完机器还指着已经不存在的 Himari"（§22 换个名字的同一场事故）。`uninstall()` 因此是 `terminate()`（还原代理/停子进程/回放账本）→ 删登录项 → 最后删文件。

**`--uninstall` 必须打印它要删什么**：CLI 把 `removed`/`kept`/`problems` 三个列表逐行输出，退出码按 problems 判。`uninstall_is_safe_to_describe_without_running_it` 断言"打印用的 `present()` 与真删用的表是同一个函数"——列出 N 项就必须删掉 N 项，否则打印是装饰。

4 条守卫里值得记的三件事：

1. **产品缺陷（已修）**：登录项是在列清单**之前**删的，所以 `present()` 再也看不见它，`report.removed` 里居然没有那个 plist——正是用户最会去查的那个文件。现在显式记账。这条红不是测试写错，是报告**少报**。
2. **夹具差点让守卫变成空转**：还原那条我先只喂了 `http_enabled=No` 与 `http_port=8080`，结果还原走了"没东西可还"分支（`Endpoint::configured()` 要 server 非空），端口留在 40443 而断言炸了。**补上 `http_server=127.0.0.1` 之后**，这条才第一次真正端到端覆盖"服务原本关着但配过端口"的还原：先 `-setwebproxy … 8080` 再 `-setwebproxystate … off`。顺序判据也在这里：`journal` 必须出现在删除表上，而 calls.log 里必须有那次还原。
3. **§38 与 §45 的分界**被写成一条守卫：同一个 daemon，`terminate()` 之后 plist **必须还在**（用户要的启动项不能被关机回滚掉，Gate AB 的结论），`uninstall()` 之后**必须没了**且 `bootout` 被调用过——两种结局都断，才说明"保留"与"删除"不是同一句话的两次执行。

**没做也别说成做了**：**信任锚删不掉**——上游 sni-gate 没有提供从系统钥匙串撤 CA 的 API（§2/§14 记过），所以 `--uninstall` 无法还原 CA 安装，这一条必须在卸载文案里对用户明说，不是代码能解决的；`.app` 本体（`/Applications/Himari.app`）与 Xray 二进制不在本清单内（前者是安装器的活，后者 §6 未实现）；`--uninstall` 只在**没有别的服务实例持锁**时安全（它会 terminate 自己，不会去叫醒别人）；`proxy_allowed`/端点的清理只删文件，不删文件所属的偏好域（用户级 defaults 没写过东西）；卸载后运行目录本身仍在（保守选择：删目录会连带删掉用户放进去的文件，与第 1 段的原则冲突）。
MD
python3 - <<'PY'
p="docs/audits/00-index-and-open-decisions.md"
s=open(p,encoding="utf-8").read()
s=s.replace("**281 passed / 0 failed / 0 ignored**","**285 passed / 0 failed / 0 ignored**").replace("AA–AL（","AA–AM（")
s=s.replace("§22 系统代理开关（含本机 BPF 权限边界的实测））","§22 系统代理开关（含本机 BPF 权限边界的实测）、§40/§45 卸载清理）")
open(p,"w",encoding="utf-8").write(s)
q="docs/audits/09-session-handover-2026-10-04.md"
h=open(q,encoding="utf-8").read()
h=h.replace("  cargo test --workspace          # 281 passed / 0 failed / 0 ignored","  cargo test --workspace          # 285 passed / 0 failed / 0 ignored")
h+="""
- **Gate AM（§40/§45）**：`himari-daemon --uninstall` 已存在；删除表是**固定名字表**（`crates/himari-daemon/src/cleanup.rs::RUNTIME_ARTIFACTS`）。**已知无法自动还原的东西：sni-gate 装的 CA 信任锚**（上游没有撤销 API），卸载文案必须如实说出来。
"""
open(q,"w",encoding="utf-8").write(h)
print("285:", "285 passed" in s, "| AM:", "AA–AM" in s, "| handover:", "Gate AM" in h)
PY
rg -n "^

## Gate AN — §52 反「巨型单文件」：daemon 的 lib.rs 从 1786 行拆到 1259（285 passed / clippy clean）


`crates/himari-daemon/src/lib.rs` 是全仓唯一超千行的文件（其余最大 514），§52 明确把"巨大的单个 Rust 文件"列为禁止项。按职责搬出四个模块，**行为零改动**（同一套 285 条守卫前后都绿）：

| 新模块 | 装什么 | 行数 |
|---|---|---|
| `settings.rs` | 端点默认值、SNI/地址/端口校验、`persisted_endpoint`、`is_unspecified`、`may_point_system_proxy`、`listener_is_live`，以及 `endpoint()` / `config_json()` / `apply_config()` / `save_endpoint()` | 307 |
| `diagnostics.rs` | §29 的 `diagnostics()` 与 `row_for()` | 190 |
| `gating.rs` | §14 的 `gate_plan` / `gate_paths`，以及 helper/平台错误到 `ErrorCode` 的翻译与 `remedy_for` | 85 |
| `cleanup.rs` | （Gate AM 新增）卸载的名字表与报告 | 155 |

搬的是**能独立读懂的东西**（校验规则、错误码翻译、行渲染），`handle`/`serve`/状态机留在 `lib.rs`。字段可见性改成 `pub(crate)`（不再是模块私有）是这次唯一的结构性妥协：跨模块的 `impl Daemon` 必须能碰到 `options/endpoint/logs/strategy/proxy_prior/…`；`pub(crate)` 把暴露范围限定在本 crate，且写在这里而不是藏在 diff 里。

**方法**：按"顶层项遇到 `}`、方法遇到 `    }`"整块搬，搬完让编译器逐项点名缺哪个导入、哪个可见性。**中途踩到一个真陷阱**：我写了个小脚本按 clippy 的 "unused import" 自动删行，而当时 crate 还有编译错误——crate 有错误时报出来的"未使用导入"是**假信号**（类型还没解析成功），脚本于是把 `use crate::hub::Hub;` 和 `use himari_strategy::{Selection, Strategy};` 当成没用删掉了，而它们是需要的。教训一句：**清理告警必须等编译干净之后做**，否则删掉的是必需品。剩下两处（一个孤儿 doc 注释、一处 re-export）手工修回。

**判据**：拆前拆后都是 `cargo test --workspace` **285 passed / 0 failed**（带 `HIMARI_SNI_GATE_BIN`），`cargo clippy --workspace --all-targets` 无任何自有告警（只剩第三方 `block v0.1.6` 提示）。这条"数字不变"是关键：重构若改掉了行为，测试数会动、或某条守卫会红。

**没做也别说成做了**：`lib.rs` 仍有 1259 行，`start/stop/guard_*` 那约 300 行守护逻辑还可以再出一个 `guardian.rs`（本轮没动，因为剩下的轮次要留给功能）；新模块里的 `pub(crate)` 字段访问让"谁能改状态"的边界比原来松（虽然仍限于本 crate）；`himari-ui` 的 `app.rs`(430)/`diagnostics.rs`(511) 未动。
MD
python3 - <<'PY'
q="docs/audits/09-session-handover-2026-10-04.md"
h=open(q,encoding="utf-8").read()
h+="""
- **Gate AN（§52）**：`himari-daemon` 的 lib.rs 已按职责拆出 `settings.rs`／`diagnostics.rs`／`gating.rs`／`cleanup.rs`（1786 → 1259 行，测试数不变 285）。**踩过的坑值得接手的人知道**：在 crate 还有编译错误时按 clippy 的 "unused import" 自动删行会删掉**必需**的导入（那时类型还没解析成功，报的是假信号）——清理告警一定等 `cargo build` 干净之后再做。
"""
open(q,"w",encoding="utf-8").write(h)
p="docs/audits/00-index-and-open-decisions.md"
s=open(p,encoding="utf-8").read()
s=s.replace("AA–AM（","AA–AN（").replace("§40/§45 卸载清理）","§40/§45 卸载清理、§52 拆分巨型文件）")
open(p,"w",encoding="utf-8").write(s)
print("AN ok:", "AA–AN" in open(p,encoding="utf-8").read())
PY
rg -n "^

## Gate AO — §44/§45/§12：编造的测量从"不该做"变成"做不出来"（288 passed / clippy clean）


先量上游：钉住那个 commit 的 checkout 里，`sni_spoof_rs::RunningProxy` 的公开面只有 `is_running / stop / wait / request_stop`——**没有任何计数器**。而服务原来对 `get_stats` 的回答是 `Stats { connections: 0, bytes_in: 0, bytes_out: 0 }`：一个形状漂亮、语气平静的**零**，前端会照着画成"0 条连接"。§44/§45 不许假声明，这条就是假声明本身——它不是"还没实现"，是"实现成了谎"。

改动：

- `get_stats` → `Error{BackendUnavailable, NO_STATS}`，原因里点名 `RunningProxy` 与"只报理由不报零"。
- `check_egress` → 从**通用**的 "not implemented in this build" 换成点名缺口的 `NO_EGRESS_PROOF`（既没有可穿过的 live 监听器，核心也不计数，所以外部服务返回的地址**无法归因于 Himari**，§12 明确拒绝把这种地址当证据）。
- 诊断页新增一行 `egress proof`：`Unknown` +  remedy 说要经特权 helper 起来再复核。缺口必须**占一行**才有地方被读；没有行就等于没说。
- UI 侧同步加 `Area::EgressProof` 与 `measured_area("egress proof")` 映射——这条是被上一轮的**跨仓漂移守卫**逼出来的：服务新增区域而前端没接，那条守卫就会红（它红了，说明它活着）。

三条守卫（`honest_gauges.rs`）关键在两个"不许怪协议"的对照：`Response::Stats{7,8,9}` 与 `Response::Egress{…}` **仍能完整序列化/反序列化**——也就是说现在的拒绝是对**本构建的陈述**，不是类型缺失导致的假拒。另一条断言拒词里**不许**再出现通用桩字符串（"not implemented in this build"），否则等于换了个说法继续隐瞒。最后一条断言新行存在、是 `Unknown`、且**不可能**是 `Ok`。

**结构性结果**（比任何单条断言都强）：全仓 grep `Response::Stats {` / `Response::Egress {` 的**构造点已经为零**——除测试夹具外，代码里没有任何路径能产出一个编造的测量。这从"约定不要写"升级成"写不出来"。

**没做也别说成做了**：`Push::Stats{connections}` 与 `EventKind::Stats` 仍在协议里但**无人可发**（订阅它=永远等不到；下轮该删或给它一个真来源）；§12 的真做需要 (a) 一个能穿过的监听器、(b) 每连接归因，二者都要特权或上游改动，**不是本机可验**；`Stats`/`Egress` 两个变体保留是为了 §49 的方法名与将来的实现，但它们现在是"只有形状没有生产者"的面，这一点写在上面而不是藏在 diff 里；GUI 的 Check-IP 按钮现在收到的就是这句拒绝，**页面文案怎么呈现给用户没验过**（§30/§31 未定稿）。
MD
python3 - <<'PY'
p="docs/audits/00-index-and-open-decisions.md"
s=open(p,encoding="utf-8").read()
s=s.replace("**285 passed / 0 failed / 0 ignored**","**288 passed / 0 failed / 0 ignored**").replace("AA–AN（","AA–AO（")
s=s.replace("§52 拆分巨型文件）","§52 拆分巨型文件、§44/§45/§12 取消编造测量）")
open(p,"w",encoding="utf-8").write(s)
q="docs/audits/09-session-handover-2026-10-04.md"
h=open(q,encoding="utf-8").read()
h=h.replace("  cargo test --workspace          # 285 passed / 0 failed / 0 ignored","  cargo test --workspace          # 288 passed / 0 failed / 0 ignored")
h+="""
- **Gate AO（§44/§45/§12）**：钉住的上游核心**没有计数器**（`RunningProxy` 只有 is_running/stop/wait/request_stop）。`get_stats` 与 `check_egress` 现在报"为什么量不出来"而不是漂亮的零；全仓已**没有任何代码路径能构造** `Response::Stats`/`Response::Egress`。**遗留假面**：`Push::Stats` 与 `EventKind::Stats` 无人可发（订阅它永远等不到），下轮要么删要么接上真来源。
"""
open(q,"w",encoding="utf-8").write(h)
print("ok:", "288 passed" in open(p,encoding="utf-8").read(), "AO:", "AA–AO" in open(p,encoding="utf-8").read())
PY
rg -n "^

## Gate AP — §49/§52：删掉"订阅了就永远等不到"的面，并把过滤表变成双向完整性守卫（291 passed / clippy clean）


上一节留下的最明确一个口子：`Push::Stats{connections}` 与 `EventKind::Stats` 都还在协议里，但**没有任何代码能发出它**（核心没计数器，AO 之后连 `Response::Stats` 也没有生产者）。客户端 `subscribe(["stats"])` 会被 `Accepted`，然后**永远收不到东西**——这与本轮修掉的 `ScanFinished` 归错流是同一类谎言：接口答应了一件不会发生的事。

删法与理由：
- 从 `Push` 与 `EventKind` 里**删掉** Stats。`Response::Stats` **留着**——这个不对称是有原则的：一个方法今天答"量不出来，原因是 X"是诚实的（客户端主动问、当场得到理由）；一个永远不会到的推送是陷阱（客户端只能干等或靠超时猜）。§49 点名的 `get_stats` 作为**方法**依然存在。
- `EventKind` 加 `all()`，`kind_matches` 的表旁边写三条完整性守卫：
  1. **每个 EventKind 必须至少匹配一个 Push**（否则就是"接受了却永不触发"的订阅）；
  2. **每个 Push 必须被某个 EventKind 覆盖**（否则只有"什么都不要过滤"的连接看得见它，等于隐藏的广播面）；
  3. 一个具体的双向选择断言：`[log_line]` 恰好放行 1 条、`[scan_progress]` 放行 2 条（进度＋终止），且没有任何一条同时属于两个 kind。
- 守卫还钉了一句 `assert_eq!(pushes.len(), 4)`：**新增一个 Push 变体却没进表，第一轮就会红**。这类"数量当哨兵"的写法在本仓已用过（`codec_guards` 的 18 个方法名），防的是同一种失败：清单被复制而不是被穷举。

顺带把我这次改动自己制造的一个缺陷也说清楚：用 perl 批量改注释时把 `kind_matches` 的文档注释**改成了重复半句**（同一句话出现两次、语义断裂）。测试是绿的，缺陷只在可读性上——但它说明**批量文本替换后必须回读那一段**，不能只看编译结果。已修。

**没做也别说成做了**：`Response::Stats` 仍无生产者（故意的，见上），要接真数据得等上游出计数器或换 `TrafficBackend` 的实现；§12 的证据链本身还是空的（本轮只保证它不许用假数据冒充）；GUI 里没有 stats 面板，所以这条删除对用户可见行为无影响，但这意味着**"流量统计"这一页仍然没有实现**（§5/§49 的 get_stats 只到"接口存在且拒绝说谎"为止）。
MD
python3 - <<'PY'
import re
p="docs/audits/08-phase3-gates-evidence.md"
s=open(p,encoding="utf-8").read()
old="**没做也别说成做了**：`Push::Stats{connections}` 与 `EventKind::Stats` 仍在协议里但**无人可发**（订阅它=永远等不到；下轮该删或给它一个真来源）"
new="**没做也别说成做了**：~~`Push::Stats` 与 `EventKind::Stats` 仍在协议里但无人可发~~（已在下一节 Gate AP 删除，并补了双向完整性守卫）"
print("AO caveat corrected:", old in s)
s=s.replace(old,new,1)
open(p,"w",encoding="utf-8").write(s)
q="docs/audits/00-index-and-open-decisions.md"
t=open(q,encoding="utf-8").read()
t=t.replace("**288 passed / 0 failed / 0 ignored**","**291 passed / 0 failed / 0 ignored**").replace("AA–AO（","AA–AP（")
t=t.replace("§44/§45/§12 取消编造测量）","§44/§45/§12 取消编造测量、§49/§52 删除永不触发的订阅面）")
open(q,"w",encoding="utf-8").write(t)
r="docs/audits/09-session-handover-2026-10-04.md"
h=open(r,encoding="utf-8").read()
h=h.replace("  cargo test --workspace          # 288 passed / 0 failed / 0 ignored","  cargo test --workspace          # 291 passed / 0 failed / 0 ignored")
h=h.replace("**遗留假面**：`Push::Stats` 与 `EventKind::Stats` 无人可发（订阅它永远等不到），下轮要么删要么接上真来源。","`Push::Stats`／`EventKind::Stats` 已删除（Gate AP）：订阅它=永远等不到，属于接口答应了不会发生的事；`Response::Stats` 保留但**无生产者**，是有意不对称（方法当场给理由诚实，永不触发的推送是陷阱）。")
open(r,"w",encoding="utf-8").write(h)
print("index:", "291 passed" in t)
PY
rg -n "^

## Gate AQ — §6 Xray 链接导入：用上游的解析器，凭据不外流（296 passed / clippy clean，`Cargo.lock` 未变）


先查上游再说做不做：钉住 commit 的 checkout 里已经有 `sni_spoof_rs::xray::parse_share_link`（VLESS/VMess/Trojan → `XrayShare{protocol,name,user_id,host,port,sni,network,…}`）。§1 铁律是"不要为了 GUI 重写成熟核心"，所以**链接解析一行都不自己写**；`himari-runtime` 只做一行 re-export，让 daemon 不必直接点 git 依赖（用哪个 commit 的解析器只有一个地方决定）。**没有新增依赖**，`Cargo.lock` 一字未动——这条对 §42 的许可台账很重要。

**凭据的走法**是被这节真正约束的东西。`XrayShare.user_id` 就是密钥，而本轮的日志脱敏器只认 `authorization:`/`cookie:`/`token=` 这类**形状**，裸 UUID 穿得过去。所以不能靠脱敏，要靠**类型**：
- IPC 上只有 `ShareInfo{label, protocol, host, port, endpoint_is_local}`，**没有** user_id 字段；
- 日志只写 `share added: vless 198.51.100.20:443`（协议+主机+端口，外加"local endpoint"标记，§11 关心的是这个）；
- 诊断行 `xray shares` 说"N 条已解析入库，第一条是 vless …；本构建不启动 Xray 进程"——`Warn` 不是 `Ok`，因为存下链接≠有隧道；
- 原链接只落在 `xray-shares.json`，写入时显式 `chmod 0600`（守卫直接读文件模式断言）。

5 条守卫（`xray_shares.rs`）：
1. 导入回来的投影里、日志里、诊断行序列化文本里**都搜不到那个 UUID**（`all_text()` 把三处拼一起断言，比分开断言难绕）。
2. 坏链接按**上游的理由**拒（不自己编理由），且内存与磁盘都什么都没留。
3. 累积、去重、重启回读、0600：同一链接导两次仍是 2 条（凭据不在盘上翻倍）；rebind 后日志要宣布 `imported share …`——"缓存空了"和"缓存被拒了"不能长一样。
4. **§6 与 §45 的接缝**：把配置写、链接导入、策略记录、诊断取数全跑一遍之后，`cleanup::foreign(dir)` 必须为**空**。这个断言是"新增运行时文件却忘了登记到卸载表"的唯一自动化拦截，而且它复用了上一节已经写好的 `foreign()`。随后 uninstall 必须把 `xray-shares.json` 一起删掉（凭据文件留在盘上是最坏的一种"卸载成功"）。
5. `Start{xray}` 拒绝的理由要能当场核对（里面带的条数就是它声称的条数），并且**不许把状态 latch 成 starting**：`get_status` 里 xray 必须仍是 `stopped`。

两条自家的漂移守卫在这轮**各自红过一次**，正好证明它们活着：新增 `xray shares` 行让 `diagnostics_rows` 的 `assert_eq!(answer.len(), AREAS.len())` 抓住"清单少一项"；而 `himari-ui` 的跨仓守卫要求新区域必须有 `measured_area` 映射（我先补映射再跑，才没红第二次）。方法名清单也从 18 升到 20，同样的计数哨兵。

**顺手修掉一个只有编译器能告我的错**：`Request::Start { target } if … xray` 这条守卫我一开始加在**通配 Start 分支之后**，于是永远走不到——Rust 的 match 顺序语义在这里静默生效（编译通过、测试变红才暴露）。已挪到通用分支之前。

**没做也别说成做了**：§6 的其余部分——进程管理（下载/校验/启动 xray 二进制、conf 生成、all-in-one、日志）——**一件都没做**，`xray` 组件仍是"只有状态与拒绝"；导入的链接因此**还没有消费者**（存下来、能列出来、不能跑起来，这一点在诊断行里明说）；`AddXrayShare` 无鉴权差异（任何能连上 socket 的本用户进程都能加）；没有"删除某条链接"的方法（只能整体覆盖式导入去重）；`ShareInfo.port` 用的是上游的 `upstream_port()`（local endpoint 会回落到 443/80），不是用户字面填的端口，展示时要按这个口径理解。
MD
python3 - <<'PY'
p="docs/audits/00-index-and-open-decisions.md"
s=open(p,encoding="utf-8").read()
s=s.replace("**291 passed / 0 failed / 0 ignored**","**296 passed / 0 failed / 0 ignored**").replace("AA–AP（","AA–AQ（")
s=s.replace("§49/§52 删除永不触发的订阅面）","§49/§52 删除永不触发的订阅面、§6 Xray 链接导入（凭据不外流））")
open(p,"w",encoding="utf-8").write(s)
q="docs/audits/09-session-handover-2026-10-04.md"
h=open(q,encoding="utf-8").read()
h=h.replace("  cargo test --workspace          # 291 passed / 0 failed / 0 ignored","  cargo test --workspace          # 296 passed / 0 failed / 0 ignored")
h+="""
- **Gate AQ（§6 部分）**：Xray **链接导入**已可用（上游 `xray::parse_share_link`，`Cargo.lock` 未变）。凭据不外流的机制是**类型**：IPC 只有 `ShareInfo`（无 user_id）、日志只有协议+主机+端口、盘上 `xray-shares.json` 固定 0600，且 §45 的 `foreign()` 完整性守卫会拦"新增运行时文件却没登记到卸载表"。**§6 的进程管理仍未实现**，所以导入的链接目前**没有消费者**——别当成隧道可用。
"""
open(q,"w",encoding="utf-8").write(h)
print("index:", "296 passed" in s)
PY
cd ../himari-app && git status --porcelain=v1 Cargo.lock && echo "(Cargo.lock untouched)" ; rg -n "^

## Gate AR — §6 Xray 二进制的版本探测：数字必须是从那个文件跑出来的（303 passed / clippy clean）


`get_xray_status` 之前只有 `status_of(Component::Xray)`——也就是"状态机上写着 stopped"这一件事，页面既不知道机器上有没有 Xray、也不知道是哪一版。§6 的"binary management"至少要先能做到**如实报出被指着的那个文件说自己是什么**。

做法：`crates/himari-runtime/src/xray_bin.rs` 里 `XrayBinary::version()`——先 `is_file()`（不存在就**根本执行**，报 `Missing`），`Command::new(path).arg("version")`，stdout 空才回退读 stderr，取第一非空行，>200 字符截断并留 `…[truncated]`，**没有数字就判 `Unparsable`**（带着截断后的原文片段）。`Options::xray_binary`（`HIMARI_XRAY_BIN`）注入路径；诊断新增 `xray binary` 行：探到 → `Ok(版本)`，没配 → `Unknown` + remedy，配了但答不上来 → `Fail` + 另一条 remedy（"打包问题"与"权限/架构问题"是不同的做法，不许并成一个颜色）。`Start{xray}` 的拒绝理由现在会带它当场探到的版本或探不到的原因。

9 条新守卫（`himari-runtime/tests/xray_binary.rs` 5 条 + `xray_shares.rs` 4 条），两条是**执行层证据**：
- 假二进制的 banner 只存在于脚本里；`get_xray_status` 必须回出 `xray_version` 含 `24.99.7` 和 `deadbee`，同时 `xray` 状态**仍是 stopped**（"知道版本"≠"进程在跑"）。诊断行同数——即数字确实是从文件穿过探测进到答案，不是抄来的。
- 一个不是 Xray 的文件必须让行变 `Fail` 而不是留空（空版本在屏幕上就是"没有版本"的假象）。
- 其余：缺失不执行且理由点名路径；silent / usage / 无数字的 stderr 三种各自都判 `Unparsable`；`xray version` 写到 stderr 但有版本号 → **接受**（这条单独钉，因为它最容易被"顺手修严"成一个假阴性）；超长 banner 被截且声明被截；不可执行文件（0644）判 `Missing`＋"could not be executed"，与"没配"区分开。

**我自己这轮的三个错，都是被机器抓住的**（这才是过程该有的样子）：
1. 把诊断行脚本插到 `lib.rs`——但 `diagnostics()` 上一轮已搬进 `diagnostics.rs`，于是那次 `replace` 是**无声的空操作**。脚本化编辑**必须 assert 命中**（后半段我加了 assert，第一次没加所以漏了）；是 `diagnostics_rows` 的清单长度守卫把它逼出来的。
2. 替换掉 `status_of` 唯一调用者之后它变死代码，clippy 报 `never used` → 删掉，不留"以后可能用"。
3. 我写的守卫预期本身是错的（以为 stderr 回退该判拒），实现的行为是对的。**测试写错和实现写错在同一处会互相掩盖**——分开把两种行为各自钉住才对，所以现在是两条测试而不是一条 if 分支。
另外跨仓漂移守卫第二次尽到职责：`["xray binary"] have no Area on the diagnostics page`。

**没做也别说成做了**：§6 的**进程管理**（下载/校验/启动、conf 生成、all-in-one、日志收集）仍然一件没做，因此导入的 share 链接**依旧没有消费者**；探测是**按需**跑的（每次 `get_xray_status` 起一个子进程），没有缓存也没有超时控制——Xray 卡住会让这问变慢，这条要接真进程管理时一起处理；`xray_version` 塞在 `Response::Status.components` 里（与 `sni_gate_version` 同法），是一种字符串键旁路，正式形状应当是独立响应字段；没有"下载/更新二进制"的入口（§6 的 binary management 只到"看得见"）。
MD
python3 - <<'PY'
p="docs/audits/00-index-and-open-decisions.md"
s=open(p,encoding="utf-8").read()
s=s.replace("**296 passed / 0 failed / 0 ignored**","**303 passed / 0 failed / 0 ignored**").replace("AA–AQ（","AA–AR（")
s=s.replace("§6 Xray 链接导入（凭据不外流））","§6 Xray 链接导入（凭据不外流）、§6 Xray 二进制版本探测）")
open(p,"w",encoding="utf-8").write(s)
q="docs/audits/09-session-handover-2026-10-04.md"
h=open(q,encoding="utf-8").read()
h=h.replace("  cargo test --workspace          # 296 passed / 0 failed / 0 ignored","  cargo test --workspace          # 303 passed / 0 failed / 0 ignored")
h+="""
- **过程纪律（Gate AR 学到、值得接手的人遵守）**：脚本化编辑**每次都要 assert 命中**——我有一次把代码插到了旧位置（`diagnostics()` 已经搬到 `diagnostics.rs`），`replace` 静默什么都没做，是清单长度守卫逼出来的。第二教训：替换掉唯一调用者之后要顺手删死代码（clippy 会报 `never used`）。第三：守卫自己的预期也可能是错的，而**它和实现对齐在同一个错上会互相掩盖**——把两种行为分开各钉一条，别写成一个 if。
"""
open(q,"w",encoding="utf-8").write(h)
print("303:", "303 passed" in s, "AR:", "AA–AR" in s)
PY
rg -n "^

## Gate AS — §6 探测必须有界：卡住的二进制不许拖住状态查询（305 passed / clippy clean）


上一节自己写下"没有超时控制"。这一条把它补掉，因为后果不对：状态查询走的是**服务进程的连接线程**，一个 `xray version` 卡死（磁盘上坏掉的 fuse 挂载、被替换成脚本、SELinux 类拦截）就会让那一条连接从此不回话——而 §45 的失败定义正是"UI 显示运行中但进程其实不对"。

实现全用 std，不引依赖：`spawn` 之后**轮询 `try_wait`**（10 ms 步进）到期限；到期就 `kill` + `wait`（**先杀再收尸**，否则留一个僵尸进程在用户机器上，比慢更糟）；只有确认退出之后才去读管道（各限 4096 字节，版本 banner 是一两行）。错误面新增第三态 `TimedOut{path, waited}`，理由与"没配"（打包问题）和"配了但答不上来"（不是 Xray）**三件事三种说法**，因为修法各不相同；诊断行落在 `Fail`，remedy 不复用。默认期限 `DEFAULT_TIMEOUT = 5s`，并暴露 `with_timeout` 让守卫能用 400 ms 真的验到超时而不是把测试拖成 5 秒。

2 条新守卫，关键是第二条**可证伪**：
- 假二进制 `echo $$ > pid; sleep 30`：400 ms 期限内必须回 `TimedOut`，且整个调用 < 5s（断的是"没把 30 秒等完"，不是"最后也会回来"）。
- 然后**读脚本自己公布的 pid**，用 `kill -0` 轮询证明它已经不在了——"探测不留下孤儿"这句话只有这么量才算证据；否则一个不 kill 的实现同样能过上一条。整轮 runtime 测试 0.41s 跑完，就是它确实被杀的旁证。
- 反向对照：正常 banner 在默认期限下不受影响（数字完整、`started.elapsed() < 2s`），并断言 `DEFAULT_TIMEOUT == 5s`，免得有人为了测试把默认值调小。

**没做也别说成做了**：daemon 侧走的是默认期限（`Options` 里没有可注入的 timeout），所以服务层的超时行为只有 runtime 层这一层证据；仍**没有缓存**（每次状态查询都会起一个子进程问版本，进程管理接上时应改成启动时问一次并留快照）；读管道发生在退出**之后**，理论上对一个输出海量且不退出、只靠超时被杀的 child 不会有死锁（它已被 kill），但那条路径没有被显式守卫；§6 其余部分——下载/校验/启动/conf 生成/all-in-one/日志收集——仍然一件没做，导入的 share 链接**依然没有消费者**。
"""
open(p,"w",encoding="utf-8").write(s)
q="docs/audits/00-index-and-open-decisions.md"
t=open(q,encoding="utf-8").read()
t=t.replace("**303 passed / 0 failed / 0 ignored**","**305 passed / 0 failed / 0 ignored**").replace("AA–AR（","AA–AS（")
t=t.replace("§6 Xray 二进制版本探测）","§6 Xray 二进制版本探测（含超时与不留孤儿））")
open(q,"w",encoding="utf-8").write(t)
r="docs/audits/09-session-handover-2026-10-04.md"
h=open(r,encoding="utf-8").read()
h=h.replace("  cargo test --workspace          # 303 passed / 0 failed / 0 ignored","  cargo test --workspace          # 305 passed / 0 failed / 0 ignored")
h+="""
- **Gate AS**：`XrayBinary::version()` 现在有期限（默认 5s，`with_timeout` 可注入），到期 `kill`+`wait`；守卫用脚本自报 pid + `kill -0` 证明**不留孤儿**——"有超时"这句话不杀进程也能过，所以必须这样量。仍未缓存版本（每次状态查询起一个子进程）。
"""
open(r,"w",encoding="utf-8").write(h)
print("index 305:", "305 passed" in t, "| AS:", "AA–AS" in t)
PY
rg -n "^

## Gate AT — §6 探测缓存：带身份失效，而不是"记住第一次的答案"（308 passed / clippy clean）


上一节留了一句"没有缓存，每次状态查询都起一个子进程"。直接按字面做缓存会**推翻 Gate AR 自己立的性质**——"记住的版本"正是那个文件被换掉之后还在报旧号的东西。所以缓存的键不是路径，而是 `(path, mtime, size)`：身份变了就一定重问。失败答案同样进缓存（一个不是 Xray 的文件不会在两次查询之间自己变成 Xray，除非它的身份变了）。

`Options::xray_timeout` 同时加上，让**服务层**的超时行为可被守卫驱动（否则只有 runtime 层那一层证据，而这正是我上一节写下的"没做"）。

3 条守卫，每一条都配了"只做对一半也能过"的反面：
- `runs.log` 由假二进制自己追加：两次查询 → **只跑一次**（证明缓存生效）。
- 第三条断言才是关键：**把那个二进制文件本身换掉**（内容不同、字节数也不同）→ 必须报出新版本、并且 `runs.log` 涨到 2。少了这条，前一条用"永远记住第一个答案"的坏实现照样能过。
  ——顺带说清楚一次我自己写错的过程：我第一版这条测试是改**脚本去读的另一个文件**（banner），结果它红了。红得对：服务能观察到的身份只有那个文件本身，改它调用的东西不是"换了二进制"。**测试前提错、守卫把它逼出来**，这比守卫放过它更有价值。
- 失败答案的缓存：`runs.log` 两次查询仍是 1，且理由文本一致（不许每次查询换一个说法）。
- 服务层超时：`xray_timeout = 400ms` + `sleep 30` 的假二进制 → `get_xray_status` 里 `xray_binary` 必须是"did not answer within 400 ms"、整体耗时 < 4s，**且脚本自报的 pid 之后被 `kill -0` 判为已消失**；再查一次用的是缓存里的同一份超时答案（不重新起一个 30 秒的子进程）。

**没做也别说成做了**：身份是 `mtime+size`，理论上"同秒、同字节数"的替换会漏（真换 Xray 二进制几乎不可能同字节数，但这仍是**已知上限**，不是完备证明）；缓存没有失效时钟（文件被 touch 但内容不变会白问一次，代价可接受）；§6 的下载/校验/**启动**/conf 生成/all-in-one/日志收集**依旧一件没做**，导入的 share 链接仍然只是被存着、没人消费；`xray_version` 还是走 `Response::Status.components` 的字符串键旁路。
"""
open(p,"w",encoding="utf-8").write(s)
q="docs/audits/00-index-and-open-decisions.md"
t=open(q,encoding="utf-8").read()
t=t.replace("**305 passed / 0 failed / 0 ignored**","**308 passed / 0 failed / 0 ignored**").replace("AA–AS（","AA–AT（")
t=t.replace("§6 Xray 二进制版本探测（含超时与不留孤儿））","§6 Xray 二进制版本探测（含超时与不留孤儿）、§6 带身份失效的探测缓存）")
open(q,"w",encoding="utf-8").write(t)
r="docs/audits/09-session-handover-2026-10-04.md"
h=open(r,encoding="utf-8").read()
h=h.replace("  cargo test --workspace          # 305 passed / 0 failed / 0 ignored","  cargo test --workspace          # 308 passed / 0 failed / 0 ignored")
h+="""
- **Gate AT**：Xray 探测缓存的键是 `(path, mtime, size)`，**失败也缓存**；`Options::xray_timeout` 注入后服务层超时才可测。写缓存前先问一句"缓存会不会推翻我刚才立的性质"——这里差点把"版本必须来自那个文件"缓存成"记住第一次的答案"。上限：同秒同字节数的替换会漏。
"""
open(r,"w",encoding="utf-8").write(h)
print("308:", "308 passed" in t, "AT:", "AA–AT" in t)
PY
rg -n "^

## Gate AU — §49 状态方法各归其形：`components` 表里不许混进非组件的键（312 passed / clippy clean）


`get_sni_gate_status` 与 `get_xray_status` 都借用 `Response::Status`，把 `sni_gate_version` / `xray_version` 当"组件"塞进 `components` 表。后果不是难看：那张表的语义是**组件→状态**，主页遍历它、并按 `Component::parse(key)` 决定一个键是什么东西——一个 parse 不出来的键要么被丢掉（版本永远显示不出来），要么被当成组件渲染（页面出现一个叫 `xray_version` 的"组件"，值还不属于那六个状态词）。两种都不是测量。

改法：两个方法各用自己的响应形状，`Status` **只**留给 `get_status`。
- `Response::GateStatus { state, generation, version: Option, error: Option }`
- `Response::XrayStatus { state, generation, version: Option, error: Option, shares, counted }`
"探不到版本"与"版本是空字符串"从此是两件事（`Option`），而 `shares` 直接进答案，页面不需要第二次往返。

4 条守卫（`status_shapes.rs`）+ 3 处迁移：
1. `get_status` 的表**长度必须等于 `Component::ALL.len()`，每个键都要能被 `Component::parse` 认出来，每个值都必须落在那六个状态词里**——这条同时挡住"以后再加个旁路键"和"往值里塞自由文本"。
2. **反向对照**：`Component::parse("sni_gate_version")`／`("xray_version")`／`("xray_binary")` 必须为 `None`，而真键必须为 `Some`。没有这条，第 1 条可能在保护一个根本不存在的问题（也可能是把所有键都判成非组件后一路放行）。
3. 两个状态方法**再也回不到** `Response::Status`（`matches!` 各钉一次）。
4. 没配二进制时：`version` 必须是 `None`、`error` 必须点名 `HIMARI_XRAY_BIN`、`counted == 0` 且 `shares` 为空——" absence 要解释，不许留空白"。

迁移的三处断言各自变强了：网关停止之后现在会断 `version.is_none()`（**已停的网关不许还挂着一个活版本号**，这是旧形状表达不了的性质）；`gateway-binary-missing` 从"表里某行的值"变成 `error` 字段，判的是 `Some(那个阶段名)`。

**没做也别说成做了**：`counted` 与 `shares.len()` 是**同一个数的两份**（为了断言"计数必须与它数的东西一致"而留，真删掉更好，但那要先有分页）；`Response::Status` 仍是 `Vec<(String,String)>` 而不是 map（键唯一性靠上面的长度断言）；§49 点名的 `get_stats` 仍只有"报理由"这一条路（核心无计数器）；UI 侧这次只加了 `summarize` 的两条分支，**没有把版本显示到界面上**（那属于 §30/§31 的稿子）。
MD
python3 - <<'PY'
p="docs/audits/00-index-and-open-decisions.md"
s=open(p,encoding="utf-8").read()
s=s.replace("**308 passed / 0 failed / 0 ignored**","**312 passed / 0 failed / 0 ignored**").replace("AA–AT（","AA–AU（")
s=s.replace("§6 带身份失效的探测缓存）","§6 带身份失效的探测缓存、§49 状态方法的正式形状）")
open(p,"w",encoding="utf-8").write(s)
q="docs/audits/09-session-handover-2026-10-04.md"
h=open(q,encoding="utf-8").read()
h=h.replace("  cargo test --workspace          # 308 passed / 0 failed / 0 ignored","  cargo test --workspace          # 312 passed / 0 failed / 0 ignored")
h+="""
- **Gate AU**：`get_sni_gate_status`／`get_xray_status` 不再借用 `Response::Status`；`Status.components` 由守卫保证**只含组件、只含六个状态词**。写这类表时先问"这键能被 parse 回来吗"。
"""
open(q,"w",encoding="utf-8").write(h)
print("312:", "312 passed" in s, "AU:", "AA–AU" in s)
PY
rg -n "^

## Gate AV — §43 自建语料：候选只能来自这台机器量过的东西（318 passed / clippy clean）


需求书 §43 反复强调"不许把参考项目里的 payload / 候选清单抄过来"，而本仓此前对它的唯一回应是一句拒绝文案
（"Himari keeps its own corpus and will not invent one"）。那句话当时是**空头承诺**：没有任何东西写过 corpus，
也没有任何东西读过它。本轮把它变成一条有来源、有落盘、有消费者的机制。

新增 `crates/himari-daemon/src/corpus.rs`（`Entry{tld}` 之外的字段见下）。来源只有两个：本服务跑过的探测，
和用户自己敲进去的名字；**文件里没有任何 seed list**，`MAX_SCAN_BATCH = 64` 给它加了上界（语料的意义是记住
量过什么，不是攒够一天把见过的每个名字都去问一遍——那违背 §7 的有界并发精神）。

接线（全部走同一条码路，不留旁路）：
1. **写**：`jobs.rs` 的扫描回调在 `hub.broadcast(ScanProgress)` **之前**记账。判据不是"顺手记一下"，而是
   "客户端看到的那一行必须能在语料里被重新推导出来"；写盘失败只丢持久性、不丢本轮测量，且**说进日志**。
2. **读**：`Request::Scan` 的 `snis` 为空 = "扫你已经知道的那些"，由 `Corpus::names()` 按最近观测优先给出；
   两边都空时才拒绝，拒绝理由点名 §43。
3. **披露**：`Request::GetCorpus` → `Response::Corpus{entries, count}`，`entries.len() == count` 被断言。
   落盘格式**就是线上格式**（`Vec<CorpusEntry>`）：`himari-daemon` 没有 `serde` 依赖，先前想给内部 `Entry`
   加 `#[derive(Serialize)]` 直接编不过——而"文件与 UI 读同一条契约"恰好是更强的性质，旧构建写下的文件
   要由同一份合同来读。
4. **卸载**：`corpus.json` 进 `RUNTIME_ARTIFACTS` 与用途表两处（Gate AH/§40 那条 `foreign(dir) == 空` 的守卫
   会自动盯着第二处——漏了它，卸载后目录里会剩一个"别人家的文件"）。

6 条守卫（`crates/himari-daemon/tests/corpus_guards.rs`，全部走真 IPC）：
1. `a_candidate_enters_the_corpus_only_because_this_service_probed_it`：先断**新鲜目录语料为 0**（否则"后来有 1 条"
   可以读成预置清单），再探一个名字，断 `times_probed == 1`、`latency_ms` 是 `Some`（探测一定量到了时间，
   `None` 意味着数字在 scanner→store 之间掉了）、`last_tested_ms > 0`，**并把磁盘上的 `corpus.json` 反序列化回来
   与内存答案逐字节比**（磁盘不许是脱敏/审计的旁路）。
2. `an_empty_candidate_list_probes_the_corpus_and_refuses_when_there_is_none`：**先判拒绝再判放行**，同一条请求形状
   两个分支——只跑放行分支的守卫等于一把没眼睛的尺（[[feedback-gauges-need-positive-controls]]）。
3. 重复探测：`count` 仍为 1、`times_probed == 2`（同名不许追加第二条，也不许第二次观测不被计数）。
4. `the_corpus_is_read_back_from_disk_by_the_next_daemon`：第二个进程**自己一次都没探**，所以它报出来的
   `count == 1 && times_probed == 1` 只能来自文件；随后空列表扫描必须排到 1 条。
5. 坏文件：`bind` 不许崩、语料必须为 0、且 `GetLogs{core}` 里必须有一行点名 `corpus.json ... rejected`
   （"静默空"与"干净空"是两件事，§29）。
6. `the_corpus_module_contains_no_inherited_candidate`：文本门禁只禁**代码形状**（host 字面量、字面量数组），
   并先用一个手种的 `["www.cloudflare.com", "example.org"]` 证明这把尺看得见（命中 3 处：两个 host + 一个列表），
   再断 `"corpus.json"` / `"json.tmp"` 这类合法值不触发；行为层那条（新鲜机器无可扫）由第 2 条负责，
   文本扫描证明不了它，注释里写明了本门禁只管这一个模块。

**一次"老守卫变红"是发现，不是回归**：接完语料跑全量，§42 的 `every_transitive_dependency_carries_an_allowlisted_license`
红了，报 `NOT_ON_THIS_PLATFORM` 里 `async-trait 0.1.92`、`uuid 1.27.0` 两条**豁免已过期**——bridge 那条 lane 把这两个
crate 拉进了 macOS 依赖图。本轮去读了两份下载下来的 manifest（各 `MIT OR Apache-2.0`），把它们从豁免表删除、
改由主循环按普通包核验，并在表上方写下"这条断言为什么红的"。这正是该断言存在的理由：豁免表不许悄悄变成猜测的藏身处。

**按 D-10 的冻结口径退回去的部分**：本轮一度给 Iced 那条 lane 加了 `Intent::ShowCorpus`、Scanner 页两个按钮和
bridge 路由；项目记忆里的定稿是"前端唯一 = Flutter，Iced GUI 停写、代码冻结、该 lane 转后端"，于是**全部撤回**，
只保留两处被迫的编译性修改：`client.rs::summarize` 的 `Response::Corpus` 分支（match 必须穷尽），以及
`RunScan(CurrentFake) → None` 的更正——§43 之后"空列表"有了新的含义，沿用旧映射会"用户按当前 fake SNI，
程序去扫语料"，属于静默读错；`App::reduce` 在送到 bridge 之前就把这个 intent 解析成 `Hosts([fake_sni])`，
所以没有活路径受影响。`GetCorpus` 的消费者目前是 `corpus_guards` 里那条真 socket 路径；界面归 Flutter 那条 lane。

**没做也别说成做了**：语料**没有进诊断页**（那需要新 area，而 Iced 侧已冻结；§43 的溯源目前由 `GetCorpus` 承载，
等 Flutter 侧渲染）；`Builtin` 扫描在 Iced 侧不可达（冻结），空列表语义只有守卫在用；语料条目不记"这个名字是谁
写进来的"（探测 vs 用户手输），所以现在只能说"这台机器量过"，不能说"谁让它来的"；`*.json.tmp` 残留仍会被
`foreign()` 判成外来文件（三个写入点都有这个问题，是既有缺口，本轮没扩范围）；§43 的 payload/参考数据那半边
（上游 sni-gate 自带的数据文件）仍未逐文件清点。
MD
echo "APPENDED"; tail -c 260 08-phase3-gates-evidence.md

## Gate AW` at line 11. So the whole prior content (1030+ lines, Gates A..AV) is GONE. Was it gone before my append (i.e. my read returned ~2300 chars? no — `len(s+section)`=2849 means s≈150 chars) — so at the moment of my read the file was already nearly empty (only a stub). Someone (a concurrent session, or a GitButler operation) truncated it between my 12:55 write and now.


Recovery options:
- `docs/` is untracked (`?? docs/`) → no git blob to restore from... unless GitButler's uncommitted bucket / its internal DB has it. Also `git status` shows `?? docs/` meaning untracked: no history.
- Wait — earlier in the session `git status` showed `?? docs/` too, so it was never committed. Damn.
- Check for backups: Time Machine? Not for quick restore. Check Qoder's file history? Maybe `.local/` or editor swap. Let me check:
  - `/Users/glow/Base/Code/Prometheus/Himari/.git/` — untracked, no blob.
  - GitButler might keep a "uncommitted" snapshot in `~/.local/share/gitbutler/...` — it does store the uncommitted state as a commit in its own DB! `git butler` CLI: `but` available. GitButler's uncommitted bucket is a virtual branch commit → recoverable via `git cat-file`/reflog in `~/.git/but...`.
  - Or check the 09 handover file (70 lines) — probably also lost content? It says 70 lines, maybe fine.
  - Also I have the file content partially in my context (the summary/restored context), but not all 1030 lines.

Priority: try GitButler recovery. Let me look for butler storage: `ls ~/.local/share/gitbutler` and the project's `.git/gitbutler`. Also `git fsck --lost-found`? Since docs/ was never tracked or staged, GitButler *did* include it in the uncommitted bucket (it snapshots working tree). Actually GitButler's uncommitted bucket only tracks files it knows about; untracked files are shown in the bucket as "uncommitted" and are stored as a commit when you commit... Hmm, but `but` creates commits only on demand.

Another angle: maybe another session *rewrote* the file with a small content (e.g. they replaced 08 with a stub and moved gates to a new file 10/11?). Let me check for other docs files elsewhere (e.g. himari-app/docs, or a new audits file) — maybe the gates moved and my doc edits got clobbered. Look at `.watchdog`/`UI_LANE.md`? Let me check:
1. `ls -la docs/audits` (mtimes) + any new files (e.g. 10-*).
2. Search the whole Prometheus tree for "Gate AU" text — maybe the content lives in another file now (a sibling consolidated it).

Let me check both, plus gitbutler storage.

## Gate AZ

