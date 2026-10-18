# 交接：Himari 构建会话（截至 2026-10-04，Gate AA–AK）

这份文件存在的唯一理由：`08-phase3-gates-evidence.md` 与代码树都会被并行会话整文件写回覆盖（已发生过一次把标题吃掉的事件）。下面这些是**已经被命令输出证明过**的东西，重写成本最低。

## 一、如何复现当前状态（权威口径）

```sh
cd /Users/glow/Base/Code/Prometheus/himari-app
HIMARI_SNI_GATE_BIN=/Users/glow/Base/Code/Prometheus/Himari/target/release/sni-gate \
  cargo test --workspace          # 312 passed / 0 failed / 0 ignored
cargo clippy --workspace --all-targets   # 只剩第三方 block v0.1.6 的 future-incompat 提示
```

不带 `HIMARI_SNI_GATE_BIN` 时，网关生命周期那几条**故意 panic**，绝不静默跳过变绿。数字只在带变量时成立。

## 二、工作树是脏的，而且这是最大风险

未提交：`crates/himari-daemon/`、`crates/himari-platform/`、`crates/himari-strategy/`、`crates/himari-helper/`（整目录未跟踪），以及 `crates/himari-ipc/{src/lib.rs,src/lock.rs,src/proto.rs,tests/}`、`crates/himari-ui/{src/bridge.rs,src/client.rs,src/diagnostics.rs,tests/*}`、`THIRD-PARTY.md`、`LICENSE-MIT`、`LICENSE-APACHE`、`packaging/`、`scripts/build-macos-app.sh`。

**没提交的原因不是忘了，是在等你定两件事**：① 提交到哪条分支（`main`，还是兄弟会话在用的 `flutter-frb`）；② 是否按"逐文件路径尽早提交 + `--allow-merged`"的方式走 GitButler（它的未提交桶全树共享，别人一次提交会把我这些文件一起吞掉）。

## 三、每节 Gate 真正抓到的是什么（一句话版）

| Gate | 领域 | 抓到的缺陷/新增证据 |
|---|---|---|
| AA | §40/§41 打包 | 仓库根本没有自己的 LICENSE 文件；MIT 版权行是占位，待你定署名 |
| AB | §38 登录项 | `terminate()` 会无条件回滚**用户主动开启**的启动项 ⇒ `undo_except` |
| AC | §29 取数 | 诊断页的证据进不了 UI ⇒ 新增只读 `get_diagnostics`；`Report::all_clean()` 在空集合上是假绿 |
| AD | §9–§11 端点 | `set_config` 原本一律回 "not implemented"、`get_config` 回**写死的默认值** |
| AE | §10/§53 落盘 | 配置只在内存；畸形文件必须"宣布后退回默认"，不静默采信 |
| AF | §34/§49 | `Push::LogLine` 定义了却没人广播；`subscribe` 的过滤器被 `Accepted` 后**直接丢弃**；连带照出 `ScanFinished` 不归 `ScanProgress`（客户端只能靠超时区分"跑完"与"没人喂"） |
| AG | §13 DNS | `DnsBackend` 零实现；"不许改系统 DNS" 现在是全树扫描＋阳性对照（禁 `/etc/resolv.conf` 绝对路径，不是文件名） |
| AH | §34/§37/§40 | 磁盘不许是脱敏的绕行路：守卫读**原始字节** |
| AI | §47/§48 | 层次禁令从 Cargo 描述里的散文变成门禁；**临时塞违规文件证明它会红**后才算活的 |
| AJ | §11 | GUI 的 LAN 开关是**装饰品**（发出的请求数为 0） |
| AK | §38 | 自启开关同样没接线；接法是"装完返回回读 rows"而不是 `Accepted` |

## 四、待你拍板（都卡在授权/凭据，不是卡在我）

- **D-9**：允许不允许做一次 `networksetup` 往返验，证明"普通用户的写入到底落不落地"。不做的话 §20 helper 的立项理由永远是一句推断。
- **署名口径**：`LICENSE-MIT` 的版权行现在写的是 `Copyright (c) 2026 Himari contributors`。
- **提交与分支**（见第二节）。
- **§30/§31 视觉验收**：界面逻辑通路已有，但"看起来对不对"只能你的眼睛判——本机能证明的都写在 Gate 台账里了。

## 五、明确不做（除非需求书改口径）

全功能 VPN、QUIC/HTTP3 完整栈、重写 sni-spoof-rust、把 SNIBypassGUI 的 payload/域名表/站点库搬进来（§43 禁）。

## 补充（Gate AL，同日稍后）

