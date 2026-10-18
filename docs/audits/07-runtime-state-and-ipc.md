# §35 / §36 / §37 / §49 — 统一 runtime state、崩溃恢复、系统恢复、IPC

本篇是**设计**，不含上游改动，故不受 `00-index` 的 D-1/D-4 阻塞，可先实现。
每条守卫都配一条**证伪测试**（阳性对照）：先证明「违规时该断言必须变红」，再宣称它守住了。

---

## 1. §35 状态模型

### 1.1 唯一真源

需求书给的枚举就是合同，不自创状态：

```
Stopped | Starting | Running | Stopping | Failed | Recovering
```

组件维度（对齐 §35 的 JSON 示例，逐键）：

```json
{
  "himari":    "running",
  "sni_spoof": "active",
  "sni_gate":  "running",
  "xray":      "running",
  "proxy":     "ready"
}
```

⚠️ 注意 §35 混用了两套词（`running` 与 `active`/`ready`）。合同化处理：
- **`ComponentState`** 严格取上面 6 个枚举值（机器可读、GUI 只渲染这个）；
- `sni_spoof: "active"` / `proxy: "ready"` 降级为 **§1.3 的展示层派生标签**，不再是状态本身。
这样 §9「connected / starting / stopping / failed / degraded」里的 `degraded` 也有了位置：它是**派生**的（见 §1.4），不需要往 6 态里塞第 7 个。

### 1.2 每组件的状态机与合法迁移

```
Stopped --Start--> Starting --ok--> Running
                        --err--> Failed --retry--> Starting
                                  Running --Stop--> Stopping --> Stopped
                                  Running --crash--> Recovering --> Starting | Failed
```

**禁止的迁移**（必须写成断言，不是约定）：
- `Starting → Stopped`（跳过 Running 直接「已停止」是 §24 说的「假 Running」的镜像问题）
- `Running → Failed` **不经过** `Recovering`（崩溃必须先尝试恢复；只有恢复预算耗尽才 Failed）
- 任意态 → 同态（幂等噪声；用 `generation` 单调计数区分「真的变了」与「重复上报」）

每个组件带 `(state, generation, last_error_stage, since: Instant)`。`generation` 单调递增 ⇒ §52「不要轮询刷状态」有了实现依据：**只有 generation 变化才推 UI**。

### 1.3 展示层派生（GUI 只读这里，不做任何猜测）

| 展示 | 派生规则 |
|---|---|
| `● Protected` / `● Off` | 全部关键组件 `Running` ⇒ Protected；否则按最差组件取词 |
| `SNI Protection: Active` | `sni_spoof == Running` |
| `HTTP 127.0.0.1:1080` | 来自 **实际 bind 回来的地址**，不来自配置文本 |
| `Degraded` | ≥1 个**非关键**组件 Failed，或 sni-gate 路由验证子集 < 请求子集（controller M2 的「有路由被剔除」）|

> ⚠️ 第 3 行是真要求：sni-gate **从不回报临时端口**（`proxy.rs:277` 打的是配置里写的地址，实测）。所以端口配 `0` 时，**Himari 不能显示配置值**，得自己从 bind 结果拿。上游若给不了，Himari 侧的 proxy 监听必须自己 bind 后 `local_addr()`。

### 1.4 每条守卫的证伪测试

| 守卫 | 阳性对照（必须红） |
|---|---|
| 状态只来自结构化上报 | 往 core 的 stdout 灌 `Running` 字样 ⇒ 状态**不得**变 Running |
| 崩溃不被显示成 Running | 把被管进程 `kill -9` ⇒ 必须 `Recovering`，且 ≤ N ms 内被检出 |
| generation 单调 | 人为重放同一事件 ⇒ 断言不产生第 2 次 UI 推送 |
| 端口来自 bind | 配 `:0` ⇒ 显示端口必须 ≠ 0 且与实际监听一致 |
| 派生 `Degraded` | 剔除 1 条路由后 ⇒ 首页必须出现 Degraded，不得显示 Protected |

---

## 2. §36 崩溃恢复矩阵（检测手段全部基于已核实事实）

