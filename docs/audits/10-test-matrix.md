# §50 测试矩阵：每个守卫文件对应什么（机器从日志导出，不是手写计划）

**取数方式**：解析 `Himari/.local/ws-12.log`（`cargo test --workspace --no-fail-fast`，带
`HIMARI_SNI_GATE_BIN` + `HIMARI_UPSTREAMS_DIR`，2026-10-04 14:16）。表里每一行的数字都是
**跑过并报了 ok 的测试名**，不是计划；`ok/总数` 不相等即代表那轮有红。

合计 **48 个集成测试文件 / 325 条测试**。该轮汇总行 = **69 target / 325 passed / 0 failed / rc=0**（69 另含 lib 单测 target）。

| target 文件 | crate | 测试数 | ok | 该文件第一行注释（它自己说守什么） |
|---|---|---|---|---|
| `tests/automatic_mode.rs` | `himari-daemon` | 7 | 7 | §28's automatic mode, end to end through the service: measure, remember, |
| `tests/autostart_ipc.rs` | `himari-daemon` | 3 | 3 | §38 over the wire: the autostart switch has to reach launchd, and the answer |
| `tests/bridge_replay.rs` | `himari-ui` | 6 | 6 | The replay guard: what a client does with pushed transitions. |
| `tests/client_live.rs` | `himari-ui` | 1 | 1 | The UI's own client path, exercised against a live runtime service. |
| `tests/codec_guards.rs` | `himari-ipc` | 8 | 8 | Framing guards. Everything here runs on an in-memory cursor, so the codec's |
| `tests/controller_live.rs` | `himari-gate` | 9 | 9 | The controller's lifecycle, measured against the real sni-gate binary. |
| `tests/corpus_guards.rs` | `himari-daemon` | 7 | 7 | §43: Himari keeps its *own* corpus, built only from probes this service ran. |
| `tests/corpus_provenance.rs` | `himari-runtime` | 2 | 2 | §43 provenance gate: the candidate corpus may only ever be Himari's own |
| `tests/diagnostics_guards.rs` | `himari-ui` | 17 | 17 | §29's judging rules, tested headlessly. The page needs eyes; the verdicts do |
| `tests/diagnostics_rows.rs` | `himari-daemon` | 6 | 6 | §29 over the wire: the runtime service is the only process that can see the |
| `tests/dns_backend.rs` | `himari-runtime` | 4 | 4 | §13: name resolution is a backend Himari owns, and a failure inside it must |
| `tests/endpoint_config.rs` | `himari-daemon` | 6 | 6 | §9/§10/§11: the editable endpoint. The invariant worth guarding is not |
| `tests/endpoint_persistence.rs` | `himari-daemon` | 5 | 5 | §10/§53: the endpoint the user chose survives the run. The property worth |
| `tests/executor_effects.rs` | `himari-helper` | 8 | 8 | The privileged effects, exercised against the fake `networksetup` that |
| `tests/guardian_guards.rs` | `himari-daemon` | 3 | 3 | §36: a component whose process is gone must never keep reading "running". |
| `tests/helper_authorization.rs` | `himari-helper` | 9 | 9 | The security boundary of §20/§21, tested without a root process. |
| `tests/helper_roundtrip.rs` | `himari-helper` | 6 | 6 | The helper's loop over a real socket, with a counting stand-in for the |
| `tests/helper_startup.rs` | `himari-helper` | 9 | 9 | Startup refusals, and end-to-end runs of the real helper binary. |
| `tests/honest_gauges.rs` | `himari-daemon` | 3 | 3 | §44/§45/§12: a number the service cannot measure must not appear as measured. |
| `tests/ipc_e2e.rs` | `himari-daemon` | 6 | 6 | End-to-end over a real unix socket: framing, request handling, pushed state |
| `tests/journal_guards.rs` | `himari-platform` | 9 | 9 | §37 recovery guards. Each one checks bytes on disk, because "the write |
| `tests/lan_share_wiring.rs` | `himari-ui` | 4 | 4 | §11's switch, end to end: a user action must move the listener, not just |
| `tests/launch_item_guards.rs` | `himari-platform` | 10 | 10 | §38 guards, run against a fake `launchctl`. |
| `tests/layering_guards.rs` | `himari-ui` | 3 | 3 | §47/§48/§52 as a machine check, not a promise in a description line. |
| `tests/live_scan.rs` | `himari-runtime` | 6 | 6 | Live scanner guards. These reach the network (the same outbound TLS probes |
| `tests/login_item.rs` | `himari-daemon` | 4 | 4 | §38 through the service: the login item is installed and removed on request, |
| `tests/macos_bundle.rs` | `himari-daemon` | 5 | 5 | The macOS bundle contract (§40/§41), checked without shipping anything. |
| `tests/psl_source.rs` | `himari-gate` | 2 | 2 | §43/§46 boundary at the gateway: the public-suffix table the child process uses |
| `tests/readiness_impostor.rs` | `himari-gate` | 2 | 2 | §14's readiness gate, attacked rather than described. |
| `tests/render_guards.rs` | `himari-gate` | 7 | 7 |  |
| `tests/scan_guards.rs` | `himari-daemon` | 5 | 5 | §7 through the real socket: a scan started over IPC streams progress back, |
| `tests/service_resolution_guards.rs` | `himari-platform` | 7 | 7 | §23 resolvers, driven by output captured from a real Mac rather than by a |
| `tests/single_instance.rs` | `himari-ipc` | 4 | 4 | Single-instance guard (§39) and stale-socket handling. Unix only, because |
| `tests/sni_gate_accepts.rs` | `himari-gate` | 5 | 5 | Validates rendered config against the REAL sni-gate binary. |
| `tests/spoof_guards.rs` | `himari-runtime` | 6 | 6 | Guards for the upstream wrapper. Nothing here needs root: they test config |
| `tests/state_guards.rs` | `himari-core` | 15 | 15 | Guards from docs/audits/07 §1.4. Each one carries the positive control that |
| `tests/status_shapes.rs` | `himari-daemon` | 4 | 4 | §49's status methods, each with a shape that cannot lie by construction. |
| `tests/strategy_guards.rs` | `himari-strategy` | 16 | 16 | §27/§28 guards for the strategy engine. |
| `tests/structured_logs.rs` | `himari-daemon` | 9 | 9 | §34's log contract, tested where it is written: the ring, the fields, and the |
| `tests/supervisor_guards.rs` | `himari-gate` | 14 | 14 | Restart-policy guards (mechanisms M4 / M5 / §36). All pure: the clock is an |
| `tests/system_proxy_guards.rs` | `himari-platform` | 11 | 11 | §22 guards, run against a fake `networksetup` (tests/networksetup_stub.sh). |
| `tests/system_proxy_setting.rs` | `himari-daemon` | 5 | 5 | §22's switch, as a setting with a real reader. The claim worth guarding is not |
| `tests/system_proxy_wiring.rs` | `himari-daemon` | 5 | 5 | §22 wiring: the system proxy may only be touched while there is a listener |
| `tests/third_party_ledger.rs` | `himari-daemon` | 4 | 4 | §42: the license ledger is a gate, not a note. |
| `tests/ui_reduce.rs` | `himari-ui` | 17 | 17 | §50's UI row, tested headlessly. The window needs eyes; the behaviour does |
| `tests/uninstall_cleanup.rs` | `himari-daemon` | 4 | 4 | §40/§45: uninstall removes what the service made, restores what it changed, and |
| `tests/xray_binary.rs` | `himari-runtime` | 7 | 7 | §6: the version comes from the binary. These guards run fake executables, so |
| `tests/xray_shares.rs` | `himari-daemon` | 10 | 10 | §6: imported share links. The interesting property is not parsing -- the |

## §50 的行与尺的对照（诚实版）

**有尺的**：状态机 §5/§35（`himari-core` 单测 + `automatic_mode.rs`）；扫描有界并发 §7（`scan_guards.rs`、`structured_logs.rs`）；
端点与 LAN 曝光 §9–§11（`endpoint_config.rs`、`endpoint_persistence.rs`、`lan_share_wiring.rs`）；系统代理保存/还原 §22
（`system_proxy_setting.rs`，含跨重启持久 = Gate BB）；崩溃/异常退出恢复 §36/§37 与卸载 §40（`uninstall_cleanup.rs`，判据 `foreign(dir)==空`）；
登录项 §38（`autostart_ipc.rs`）；单实例 §39（`himari-ipc` lock 守卫）；结构化日志与脱敏 §34（读原始字节，磁盘不绕过）；
诊断的为什么 §29（`diagnostics_rows.rs`）；许可台账 §42（`third_party_ledger.rs`）；分层禁令 §47/§48（`layering_guards.rs`）；
§43 溯源三件套（`corpus_guards.rs`、`corpus_provenance.rs`、`psl_source.rs`）；§6 的链接解析与二进制探测（`xray_shares.rs`、`xray_binary.rs`）；
§13 DNS 后端（`dns_backend.rs`）。

**§50 目前没有尺的行**（这张表不是覆盖声明）：
- Xray 隧道真的起来：conf 生成、进程起停、all-in-one、日志收集 —— 导入的 share 链接仍无消费者；
- §12 出口证明（流量确实经过 Himari）与 §24 的 BPF 生命周期/重开 —— 被本机 `/dev/bpf` 权限边界挡住；
- §26/§27 分片/disorder/TLS-record（D-4 未拍板）；
- §5/§40 Windows 侧（D-7 WinDivert 许可、`windivert-sys` 的 host-only build script）；
- §41 签名/硬运行时/公证/entitlements 的产物侧；§30/§31 视觉验收（前端归 Flutter lane）；
- §7 的批量扫描与 Cloudflare 识别只覆盖了可在本机跑的那部分（DNS 判据 + 已知答案回读）。

> 口径：这张表由日志导出，**下一次全量跑完要重导一次**；手工维护它只会让它变成第二份会说谎的清单。
> 重导命令见 `docs/audits/09-session-handover-2026-10-04.md` 的 14:20 末态一节。
