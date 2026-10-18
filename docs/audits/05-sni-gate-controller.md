# §14 + §16 — SniGate controller 设计（Himari 自有实现）

需求书 §14 规定职责分离：

```
Himari → SniGate controller → sni-gate process
```

controller 负责：生成配置、启动、停止、restart、health check、version detection、crash detection、config validation、log collection。
sni-gate 负责：TLS、SNI、routing、upstream、certificates、ECH。

§16 指定 `FxxkLocation/Github-direct` 为集成参考：**只研究 architecture / behavior，用 Himari 自己的代码实现，不复制其代码**。本文即该研究的结论。

## 0. 前置事实（决定 controller 形态）

| 事实 | 证据 | 后果 |
|---|---|---|
| sni-gate **没有 lib target**，只有 `[[bin]]` | 本仓 `find src -name '*.rs'` 无 `lib.rs` | 只能**进程集成**，不是 crate 集成（与 §1「优先独立 crate/独立进程/独立组件」相容，取独立进程） |
| sni-gate **无 IPC / 无 status 端点 / 无 pidfile / 无 lock** | `rg "jsonrpc\|UnixListener\|named_pipe\|/status" src/` → 只命中池内部 health | 状态必须**由 controller 构造**，不能指望被控端 |
| sni-gate **无配置热加载**；SIGHUP 只重载 PSL | `main.rs:1294-1298`、`psl.rs:306-315` | reload = kill + 重写 TOML + 重起，**必然断在飞连接**；UI 上要说实话「需重启生效」（§44 不许虚假宣传同样适用于文案） |
| sni-gate 配置错误走 **stderr 且早于 tracing init** | `main.rs:105,:109` | log collection 必须**同时收 stdout 与 stderr**，且退出码只有 0/1 |
| `panic = "abort"`（`Cargo.toml:74`） | — | 崩溃检测读 **wait status（信号）**，不是 exit code |
| 上游 `Router` 同层重复 key / 同址 listener **启动即报错** | `router.rs:437-450`、README 的 duplicate-listener | **跨进程**重复实例不会被上游发现 → §39 单实例锁是 controller 的职责 |
| sni-gate 信任根**只有安装、没有卸载** | `trust.rs` 仅 `ensure_installed`（`:91/:303/:396`），全仓无 delete/uninstall | §37 系统恢复要 Himari 自带逆操作；且 macOS 路径 shell out + root，与 §20/§21「GUI 不整体 root」冲突 → 必须由 helper 执行 |

## 1. 采纳的机制（来自 Github-direct 的行为研究，Himari 自行实现）

以下每条都是**设计决定**，不是代码搬运。

### M1 纯函数 renderer，与 IO 完全隔离
配置渲染做成 `render(plan, paths) -> String`：无 IO、无时钟、无上下文依赖。
收益：golden-file 测试、注入安全断言、以及「这个状态对应的配置就是这段文本」的可审计性。这是参考项目里性价比最高的一条结构决定。
**禁止**用手写字符串拼 TOML 之外再引入模板引擎（拼字符串本身够用，但**必须**由 serde 结构体驱动，避免 `deny_unknown_fields` 踩雷）。

### M2 两阶段发布：requested plan → 验证子集 → 重写 → 重启 → publish
绝不发布没被证明可用的路由；也绝不因单个域名握手失败就废掉整个功能（降级不砖化）。

### M3 真 TLS 握手 = 唯一诚实的 health check
信任库**只放自己这张 CA**，断言对端链非空。因为 sni-gate 先完成上游握手再呈现动态叶子，**一次探测同时证明**：本地 CA 链与主机名、上游 TCP 可达、ECH 被接受、上游公网证书对内层名字校验通过。
配套硬约束（都要有测试）：固定 worker 上限、每路由超时 + 一个**全局批次 deadline**、后缀路由用**代表子域**探测（apex 可能合法地不服务任何东西）、并断言**校验域不得越过该路由自身边界**。

### M4 失败退避按**条目**不按进程
60 s / 5 m / 30 m 阶梯，每轮裁剪到当前计划内的条目，用户显式「Recheck」可绕过等待。
这是防止 UI 反复 thrash 的关键；§28 的策略结果缓存与之共用同一张表。

### M5 复用已验证的工作，按 generation 判定
「重算 plan」不等于「必须重验 + 必须重启」。把决策写成纯函数：
`(requested, published, alive, force, now, retry_table) → reuse | verify`。极好测。

