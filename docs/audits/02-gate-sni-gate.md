# §55.B Gate 审计 — `racpast/sni-gate` v2.5.1

审计对象就是当前工作树（`origin=HibernalGlow/Himari`，`upstream=racpast/sni-gate`，HEAD `e6e807e` == upstream HEAD，**尚未分叉**）。
`Cargo.toml` → `name="sni-gate"` `version="2.5.1"` `rust-version="1.88"`，license **MIT OR Apache-2.0**。
**只有 `[[bin]]`，`src/` 下没有 `lib.rs`**（已用 `find` 复核）→ 想「独立 Rust crate 集成」（需求书 §1）当前不成立，只能按**独立进程**集成。

## 1. CLI 与进程模型（§55.B 点名「CLI / process model」）

`src/main.rs:74-90` clap derive，**无子命令**：

| 参数 | 行为 | 证据 |
|---|---|---|
| `-c, --config <PATH>` | 默认 `"sni-gate.toml"`，**相对进程 CWD**；无搜索路径、无 env 覆盖 | `main.rs:82` |
| `--install-ca` | 载入/生成 CA → 装进系统信任根 → **退出** | `main.rs:154`、`:111-119` |
| `-V, --version` | 有，输出 `sni-gate 2.5.1`（`env!("CARGO_PKG_VERSION")`） | `main.rs:77,:180` |

进程事实（controller 必须按这些写）：

- **不 daemonize**（无 fork/setsid/re-exec），**无 pidfile / 无 control socket / 无 lock file**。
- 启动序：装 aws-lc-rs provider → `Cli::parse` → `Config::load` → `init_tracing` → 可选 `--install-ca` → 建多线程 tokio → `block_on(run)`。
- **配置错误走 stderr，且发生在 `init_tracing` 之前**（`main.rs:105,:109`）→ `SNI_GATE_LOG`/`RUST_LOG` 都管不了它；其余全部走 **stdout**。
- 退出码只有 **0 和 1**，配置错误/绑定失败/运行时致命错误**不可区分**。
- `panic = "abort"`（`Cargo.toml:74`）→ panic 以信号死亡而非 101，**崩溃检测必须读 wait status，不能只看 code**。
- 信号：Unix 装 SIGTERM+SIGINT（`main.rs:1294-1298`），非 Unix 只有 Ctrl+C。**SIGHUP 只重载 PSL**（`psl.rs:306-315`），**不重载主配置**；`notify` 只盯 PSL 文件。→ **全项目没有任何配置热加载**，reload = kill + 改 TOML + 重起（会断在飞连接）。
- ⚠️ **实测更正（2026-10-03，真二进制）**：SIGTERM 的结局**取决于发送时机**。
  ```
  settle=0s -> rc=143  (被信号 15 杀死)
  settle=1s -> rc=0    (干净退出)
  settle=2s -> rc=0
  ```
  即 sni-gate 的 TERM handler 要等启动流程跑到 `run()` 里才装上。**graceful stop 必须先过就绪门**，否则控制器会把用户的一次「停止」记录成一次崩溃并触发重启——正是 §36 要避免的误判。该行为已钉在 `himari-app/crates/himari-gate/tests/sni_gate_accepts.rs` 的 settle 循环里。
- **关闭不优雅**：`main.rs:267-277` 收到信号后直接 `return Ok(())`，accept loop 无 shutdown hook，每连接是裸 `tokio::spawn`（`proxy.rs:289-293`）→ 在飞连接被**切断而非 drain**（就绪之后退出码为 0）。

## 2. Config schema（§55.B「config」/ §14「config validation」）

`deny_unknown_fields` 覆盖**每一个 section**（`config.rs:37,100,141,248,342,385,488,805,937,1013,1245,...`）→ controller **多写一个键就是硬失败**；好处是拼写错误不会被静默吞掉。

顶层（`config.rs:36-96`）：`[global]`、**`[ca]` 必填**、`[store]`、`[cache]`、`[psl]`、`[templates.<n>]`、`[pools.<n>]`、`[resolvers.<n>]`、`[regexes.<n>]`、**`[[listener]]` 必填且非空**（注意键是**单数** `listener`，路由是 `[[listener.route]]`）。

继承阶梯：`route → route 的 template → listener → listener 的 template → global`（`config.rs:7-19`，实现 `Config::effective` `:2804-2920`），`[ech]/[http2]/[verify]` 沿同一阶梯**逐字段**继承。

controller 最常生成的部分：