| 崩溃对象 | **检测** | **恢复** | UI 更新 |
|---|---|---|---|
| GUI | helper/service 侧守护：客户端 socket 断开即视为 GUI 丢失；**心跳文件 + 陈旧阈值**（controller 研究里 M7 的 guardian 形态） | 不重启 GUI（用户可感知），但**超过 T 秒无 GUI 且用户已退出** ⇒ 撤销系统级改动（见 §3 账本） | — |
| Core（sni-spoof 转发） | `RunningProxy::is_running()`（上游 `proxy.rs:44-49` 用 `JoinHandle::is_finished`，实测存在）→ 这是上游唯一真实的存活信号，**必须复用** | `Recovering` → 重建 `start_proxy(config)`；连续失败超 fuse ⇒ `Failed` 并禁用该功能 | 首页胶囊 + `last_error_stage` |
| sni-gate | 控制器**握着 `std::process::Child`** ⇒ `try_wait()`/`wait()` 拿退出状态；`panic="abort"` ⇒ **按信号判定，不看 exit code**（实测退出码只有 0/1） | 重起 + 重跑 M3 握手验证；**不许用「端口能连」代替健康** | `Recovering` → `Running`/`Failed` |
| Xray | `Command` 起的 `Child`（上游 UI 已有此形态，实测 `ui.rs:494` 用 `try_wait`） | 重起；上游是 `child.kill()`=SIGKILL，**Himari 侧改为先礼后兵**（无优雅通道就明确写「强杀」） | 同上 |
| Helper | launchd/服务侧 `KeepAlive` + IPC 连接断开事件 | GUI 重连；helper 不可用 ⇒ 特权功能降级为不可用，**不得**伪装成可用（§34 诚实性） | 明确「需要权限，当前未授权」 |

**fuse 与退避**（与 controller M4 同一张表，别做两套）：重启阶梯 1 s / 5 s / 30 s / 5 m，**连续失败上限后禁用并上报**，健康 5 分钟后重置计数。

> §52 明令「不要用轮询疯狂刷新状态」。上表除 GUI 心跳外全是**事件驱动**（`is_finished` / `wait` / socket 断开），符合。

---

## 3. §37 系统恢复账本（本篇最需要新写的部分）

§37 要求：Proxy / DNS / PF / Routes / Helper / Services **任何被改过的系统状态都可恢复，异常退出也要能恢复**。
上游与参考项目都没有这个（sni-gate 的 `trust.rs` **只有装没有卸**，实测零删除函数）。所以这是 Himari 净增的机制。

### 3.1 形式化

一个**先写日志再动手**的账本（write-ahead journal），落在他用户不可读的私有数据目录：

```
before(op): append{kind, target, prior_state_snapshot, inverse_op, ownership_marker, ts}; fsync; then mutate; then verify
on_startup: if journal has incomplete entries ⇒ 按逆序 replay inverse_op
undo_all:   逐条执行 inverse_op，逐条 verify，失败即停在原地并升级给用户（不许静默跳过）
```

关键约束（都来自实测事实，不是美学）：

1. **必须有 `prior_state_snapshot`**。§22 明令绝不能「关掉 Himari → 直接清空代理」，要保存 HTTP/HTTPS proxy、bypass list、PAC 状态、当前 network service。⇒ `inverse_op` 只能是「恢复快照」，不能是「关闭开关」。
2. **必须有 `ownership_marker`**。参考实现的所有权标记/拒绝外来对象/精确移除（M9）在此落地；清理**永远不使用通配**。
3. **verify 用回读，不用返回值**。例如装完信任根后必须**回读系统信任存储确认存在**；卸载后确认**不存在**。且注意 sni-gate 的 `fingerprint()` 在 Windows 是 SHA-1、macOS/Linux 是 SHA-256（实测）⇒ **不能拿它当跨平台主键**，账本自己的 ID 必须与之一致无关。
4. **网络变化后重检接口，禁止永久缓存 interface name**（§23）。账本里的 `target` 若含接口名，replay 前必须重新解析当前活动服务，否则会把恢复操作打到已消失的服务上。

### 3.2 需要登记的操作类型

| kind | 改动 | inverse | 检测/verify |
|---|---|---|---|
| `system_proxy` | macOS `networksetup` 代理 / Windows 注册表代理 | 恢复快照 | 回读当前代理配置 |
| `dns` | resolver 指向本地 DNS | 恢复快照 | 回读 + 一次真实解析 |
| `ca_trust` | 安装本地 CA 到系统信任根 | **删除该条目**（Himari 自实现，上游没有） | 回读存在/不存在 |
| `pf` | macOS pf 锚点 | 移除锚点 + 恢复原 anchor 文件 | `pfctl -s` 回读 |
| `route` | 策略路由/转发 | 删除自建路由 | 回读路由表 |
| `helper` | 安装特权 helper | 卸载 helper + 删除 plist | 服务列表回读 |
| `service` | Windows 服务 | `sc delete` | 服务存在性回读 |
| `listeners` | 本地端口占用 | 关闭子进程 | 端口不再可连 |