### M6 原子写 + 内容寻址文件名
写：`<name>.tmp` → fsync → 替换。
二进制：`sni-gate-<ver>-<sha256 前 16>`，好处是升级/降级不会覆盖正在运行的进程文件，陈旧版本一眼可见。
（参考项目在临时后缀上有并发碰撞隐患；Himari 用唯一临时名即可。）

### M7 fail-open 的顺序律
发布：**先路由注册表，后 DNS 作用域**；拆除：**先 DNS 作用域，后注册表**。
泛化到 Himari：**网关验证通过之后才装接管规则；拆接管规则之前先杀网关。** 任何子系统故障只降级、不断网（§37 的核心精神）。

### M8 功能开关在「验证过的信任状态」上，不在「我装过了」上
controller 在 CA 未被读回确认为受信任时**拒绝**启动网关；信任状态过期时 UI 自动取消勾选并禁用依赖项。这是「看起来坏了」与「诚实报告」的分界。

### M9 幂等 + 所有权标记 + 精确移除
安装前先查状态，已受信任则短路成功；只碰自己盖过所有权标记的东西，遇到外来对象拒绝操作；移除路径精确到具体条目，**不使用通配清理**。
这条直接对应 §37 与 §19「uninstall cleanup」，也是 sni-gate 侧唯一必须由 Himari 补齐的能力。

### M10 有界、结构化的状态对象流向 UI
`active / generation / routeCount / … / detail`，失败**阶段码**与**消息**分离（如 `engine.init`、`backend.start`、`rules.refresh`），诊断文本做折叠与截断。
对应 §35：GUI 只渲染状态；§29：Diagnostics 要能回答「为什么失败」而不是只给错误码。

### M11 供应链纪律
钉住不可变 upstream commit（不是 tag 名）；构建脚本在「工作树脏 / revision 不等于钉住的 commit」时**直接拒绝构建**，并在 `finally` 里反向应用补丁；用**哈希把产物和代码常量绑在一起**——CI 里把源码声明的哈希表与随包产物逐个重算比对，漂移即失败。
对应 §42（license/version/attribution/redistributability）与 §41（bundle 内二进制的签名与来源关系）。

### M12 探针不许削弱校验
明确禁止 Trust-All；no-SNI 路径仍校验链并**手工做主机名验证**。
落法：写成一条自检测试（扫自己代码里的被禁模式），而不是靠 code review。

## 2. 明确**不采纳**（含理由，避免后来者重新讨论）

| 不采纳 | 理由 |
|---|---|
| 用 `su` + 生成的 shell 脚本当进程 API（含放宽字符白名单的「trusted script」通道） | 那是 Android 无法直接 fork 特权子进程的产物；Rust 用 `std::process::Command` + 服务句柄更简单也更安全 |
| 把 28 MB 多 ABI 的 ELF 二进制塞进资源包 | 桌面应「下载并校验」或每个平台安装包各带一个；**采纳哈希纪律，不采纳 in-package 载荷** |
| hosts 文件式候选来源（第三方 github-hosts、`api.github.com/meta` 快照、社区种子） | 属第三方控制的路由数据，与 §43「Himari 自己维护自己的数据」冲突；保留 DoH + 每连接解析模型 |
| `HostsProvider` / 1352 行的 name→IP 表抽象 | Himari 的路由属于 sni-gate 的 matcher，不属于主机名表 |
| iptables/ipset/REDIRECT/`SO_ORIGINAL_DST`、`VpnService`、虚拟 IP 池 | 全是 Android netfilter 产物。macOS 走 pf/BPF/Network Extension，Windows 走 WinDivert/系统代理，**只搬 fail-open + 事务回滚 + 装前先探能力这套纪律** |
| Magisk 模块注入 / APEX cacerts / `nsenter` 进 Zygote / 重启暂存态 / 浏览器托管策略 | Android CA 管路专用（`subject_hash_old` 也是），只保留「派生稳定标识符使重复安装成为 no-op、移除精确」这个概念 |
| OEM 保活 hack（ColorOS/HANS lease 等） | 桌面有真实服务生命周期语义 |
| **为生成 CA 而用一条假域名路由先起一个 TLS 网关**（`ca-init.invalid`） | 该 hack 只因为 sni-gate 是 CA 生成器且不能零路由启动。Himari 若需要密钥就**在进程内用 rcgen 生成**（sni-gate 本就用 rcgen），省掉「起一个网关造 CA 再停掉」 |
| `isLocalPortOpen` 当健康判据 | 端口开只证明 listener 存在。Himari 把子进程**真握着 `Child`**（可 `wait()`、拿退出状态、读管道），health 用 M3 |
| 停止时不清理磁盘产物、日志不轮转不封顶 | 属缺陷。Himari 要有大小封顶的轮转日志 + 显式 uninstall 清干净（binary+TOML+cert store+trust 条目） |
| 把 `fragment_client_hello` 当可用开关 | 参考项目里该功能**只存在于补丁**，renderer 从不发出，其文档记录「没帮助、生产配置里没带」。**不携带死旋钮** |

