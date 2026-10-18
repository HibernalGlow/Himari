# 特权 helper 的契约与边界（需求书 §20 / §21 / §24 / §37）

状态：**协议、启动自检、真实二进制端到端、以及 daemon 的选路都已落地并有守卫**（见 `docs/audits/08` 的 Gate V/W/X）；**尚未落地**的是 root 安装与一次性提权（§21）、代码签名校验（§41）、launchd 启动项（§38，方法在表里但明确回 `not wired`）、BPF 生命周期（§24）。下面写的是已经定死并被断言的边界，不是待办清单。

## 一、为什么必须有它（不是"更正规"，是现有代码已经撞到的墙）

`himari-platform::PlatformError::NotApplied` 是实测产物：本机以普通用户执行 `networksetup -setwebproxystate "Wi-Fi" off`（值本来就是 off）返回 rc=0，而 `preferences.plist` 的 mtime 与大小前后完全一致。**"非特权写入是否落地"没有被证明**——证明它需要改动用户真实的网络配置，那是 §22 明令禁止的拿用户机器试错。

所以 `point_at` 的口径是"写完必须读回，读不回报 `NotApplied`"，而 daemon 把它映射成 `ErrorCode::NeedsPrivilege`（`himari-daemon/src/lib.rs::proxy_error_code`）。**helper 就是这个错误码目前唯一的出口。** 没有它，阶段 4 的"打开即工作"（§53）不成立。

## 二、职责边界：helper 只做"OS 不肯让普通用户做的事"

必须进 helper（写操作，且需要提权）：

1. 系统代理写入（`networksetup -setwebproxy/-setsecurewebproxy/-setwebproxystate/-setsecurewebproxystate`）。**允许的 flag 以 `himari-platform/src/system_proxy.rs` 顶部注释里那份实测清单为准**，新增 flag 必须先实测再加，不得凭记忆写。
2. BPF 设备的开启与配置（§24 生命周期：open→configure→start→stop→close→reopen）。
3. launchd 标签的安装/卸载（§38 启动项），以及它的**卸载清理**（§19 的 uninstall cleanup）。
4. 未来 PF anchor / Windows 服务或 WinDivert 的装载（§25/§26）。

明确**不进** helper：任何策略决定（选哪条 SNI、要不要重试）、任何 UI 状态、任何网络 I/O、任何"记住上次哪个接口"。§20 的原文口径是 GUI 非特权、特权动作隔离在 helper 内；反过来同样重要——**helper 不是业务逻辑的垃圾桶**，否则它就变成了第二个 daemon，而它没有 daemon 的测试覆盖。

## 三、提权时机与鉴权

- §21：**安装时一次**，之后不再问密码。正常 UX 里不许出现 `sudo ./himari`。
- helper 由 launchd 以 root 常驻（`SMAppService`/`launchd.plist` 二选一，**这条要实测，不凭印象写死**），daemon（普通用户）经 Unix domain socket 调它。
- 鉴权靠 **`LOCAL_PEERCRED` 对端凭据 + 代码签名 Team ID**，socket 权限 `0600`，路径与 `himari_ipc::lock::default_runtime_dir()` 同源。理由：任何本地进程都能连上一个宽松的 socket，而 §20 说"之后受控 IPC 使用"——受控的意思就是**对端可验证**。
- **实测更正（读 `libc` Apple 定义得证）**：macOS 的 `LOCAL_PEERCRED` 返回的是 `xucred{cr_version, cr_uid, cr_ngroups, cr_groups[16]}`——**只有 uid 和组，没有 pid**（Linux 的 `sockpeercred` 才有 pid）。所以"哪个进程连上来了"**不可能**靠这个调用得到，`himari_helper::Peer.pid` 恒为 `None` 并有断言钉住（哪天变成 Some 就是平台行为变了，测试会红）。可验证的是**权限身份（uid）**；**程序身份只能由代码签名承担**（§41），即 `SecCodeCheckValidity` 那条路，本 crate 尚未引入。
- **每个方法都必须带幂等键（generation）**：helper 收到重复的 enable 不许再写一遍；`set_proxy_off` 之后不许有"顺手清空代理"的路径（§22）。这条与 `himari-core::state` 的 generation 语义一致，可直接复用。
- helper **不缓存 §23 的答案**：它只能被问"现在哪个服务在带路由"（`himari-platform::interfaces::resolve`），答案不许持久化。已经存在的双向守卫是 `Daemon::current_network_service()`（同一实例、不重启，只改路由状态就必须变答案）。

## 四、恢复与卸载（§37 / §19）

- helper 干的**任何**持久变更都必须先经 `himari_platform::Ledger`：写前落盘、`undo_kind(kind)` 支持按功能回滚（已存在），异常退出后 `Daemon::recover_after_crash()` 只回滚未完成项。
- 卸载顺序：helper 先恢复它改过的一切（系统代理 / BPF / PF / launchd 标签），再**最后**删自己；删自己之前留一条可复核的账本尾迹，否则 §37 的覆盖面在卸载那一步归零。
- sni-gate 的 CA **没有卸载 API**（`docs/audits/02-gate-sni-gate.md` 实测），且指纹跨平台不一致（Windows 用 SHA-1、其余 SHA-256），所以"信任锚清理"不能靠指纹当键——这是 §36/§37 的一个已知未解项，helper 规格里先显式记账，不要留给实现时 surprise。

## 五、开工时的第一件实测（按本仓库惯例：先量再写）

1. 用**必然被拒的输入**自检管路（例如向 helper 要一个未注册的方法名，必须拿到明确拒绝），再测正向路径——与 `feedback-destructive-live-tests-selfcheck` 同口径。
2. `SMAppService.register` 在本机（arm64 / macOS 27 beta / SIP 开）到底需要 `developer tools` entitlement 还是明文签名即可：**现读现测**，别照抄文章。
3. 非特权 daemon → root helper 的对端凭据读取，必须配一条**阳性对照**（伪造对端身份必须失败），否则"我检查了 peer uid"只是自我声明。
4. 阈值/超时也要先量（`sni-gate` 的 SIGTERM 就绪窗口那次就是这么翻车的：立即 TERM = 信号 15 死，settle≥1s 才 exit 0）。

## 六、这份文档不承诺的事

- 不承诺 helper 的 API 形状（等 §49 方法表决定 D-11 再定，避免为一个字段扩 `get_status` 还是新增只读方法而先写死一半契约）。
- 不含 Windows 侧（WinDivert 的 LGPL-3.0 / 商业双授许可决定 **D-7** 未拍板，且它的 crate 已经在 `Cargo.lock` 里——见 `THIRD-PARTY.md`）。
- 不含任何"已验证"字样：这里所有事实只有两类，**已在代码里落地的**（给了符号名）与**待实测的**（给了清单）。