- `Listener.addr` **必填**，接受 `"0.0.0.0:443"` / `"[::]:443"` / `"443"` / `443`；裸端口展开成 **IPv4 wildcard**（对齐 nginx `listen 443`）；**不接受主机名**；**端口 0 合法**（`:3656-3662`）。
- `Route`：`type = ech|tls|http|raw`、`match_sni`（精确 / `*.x` 单标签 / `.x` 后缀 / `@named-regex`；内联 `~re` 是**加载错误**）、`upstream`、`override_sni`（缺省=Reflect、`"name"`=Fixed、`""`=**不发 SNI 扩展**）、`cert_file/key_file`（both-or-neither，`raw` 上禁止）、`select`（仅 pool）、`fail`。
- `FailPolicy`：`close`（默认）| `system-outbound`（按 SNI/Host 透明出网）| `{mode="passthrough", addr=...}`（无 addr 直接反序列化失败）。
- `[resolvers.<n>]`：`endpoint` 必填且必须是**内联**规格（不能 `@ref`）；校验含**图无环检查** `resolver_build_order`（`:2583-...`，会报出环路径）。→ §13 要的 `DnsBackend` **上游已有实体**：DoH / DoT / 明文（`Cargo.toml` 用 hickory-resolver，`https-aws-lc-rs` + `tls-aws-lc-rs`）。
- `[pools.<n>]`：`targets` 非空（`/`→CIDR、IP→字面量、其余→域名），**CIDR 展开超 64 且没写 `[N]` 是加载错误**（`MAX_CIDR_EXPANSION`，`pool.rs:84,:305-319`，报错文案直接告诉你怎么改）；`probe` 必填；`ProbeDef::reduce`（`:1563-1689`）把「该 mode 用不上的字段」变成**指名道姓的错误**而不是忽略 —— 这是 §29 Diagnostics 想要的品质。
- `[verify]`：`mode = full|chain|none`、`pins`（`sha256/<base64>`，**整组替换不合并**）、`ca_file`、`trust_webpki`，并有一致性规则（如 `mode=none` 时写 `ca_file` 报错）。

⚠️ **§14「config validation」的现实障碍**：验证只能靠真加载。`[[listener]]` 必填非空 ⇒ 任何 load 测试都会**建 CA、刷 PSL、建 resolvers/pools 并真的 bind 端口**。可行绕法：生成一次性 config，`addr = 0`（合法）+ 一条不可达 `type="raw"` 路由 ⇒ 退出 0 = schema 合法、退出 1 = 非法（信息在 stderr）。副作用必须处理：CA 目录仍会被创建/写入；`[psl].auto_update` 与非 `off` 的 `http2.probe` **仍会发网络请求**；而且**临时端口从不回报**（`proxy.rs:277` 打的是配置里写的地址）。→ Himari 侧要么接受「校验=一次带副作用的试跑」，要么给上游加 `--check-config`。

## 3. TLS 终止 / 动态签发 / 信任根（§55.B「TLS / certificate」）

- **CA**（`src/ca.rs`）：`load_or_generate`（`:45-55`，**cert+key 都在才 load，否则生成**；load 时**不检查到期**）。rcgen 默认 ECDSA P-256，`IsCa::Ca(Unconstrained)`，CA 有效期 `now-1d → now+3650d`，key 落盘 chmod 0600（`:266-274`）。leaf：`not_before=now-1h`（时钟偏移容忍）、`not_after=now+leaf_validity_days`（默认 **397**，`:3717`），EKU 只有 **ServerAuth**。
  ⚠️ **无吊销**：CA 上有 `CrlSign` 但既不出 CRL，leaf 也没 AIA/OCSP/CRL ⇒ 一旦信任，除等到期外无法作废。
- **按 SNI 签发**：`DynamicResolver: ResolvesServerCert`（`resolver.rs:169-180`），用 `block_in_place`（**不是 spawn_blocking**，注释说明必须先持久化再服务、文件 IO 比 keygen 慢约 40×，`:274-287`）。缓存 key 是**精确 SNI**（`(CertScope, sni_host)`，故意不让兄弟名继承镜像覆盖）。
- **磁盘 cert store**（`src/store.rs`）：`<dir>/<scope>/<host>.crt|.key|.observed`，**原子写**（`.tmp` 后缀避免 crt/key 撞名）+ rename，`.observed` 最后写且无观测时删除。
- **越权签发防护**（这是本项目最硬的安全不变量）：一条连接上签出的证书，**不得对本网关路由到别的目的地的任何名字有效**（`certscope.rs:19-22`）。通配需单层 + 过 PSL 可签发判定（`suffix.rs:92-102`，且**只认 ICANN 段**，所以真的 `*.github.io` 镜像能活）；`router_fingerprint`（`:237-278`）让两张相同路由表的 listener（典型是 `0.0.0.0:443` + `[::]:443`）**共享证书**。
- **信任根安装**（`src/trust.rs`，只有 `ensure_installed`；三平台实现在 `:91/:303/:396`）：
  - Windows：**CryptoAPI 进程内**（`CertOpenStore(LOCAL_MACHINE,"ROOT")` + `CertFindCertificateInStore(SHA1)` 幂等探测 + `CERT_STORE_ADD_NEW`，并把 `CRYPT_E_EXISTS` 当成功以熬过并发）。**无 PowerShell / 无 certutil / 无临时文件**；但只有 **LocalMachine** ⇒ 必须管理员，**没有 current-user 选项**。
  - macOS：**shell out**：`security find-certificate -Z -a /Library/Keychains/System.keychain` 比 `"SHA-256 hash:"`，然后 `security add-trusted-cert -d -r trustRoot -k System.keychain`。⇒ **需要 root，且与 App Sandbox / 无提权 helper 的设计直接冲突**（需求书 §20/§21）。
  - Linux：读 `/etc/os-release` 分 Debian/RHEL/Arch 三类，幂等判据是**锚文件是否存在**，更新命令失败会**回滚已写文件**。
  - ⚠️ `fingerprint()` 在 Windows 是 **SHA-1**、macOS/Linux 是 **SHA-256** ⇒ **不是跨平台身份**，Himari 不能拿它当主键。
  - 结果 `Outcome::Installed|AlreadyTrusted` **只进日志**（`main.rs:166-176`：Installed→warn，AlreadyTrusted→info），无返回值/无状态文件/退出码不区分。