## 3. 隔离清单（§15/§43 的合规证据，禁止进入 Himari）

`FxxkLocation/Github-direct` 内以下材料**只看路径、不取内容、不搬运**：

- `app/src/main/assets/sni-gate/<abi>/sni-gate` —— 28 MB 别人重编译的 sni-gate 载荷；其 SHA-256 表（`EXPECTED_SHA256`）与 `-ghd1` 版本串描述的是**他们的构建**，**不得**用作 Himari 的钉扎值。
- `third_party/sni-gate/patches/0001-opt-in-clienthello-record-fragmentation.patch` —— **708 行、内含 503 行 Rust 的 sni-gate 源码补丁**，实质是 sni-gate 源代码寄存在别人仓里。禁止转录、禁止「改写式」复用。
- `app/src/main/assets/rules/profiles.json`（222 KB / 61 profiles / 约 210 条域名规则）—— 别人的路由语料。**⚠️ 额外风险**：其中多条 `notes` 反复引用一份「架构 §12.1」矩阵并描述 nginx 反代适配问题（`nginx h2 协商 403`、`X-Forwarded-For 伪造为反代专属`、`矩阵 SNI 伪装 amazon.com` 等），即该语料疑似**从一个 nginx 式逐站反代项目分析衍生而来**；而那份被引用的文档不在本仓（`docs/` 只有两个文件，30 个提交的历史里也没有被删的文档）。因此这份 JSON 既有版权疑点，又可能间接携带 SNIBypassGUI 类项目的分析结论 —— **连同它的 notes 一起隔离**。该语料还含大量成人站点条目，桌面产品自带此类清单有独立的名誉/合规问题。
- `app/src/main/assets/routes/github_snapshot.json`、`google_frontend_prefixes.json` —— 他们的快照产物（Google 那份若要范围请**直接向 Google 取**）。
- `GithubHostsProvider.kt` 里硬编码的第三方 hosts URL。

Himari 的数据集一律走 §43 的口径：**自主扫描 → 自主测试 → 自主验证**，产出自己的数据。

## 4. §14 九项职责的落点（下一步可执行清单）

| 职责 | 落点 | 关键约束 |
|---|---|---|
| 生成配置 | `gate/config_render.rs`（M1，由 serde 结构驱动） | `deny_unknown_fields` 全覆盖 ⇒ 多键即失败；裸端口会展开成 IPv4 wildcard；`upstream` 省略会继承监听端口 ⇒ **反射型路由必须显式写 `upstream = "443"`，否则自环**（这是参考项目踩过的坑） |
| 启动 | `gate/process.rs`：`Command` + 捕获 `Child`，`-c <abs path>`，**显式设 CWD**（配置默认值相对 CWD），stdout/stderr 各起读线程 | 参数只有 `-c`；没有 env 覆盖配置文件路径 |
| 停止 | SIGTERM（Unix）/ 优雅关闭尽力（Windows 无信号） | **必须等就绪之后再发 TERM**：实测就绪前发 TERM 会以信号 15 死亡（被误判为崩溃），就绪后才是 exit 0。上游**不 drain 在飞连接**，UI 不得宣称「平滑关闭」 |
| restart | generation 变化时：先撤接管规则 → kill → 写新 TOML → 起 → M3 验证 → 发布（M7） | 无热加载，必须诚实标注 |
| health check | M3 真握手 + M5 generation 复用；**不靠轮询端口**（§52 禁止疯狂轮询刷新） | worker 上限 + 全局 deadline |
| version detection | `sni-gate -V`（存在，已复核）+ 钉扎 commit（M11） | 只有 0/1 退出码，不能用退出码区分原因 |
| crash detection | 握 `Child` → `wait()` 拿状态；`panic=abort`  ⇒ 按信号判定；重启阶梯 + 连续失败保险丝（超限则禁用该功能并暴露状态） | §36 不许「显示运行但进程已死」 |
| config validation | 一次性 config：`addr = 0` + 不可达 `raw` 路由，退出码判合法性 | 副作用：会写 CA 目录、可能触发 PSL/http2 网络 IO；**建议向上游提 `--check-config`** |
| log collection | stdout+stderr 双管道 → §34 结构化（timestamp/component/event/domain/strategy/result/latency/error），带大小轮转 | 上游非 JSON；**默认不落敏感流量内容**（§34） |
