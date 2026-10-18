# Himari Phase-1 审计 — 索引与未决项

日期 2026-10-03。需求书 §55 要求「开始开发前必须先完成」的 A/B/C/D 四项，产物如下：

| 文件 | 对应条款 | 状态 |
|---|---|---|
| `01-upstream-sni-spoofing-rust.md` | §55.A 上游审计 | 已完成 |
| `02-gate-sni-gate.md` | §55.B Gate 审计 | 已完成 |
| `03-reference-map.md` | §55.C 参考实现归类 | 已完成（9 仓，许可证逐仓 `head -1` 本人复核） |
| `04-feature-parity.md` | §55.D 功能对照表 | 已完成（依赖 03 的部分留空） |
| `05-sni-gate-controller.md` | §14 / §16 controller 设计 | 已完成 |
| `06-toolchain-and-crates.md` | §3 / §4 / §42 / Phase 2 API 面 | 已完成（全部实测取证） |
| `07-runtime-state-and-ipc.md` | §35 / §36 / §37 / §49 | 已完成 |
| `08-phase3-gates-evidence.md` | §51 Phase 3 门禁证据 | Gate A/B/C 已过；Gate D（托盘×iced）未开始 |

审计用只读克隆在 `/Users/glow/Base/Code/Prometheus/_upstreams/`（需求书要求「不要修改」上游，故一律只读，不在 Himari 仓内构建）。

---

## 一、必须先由用户拍板的阻塞项

### D-1（最高优先）：当前仓库身份与需求书冲突

实测：`/Users/glow/Base/Code/Prometheus/Himari` 的 `Cargo.toml` 里 `name = "sni-gate"`、`version = "2.5.1"`，`git remote` 是

```
origin    https://github.com/HibernalGlow/Himari.git
upstream  https://github.com/racpast/sni-gate.git
```

即**这个目录当前就是 sni-gate 的 fork**，且 fork 头 `e6e807e` 与 upstream 头完全一致（尚未分叉）。

而需求书 §1 规定：主上游是 `therealaleph/sni-spoofing-rust`，sni-gate 只是**第二个、独立集成**的上游（§1「优先作为独立 Rust crate / 独立进程 / 独立组件集成」）。

两条路不可同时成立：

- **(a) 本目录继续当 sni-gate 的 fork**：保持 `git fetch upstream` 干净（现在只差 0 个提交），Himari 产品代码另起一个仓库，用 git 依赖/submodule 引 sni-gate 与 sni-spoofing-rust。
- **(b) 在本目录长成 Himari 产品**：GUI/platform/orchestration 的提交会和 sni-gate 的 upstream 历史混在一条线上，§2 要求的「方便同步上游」对 sni-gate 这条线**永久退化**（此后每次 `git fetch upstream` 都要在混合历史里做三方合并）。

推荐 **(a)**，依据是 §2 与 §47（`third-party/` 独立组件目录）把两个上游都当成外部可同步单元。选 (b) 也能做，但要明确接受 sni-gate 同步成本。

> **进展（2026-10-03）**：已按 **(a)** 开工 —— 产品代码在同级新仓 `Code/Prometheus/himari-app`（workspace：`himari-core / himari-gate / himari-ipc / himari-runtime / himari-ui`，上游走钉 commit 的 Cargo git 依赖）。本目录保持**未分叉**的 sni-gate fork（HEAD 仍等于 `upstream/main`，只多 `docs/`）。这不是不可逆决定：若你要改成 (b)，搬家成本是把 `crates/` 移进本仓一次提交。

### D-2：§5「HTTP/HTTPS/SOCKS5 proxy」在 Rust 核心里并不存在

实测：`sni-spoofing-rust/src/` 无任何 SOCKS/HTTP 代理协议实现，`src/listener.rs` 是**TCP 转发 + SNI 注入**；1080/1081 的 HTTP/SOCKS 全部由 UI 生成 Xray inbound 配置提供（`src/bin/ui.rs:2147-2165`）。需求书把「HTTP proxy / HTTPS proxy / SOCKS5 proxy」列在 *Core proxy* 标题下，属**对上游能力的误判**。

需用户定：Himari 是（i）照现状继续让 Xray 提供应用层代理（功能对等，最快），还是（ii）在 Himari 里自己实现 HTTP/SOCKS5 入站（新增核心代码，§1「不要重新发明底层」的边界要重新解释）。

### D-3：GUI 是「重写」不是「移植」