- **实测边界**：本机 `/dev/bpf` 打不开 ⇒ `SpoofGuard::start` 必失败（`NeedsPrivilege`）。因此 `the_system_proxy_follows_the_listener_either_way` 在本机只走左支；右支与 §24 的 BPF 生命周期**需要 sudo 或 helper 才有验证条件**。这条也解释了为什么 §22 的 GUI 开关被实现成"设置 + 关掉即还原"，而不是"立即改写系统代理"。
- **`cargo test --workspace` 必须带 `HIMARI_SNI_GATE_BIN`**：我本会话漏了两次，漏了就是 2 条故意 panic 的红（不是代码坏）。
- §22 / §11 / §38 三个 GUI 开关现在都会真发 IPC；清单上剩下"按了没用"的是 `InstallTrustAnchor`、`PromoteScanResult`、`SetXrayBinary`、`TestStrategy`、`RunScan(Builtin)`，它们对应的服务侧能力本来就未实现。
- 下一步优先级改为：① §12 诚实版 `check_egress`（现在更明确了：无 live 监听器在本机是**必然**状态，所以"报错而不是回 via_himari:false"这条守卫可以完全验）；② `proxy_allowed` 持久化；③ D-9。

- **Gate AM（§40/§45）**：`himari-daemon --uninstall` 已存在；删除表是**固定名字表**（`crates/himari-daemon/src/cleanup.rs::RUNTIME_ARTIFACTS`）。**已知无法自动还原的东西：sni-gate 装的 CA 信任锚**（上游没有撤销 API），卸载文案必须如实说出来。

- **Gate AN（§52）**：`himari-daemon` 的 lib.rs 已按职责拆出 `settings.rs`／`diagnostics.rs`／`gating.rs`／`cleanup.rs`（1786 → 1259 行，测试数不变 285）。**踩过的坑值得接手的人知道**：在 crate 还有编译错误时按 clippy 的 "unused import" 自动删行会删掉**必需**的导入（那时类型还没解析成功，报的是假信号）——清理告警一定等 `cargo build` 干净之后再做。

- **Gate AO（§44/§45/§12）**：钉住的上游核心**没有计数器**（`RunningProxy` 只有 is_running/stop/wait/request_stop）。`get_stats` 与 `check_egress` 现在报"为什么量不出来"而不是漂亮的零；全仓已**没有任何代码路径能构造** `Response::Stats`/`Response::Egress`。`Push::Stats`／`EventKind::Stats` 已删除（Gate AP）：订阅它=永远等不到，属于接口答应了不会发生的事；`Response::Stats` 保留但**无生产者**，是有意不对称（方法当场给理由诚实，永不触发的推送是陷阱）。

- **依赖归因**：`Cargo.lock` 里 `sha2` 家族 5 个新条目来自 `himari-platform` 的既有直接依赖（CA 指纹），§42 台账复跑覆盖到（417 份 manifest 全在白名单内）。- **Gate AQ（§6 部分）**：Xray **链接导入**已可用（上游 `xray::parse_share_link`，`Cargo.lock` 未变）。凭据不外流的机制是**类型**：IPC 只有 `ShareInfo`（无 user_id）、日志只有协议+主机+端口、盘上 `xray-shares.json` 固定 0600，且 §45 的 `foreign()` 完整性守卫会拦"新增运行时文件却没登记到卸载表"。**§6 的进程管理仍未实现**，所以导入的链接目前**没有消费者**——别当成隧道可用。

- **过程纪律（Gate AR 学到、值得接手的人遵守）**：脚本化编辑**每次都要 assert 命中**——我有一次把代码插到了旧位置（`diagnostics()` 已经搬到 `diagnostics.rs`），`replace` 静默什么都没做，是清单长度守卫逼出来的。第二教训：替换掉唯一调用者之后要顺手删死代码（clippy 会报 `never used`）。第三：守卫自己的预期也可能是错的，而**它和实现对齐在同一个错上会互相掩盖**——把两种行为分开各钉一条，别写成一个 if。

- **Gate AS**：`XrayBinary::version()` 现在有期限（默认 5s，`with_timeout` 可注入），到期 `kill`+`wait`；守卫用脚本自报 pid + `kill -0` 证明**不留孤儿**——"有超时"这句话不杀进程也能过，所以必须这样量。仍未缓存版本（每次状态查询起一个子进程）。

- **Gate AT**：Xray 探测缓存的键是 `(path, mtime, size)`，**失败也缓存**；`Options::xray_timeout` 注入后服务层超时才可测。写缓存前先问一句"缓存会不会推翻我刚才立的性质"——这里差点把"版本必须来自那个文件"缓存成"记住第一次的答案"。上限：同秒同字节数的替换会漏。