> PF 与 Network Extension 属 §25 的未来项：账本**接口现在就留好**（`kind` 是枚举 + `inverse` 是数据），但**第一阶段不实现完整 VPN**（§25 后半句同样明确）。

### 3.3 证伪测试（账本没有阳性对照就是装饰）

- 直接 `kill -9` Himari（不留 cleanup 机会）⇒ 重启后 `on_startup` 必须把代理/DNS 恢复原样。
- 账本里塞一条**不属于自己**的条目（改 ownership_marker）⇒ `undo_all` 必须**拒绝**并报错，不得删。
- 断网/切 Wi-Fi 后重放 ⇒ 必须以「当前活动服务」为 target，测试里用假接口名制造陈旧 target。
- CA 装完立刻回读必须为真；`undo_all` 后回读必须为假。

---

## 4. §49 IPC —— 逻辑接口跨平台同一套

### 4.1 方法合同（需求书点名的 12 个，逐条给签名）

```
get_status()                      -> RuntimeState (§1.1) + generation
start(target: StartTarget)        -> Accept | Busy | NeedPrivilege
stop(target: StartTarget)         -> Accept | Busy
restart(target: StartTarget)      -> Accept | Busy
get_config()                      -> HimariConfig (结构化, 不是 JSON 文本)
set_config(HimariConfig)          -> Ok | Invalid{field, reason}   // reason 必须可读, §29
scan(ScanRequest)                 -> JobId       // 异步, §7「UI 不能卡死」
   scan_results(JobId) -> Stream<ProbeResult>    // 事件推送, 不是轮询
test_strategy(target, strategy)   -> JobId        // §28 自动模式的单步
get_logs(LogQuery)                -> Page<StructuredLog>  // 分 5 类 component, §34
get_stats()                       -> Stats        // 连接数/字节数, 不含载荷内容
get_sni_gate_status()             -> GateStatus   // 含 verified/requested 路由计数 (§1.4 Degraded 来源)
get_xray_status()                 -> XrayStatus   // 含 binary 版本/路径

// —— 需求书之外新增（§49 写的是"至少包含"，见 D-11）——
get_diagnostics()                 -> [DiagnosticRow{area, verdict, why, remedy?}]
                                   // verdict ∈ {ok, warn, fail, idle, unknown} 闭集;
                                   // 只读、无副作用; §29 的证据只有服务能出，UI 不许自己碰
                                   // networksetup/BPF (§47/§48)
```

`Accept` 表示**已受理**，不是**已成功**——所有长操作走 JobId + 事件，`§35` 的 6 态由事件推进。这条区分是防「假 Running」的根。

### 4.2 transport 与约定

- **macOS**：Unix domain socket，落在用户私有 runtime dir，权限 `0700` 目录 / `0600` socket（同用户即可连，**不因此提权**，符合 §20）。
- **Windows**：named pipe，同样限同用户。
- **framing**：4 字节长度前缀 + JSON（serde）；`schema_version` 进 envelope，**不兼容即拒绝并明确报错**，不做静默降级。
- **push**：服务端在同一连接上推 `Event{state|log|scan_progress|stats}`；客户端可 `subscribe(filter)`。⇒ §52 的反轮询有实现路径。
- **§49 尾句合同**：逻辑接口两端一致，transport 才不同。⇒ 方法签名里**不得出现平台类型**（无 `HANDLE`、无 `int fd`）。

### 4.3 禁止事项（直接引需求书）

- §9：**不解析 stdout 判状态** —— stdout 只进 `get_logs`。
- §48：UI 不得直连 BPF/PF/WinDivert —— UI 只会说上面这些方法名。
- §34：日志不含 cookies / authorization headers / request bodies / 明文应用数据；`get_stats` 只给计数与字节数。

### 4.4 单实例（§39）与 IPC 的绑定

第一份连上 socket 的 GUI 成为唯一实例；后来的 GUI 进程**发现锁已存在 ⇒ 唤起已有窗口并退出**。
锁必须是**同一把**：socket 文件 + `flock`（macOS）/ pipe 名（Windows），并处理 stale（helper 死了但 socket 残留）。
> 上游两项目**都没有** pidfile/lock（实测），所以这条完全没有现成依赖，是净增。