**sni-gate 完全没有、Himari 必须自己补的（对应 §37 系统恢复）**：**无卸载/移除 API**（已复核：`rg "CertDeleteCertificateFromStore|delete-certificate|uninstall" src/trust.rs` → 零命中），**无用户级信任安装**，**无 SSL 用途策略钉扎**（macOS 不带 `-p ssl`、Windows 不设 `CERT_ENHKEY_USAGE` ⇒ 根被**全用途**信任），**无 CA 轮换/到期告警**。

## 4. 路由 / 上游池 / 探测（§55.B「SNI routing」，§5「多 listener」）

`Router`（`router.rs:27-35`）四张表 + default：`exact HashMap` / `wildcard`（剥掉 `*.`）/ `suffix`（剥掉 `.`）/ `regex Vec`（按声明序，首个命中）。
匹配优先级**固定**：`exact > wildcard(仅左一个标签) > suffix(自右向左最长祖先) > regex > default_route`（`match_host:133-178`）。
**同层重复 key 是加载错误**（`insert_unique:437-450`）⇒ 路由确定性有保障。
`raw` 类型**不终止 TLS**，直接拼 TCP；其余类型终止后按 route type 重新出网。
**不支持任何 client auth**（`with_no_client_auth`，`main.rs:491-498`）。

池与探测（`pool.rs` + `scoring.rs`，即本仓最近两个提交的主题）：CIDR 采样 + **Thompson 采样/Kalman 后验吞吐分布**、`fallback` 索引、NAT64 前缀合成、`dual` 地址族竞速（`6094aa2` 的 dual-stack 竞速）。运行时默认值集中在 `pool.rs:125-143`（端口 443、tcp/tls 3 s、http 5 s、interval 5 m、degraded 30 s 起指数回涨、`fail_threshold=2`、并发 16、Q 0.01 / R 0.1、discount 0.95）。
→ 对 Himari §28「自动模式」而言：**per-endpoint 的健康后验上游已有**，Himari 要做的是 **per-target × per-strategy** 的缓存（§28 那张 `target/strategy/success/latency/timestamp` 表），不要重造池。

## 5. 可观测性（§9 / §34 / §35 的关键判据）

**没有** admin/status HTTP 端点、**没有** IPC、**没有** socket、**没有**状态文件（已复核：`rg "jsonrpc|UnixListener|named_pipe|/status" src/` 只命中池内部 health 字段）。
可用信号只有三种：

1. 进程存活（controller 自己持有 `std::process::Child`，可 `wait()` 拿 exit status）；
2. 监听端口能否 TCP 连上；
3. `tracing` 日志：**stdout**（`main.rs:1290` 默认 writer），级别由 `SNI_GATE_LOG` → `RUST_LOG` → `[global].log`（默认 `info`）决定；**非 JSON**，纯 fmt 层。

结论：需求书 §9「不要通过解析 stdout 判断状态」、§35「统一 runtime state」**必须靠 Himari 自己造**；sni-gate 侧最诚实的健康信号是 §6 那条**真 TLS 握手**（不需要 sni-gate 配合），而「端口能连」只证明 listener 存在、**不证明路由还活着**。

## 6. controller 应采的 health check（独立实现，非上游抄来）

对本地网关做一次**真 TLS 握手**，信任库**只放自己这张 CA**，并断言对端链非空 —— 因为 sni-gate 是**先完成上游握手再呈现动态叶子**，所以一条探测同时证明：本地 CA 链 + 主机名 + 上游可达 + ECH 被接受 + 上游公网证书对内层名字校验通过。
需要配套（都是可测的硬约束，不要写成尽力而为）：固定 worker 上限、每路由 socket/handshake 超时 + 一个全局批次 deadline、**后缀路由用代表子域探测**（apex 可能合法地什么都不服务）、且**校验域必须落在该路由自身边界内**（否则直接报错，防越界探测）。