- **Gate AU**：`get_sni_gate_status`／`get_xray_status` 不再借用 `Response::Status`；`Status.components` 由守卫保证**只含组件、只含六个状态词**。写这类表时先问"这键能被 parse 回来吗"。


---

## 末态 14:20（抗覆盖副本；`08` 今天被截断过一次，别只依赖它）

**唯一可复跑的判据（本机实测，非推断）**
```
cd /Users/glow/Base/Code/Prometheus/himari-app
HIMARI_SNI_GATE_BIN=/Users/glow/Base/Code/Prometheus/Himari/target/release/sni-gate \
HIMARI_UPSTREAMS_DIR=/Users/glow/Base/Code/Prometheus/_upstreams \
cargo test --workspace --no-fail-fast        # 末次：69 target / 325 passed / 0 failed / rc=0
cargo clippy --workspace --all-targets       # 末次：0 warning（只剩第三方 block v0.1.6 future-incompat 提示）
```
日志留在 `Himari/.local/ws-12.log`（14:16 那次，clippy 修正之后）。

**今天这条 lane 新落的东西（都验证过）**
- §43 自建语料：`crates/himari-daemon/src/corpus.rs` + `tests/corpus_guards.rs`（7 条）+ `Request::GetCorpus`/
  `Response::Corpus` + `corpus.json` 进卸载表两处。台账 AV。
- §43 继承路径：`crates/himari-runtime/tests/corpus_provenance.rs`（含上游阳性对照 + "名字还在吗"反空壳对照）。台账 AX。
- §42 随仓数据文件一节（PSL=MPL-2.0 按文件头抄、上游候选表 647 条且自认会过期）。台账 AY。
- 网关 `[psl] source = "embedded"` 的渲染守卫：`crates/himari-gate/tests/psl_source.rs`。台账 AZ。
- §22 开关跨重启持久：`settings.json` + 2 条守卫（含"开→重启→仍开"的对照）。台账 BB。
- 修掉一条既有守卫的负载敏感缺陷：`xray_shares.rs` 的单次 `read_to_string(pid_file)` ⇒ 有界轮询 60×50 ms。

**已知不完整性（别当完整台账用）**
- `08` 是 transcript 重建的：`Gate D`、`Gate AK` 两节没能复原（§38 的第二份证据是 `tests/autostart_ipc.rs`）。
- `docs/` 至今未进 git（`?? docs/`）⇒ 台账是单点故障；结论要落进测试文件才算数。
- 全量数字只在**本机 macOS arm64**成立；§5/§7/§46 与 §41 的产物侧从未在这台机上跑通。

**下一轮的具体第一步（按可完成度排序）**
1. `Options::xray_timeout` 仍不持久（BB 只处理了开关）——同一条路：`settings.json` 加字段 + 白名单 + 重启读回守卫。
2. §6 Xray 的 conf 生成 + 起停 + 读回：本机有真 xray（`HIMARI_XRAY_BIN` 已被兄弟 lane 用作必需输入），
   这一条是**能端到端验的**，且直接补 §53 的"三条核心起来"。
3. §12 出口证明 / §22 右支 / §24 BPF：仍被本机 `/dev/bpf` 权限边界挡住 ⇒ 要 §20 helper 落地才能验（D-9 同一条线）。

**队列与授权（不变）**
- `.build-lock`：同一时刻只允许一个构建/测试/flutter run；owner 超 15 分钟才判陈链可抢；`pgrep -c flutter_tester` 非 0 时不起构建。
- 未提交、未 push。**等用户**：提交授权（`main` vs `flutter-frb`）、D-9 一次性 `networksetup` 往返验、
  §49 新增只读方法（`get_diagnostics`/`set_autostart`/`set_system_proxy`/`get_corpus`）是否接受。


## 交接时点的再验证（14:19，本机现跑）

停放前后各跑一次，两次数字相同 ⇒ 并发会话在我这一段落之后**没有把树改红**：

```
cargo test --workspace --no-fail-fast   →  69 target / 325 passed / 0 failed / RC=0
cargo clippy --workspace --all-targets  →  0 warning（只剩 block v0.1.6 的 future-incompat 提示）/ RC=0
```

日志：`Himari/.local/ws-final.log`（14:19）。`10-test-matrix.md` 就是同一轮日志的导出，两者数字一致。
**接续时先重跑这两条**，别拿 14:16 或 14:19 的数字当"现在"——这台机上有 3–4 个会话在同一棵树上排队。