上游 UI = `eframe/egui 0.28`（`Cargo.toml:36`，`required-features=["ui"]`，`src/bin/ui.rs` **2496 行**）。需求书 §3 强制 Iced。egui 是即时模式、Iced 是 Elm 架构，状态模型完全不同 → **§51 Phase 3 的工作量应按「新写」估，不能按「迁移」估**。

### D-4：Phase 2 的硬阻塞 —— 核心模块是私有的

`src/lib.rs` 只 `pub` 了 `config/error/packet/scan/xray` + `proxy` 的部分再导出；`sniffer / proto / handler / listener / relay / shutdown` **全是私有 mod**。后果：外部 crate  today **无法实现或选择 RawBackend**，也就无法在不动上游的前提下做 §48 的分层与 §25/§26 的 `TrafficBackend` 抽象。

这与「不要修改上游」直接冲突，需要决定：向上游提 PR 把可见性放开 / Himari 以 patch 方式维护分叉 / 或把核心当子进程用（放弃 §49 的 in-process 结构）。推荐第一种。

### D-5：§9/§35「状态必须来自正式 API」目前零基础

实测：`sni-spoofing-rust` 无任何 IPC（`rg jsonrpc|UnixListener|named_pipe|ipc src/` → 0 命中）；sni-gate 同样只有 stdout 日志 + `exit 0/1`，无 status 端点、无 socket、无 pidfile。所以 §35 的统一 runtime state **是净新增设计**，且 sni-gate 侧只能靠「子进程存活 + 监听端口可连」间接推断（Github-direct 就是这么做的，其审计把这标为设计债）。

### D-6：§37「系统恢复」缺 sni-gate 侧能力

`sni-gate` 的 `src/trust.rs` 只有**安装**信任根（Windows CryptoAPI / macOS `security add-trusted-cert` / Linux 发行版探测），**没有卸载 API**，也**没有 CA 轮换与到期告警**。macOS 路径还是 shell out + 写系统钥匙串（root）。§37 要求「任何改过的系统状态都可恢复」→ 卸载/轮换必须在 Himari 侧补，或改上游。

### D-7：许可证与再分发

- `sni-spoofing-rust`：**MIT**（`LICENSE:1`，版权行只写 `Copyright (c) 2025`，无主体名）→ 源码可闭源再分发，需保留声明。
- `sni-gate`：**MIT OR Apache-2.0**。
- ⚠️ 上游 `releases/` 目录**提交了约 87 MB 预编译第三方二进制**（Xray / WinDivert / Wintun）。其中 **WinDivert 是 LGPLv3/商业双授权**，不能想当然随 Himari 安装包分发。§42 要求记录 license/version/attribution/redistributability —— 这一项必须单独决策。
- 需求书 §43/§15：SNIBypassGUI 的 payload / supported_sites.dat / nginx 站点配置 / 配置树**禁止复用**，隔离清单见 `03-reference-map.md`。

### D-8：上游默认值是「LAN 暴露」的

提交进仓库的 `config.json:4` 是 `0.0.0.0:40443`。需求书 §11 明确要求「默认继续使用 localhost / 防止意外暴露互联网」→ Himari 的出厂默认必须改，且这是**行为差异**，不是 bug 修复，需在对照表里显式标注。

---

## 二、本轮已验证的事实基线（非二手）

自己跑过的检查，不依赖子代理转述：

- `wc -l src/**/*.rs` → 合计 8216 行，其中 `src/bin/ui.rs` 2496 行（≈30%）。
- `head -1 LICENSE` → MIT License。
- `rg "eframe" Cargo.toml` → `0.28`，`ui = ["dep:eframe"]`。
- `rg -n "trait RawBackend" src/sniffer/mod.rs` → `:20 pub trait RawBackend: Send + 'static`。
- `rg -n "cfg(target_os" src/proxy.rs` → `:85/:89/:93`（backend 选择）+ `:207/:211/:215`（提权提示）。
- `rg "1080|1081|40443"` → 默认转发端口 40443（`src/config.rs:113`），Xray 代理 1080/1081（`ui.rs:979-987`）。
- `rg -c "jsonrpc|UnixListener|named_pipe|ipc" src/` → **无命中**。
- 本仓 `Cargo.toml` → `name="sni-gate"` `version="2.5.1"`；`find src -name '*.rs'` → **无 `lib.rs`**（只有 `[[bin]]`）。
- `_upstreams/sni-gate` HEAD `e6e807e` == 本仓 HEAD。

本目录还有一份 **`10-test-matrix.md`**：§50 的测试矩阵，**由 `cargo test --workspace --no-fail-fast` 的日志机器导出**
（末次：48 个集成测试文件 / 325 条测试 / 全 ok）。它同时列出**§50 里目前没有尺的行**，所以它不是覆盖声明；
每次全量跑完要重导一次，别手工维护。

## 下一步的入口条件（阶段 4：§20/§21 特权 helper）

helper 的职责边界、鉴权口径、恢复顺序与"开工第一件实测"清单在 **`docs/privileged-helper.md`**；那一节只列接缝，规格不在这里重复。

按证据复述当前停放点：`cargo test --workspace` **325 passed / 0 failed / 0 ignored**（69 target，`--no-fail-fast`）（需带 `HIMARI_SNI_GATE_BIN`，否则网关生命周期那几条**故意 panic 而不静默跳过**；本轮该数字来自单次完整 `cargo test --workspace`，`autostart_ipc`/`lan_share_wiring`/`structured_logs` 三组各自复跑过），`cargo clippy --workspace --all-targets` 只剩第三方 `block v0.1.6` 的 future-incompat 提示。证据链见 `08-phase3-gates-evidence.md` 的 Gate P（§22）/Q（§42）/R（§36）/S（§29）/T（§23）/AA–BB（§40–§42、§38、§29 取数通道、§9–§11 可编辑端点、§10/§53 端点落盘、§34 结构化日志与 §49 订阅过滤、§13 `DnsBackend`、§34/§37/§40 日志落盘、§47/§48 层次门禁、§11 LAN 开关接通、§38 自启开关接通、§22 系统代理开关（含本机 BPF 权限边界的实测）、§40/§45 卸载清理、§52 拆分巨型文件、§44/§45/§12 取消编造测量、§49/§52 删除永不触发的订阅面、§6 Xray 链接导入（凭据不外流）、§6 Xray 二进制版本探测（含超时与不留孤儿）、§6 带身份失效的探测缓存、§49 状态方法的正式形状、§43 自建语料（含 §42 豁免表过期被自己的断言抓住）、§43 继承路径的文本门禁（含守卫自豁免与 `--no-fail-fast` 计数口径）、§43 第二个门的封堵、§22 系统代理开关的跨重启持久（§53 意义的真缺陷修复）、§42 随仓数据文件（PSL=MPL-2.0 按文件头抄、上游候选表 647 条且自认会过期、`[psl] source = embedded` 不触发运行期下载）、网关配置渲染把那条下载路径钉住（含反空壳对照：上游不许只剩 embedded）。

helper 要接的三件现有东西，位置都是确定的：

1. `himari-platform::PlatformError::NotApplied` —— 现在系统代理"写被接受但没落地"就映射到 `NeedsPrivilege`，helper 是这个错误码的**真正出口**（§20：GUI 全程非特权，提权只在安装时一次，之后走受控 IPC）。
2. `himari-platform::interfaces::resolve` —— §23 每次现问"当前哪个服务在带路由"。helper 落地后**不许改成由 helper 缓存服务名**：它只能被问，不能记事。
3. `himari-platform::Ledger`（含 `undo_kind`）—— helper 干的任何持久变更（BPF 设备权限、launchd 标签、pf anchor）都必须走同一套写前账本，§37 的异常退出恢复才有覆盖面。

**开工前必须由用户拍板的三问**：

- **D-9 非特权 `networksetup` 写入到底落不落地。**（§22 开关的接线顺序也卡在这条：先有确认的服务名，再谈 GUI 开关） 现有一切只到"rc=0 + `preferences.plist` mtime/大小不变"，无法在不改动用户实际配置的前提下判定。要么允许一次带前后读档的往返验，要么 helper 直接按"必需"立项。
- **D-10 `himari-app/flutter/` 与 §3 冲突。** ~~留还是撤~~ **已拍板（项目记忆记录，非本会话决定）：走 Flutter+FRB，§3 的"必须 Iced"作废，Iced 路线并行保留。** 本会话继续只在 `crates/himari-*` 里落实现，不碰 `flutter/`；§3 的原文与这条决定的冲突在需求书面尚未回写。
- **D-11 §23 的答案要不要进 §49 方法表。** **本会话按"§49 是下限（'至少包含'）"的口径先做了**：新增**只读**方法 `get_diagnostics`（Gate AC），不改 `get_status` 的字段。判据是加一个只读查询不影响任何调用方，而扩 `get_status` 会让状态帧变成杂物间。你要是认为契约变更必须等授权，这次改动能整体回退（集中在 `proto.rs`、`himari-daemon/src/lib.rs`、`himari-ui/{bridge,client,app,diagnostics,main}.rs` 与其测试）。
