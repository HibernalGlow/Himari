# §3 / §4 / §42 — 工具链与依赖事实（全部实测，非记忆）

取证方式：直接下载 crates.io `.crate` 源码包并读其 `Cargo.toml` / 源文件；本机 `cargo -V` / `rustc -V`。
缓存留在 `/Users/glow/Base/Code/Prometheus/.hibernal-experiment/crates/`（不在 Himari 仓内，不参与提交）。

## 1. 本机基线

```
cargo 1.98.1 (Homebrew)   rustc 1.98.1   host aarch64-apple-darwin
arch arm64                macOS SDK 27.0
```

→ 需求书 §19「Apple Silicon 原生」在本机即默认目标三元组 `aarch64-apple-darwin`，无需交叉。

## 2. GUI 框架版本（§3）

| 事实 | 值 | 来源 |
|---|---|---|
| iced `max_stable_version` | **0.14.0** | crates.io API |
| iced `newest_version` | 0.14.0（**0.14.0 已正式发布**） | 同上 |
| 前一稳定线 | 0.13.1 / 0.13.0 | 同上 |
| license | **MIT** | `iced-0.14.0/Cargo.toml` |
| edition / MSRV | **edition 2024 / rust-version 1.88** | 同上 |

**对需求书的一处更正**：§3 写「使用当前稳定版 Iced，不要依赖未来 0.15-dev API」。该条写就时 0.14 还在开发中；**现在的稳定版就是 0.14.0**，因此 Himari 钉 `0.14`，把「不许依赖 0.15-dev」原样保留即可，不需要退回 0.13。

**MSRV 巧合但要记住**：`sni-gate` 要 `rust-version = 1.88`，`iced 0.14` 也要 **1.88**，本机 1.98.1 同时满足。⇒ **Himari 的 workspace MSRV 下限 = 1.88**，写进 `Cargo.toml` 而不是靠 README。

## 3. Iced 0.14 的 theme API（这决定了 neobrutalism 怎么落地）

`iced_core-0.14.0/src/theme/palette.rs:9-22` — **`Palette` 是 6 个扁平色**：

```rust
background, text, primary, success, warning, danger
```

`theme/palette.rs:288-303` — `Extended` 是由 `Palette` **生成**的扩展集：
`background: Background / primary / secondary / success / warning / danger / is_dark: bool`。
`theme/palette.rs:450-…` — `Background` 有 8 档：`base, weakest, weaker, weak, neutral, strong, stronger, strongest`（每档是一个 `Pair`）。

入口（`iced_core-0.14.0/src/theme.rs`）：

| API | 行 | 用途 |
|---|---|---|
| `Theme::custom(name, palette)` | `:91` | 用 6 色定义主题，Extended 走默认 `generate` |
| `Theme::custom_with_fn(name, palette, generate)` | `:100` | **自定义 Extended 生成器** |
| `Theme::palette()` / `extended_palette()` | `:109 / :138` | 读回 |
| `theme::default(&Theme) -> Style` | `:329` | 取 widget 样式 |

### ⚠️ 关键约束：`Palette` 里**没有 border 色，也没有 shadow 色**

已逐字检查 `palette.rs`：`rg border|Border` 在该文件 **0 命中**。

后果（直接影响 §32 与 `docs/design-language.md`）：neobrutalism 的**粗边框与偏移阴影不能靠调色板下发**，必须在 **widget style 层**实现——`button::Style` 的 `border_width/border_color` 与 `shadow_*`（`iced_core` 确有 `border`、`padding`、`gradient`、`animation` 模块可搭配）。

⇒ Himari 需要**一层自己的 token**（例如 `ui/theme/tokens.rs` 定义 ink / surface / accent / offset-shadow 及边框宽度），由 `custom_with_fn` + 自定义 widget style 共同消费。**不要**把这层 token 假装成 iced 的 palette 字段。

`Extended::generate` 是从 6 色**算出** 8 档背景的，所以「cream 底 + 更深的 card 面」这类层级要靠：
(i) 把 cream 设为 `Palette::background`，再让自定义 generator 产出不被默认算法压扁的 `Background.stronger/strongest`；或
(ii) 直接在 token 层给 card 指定面色。**推荐 (i)+(ii) 混合，并要求有测试断言 card 面色 ≠ 窗口面色**（否则对比消失，违反该风格「contrast does all the work」）。

## 4. §4 要求的平台能力：候选 crate（license 逐个从各自 `Cargo.toml` 读出）

| 需求 | crate | 版本 | license | 备注 |
|---|---|---|---|---|
| 系统托盘（§33） | `tray-icon` | 0.26.0 | **MIT OR Apache-2.0** | tauri 生态；与 iced/winit 事件循环的接线**未验证** |
| macOS 菜单栏 / 菜单 | `muda` | 0.21.0 | **Apache-2.0 OR MIT** | 同上 |
| 通知 | `notify-rust` | 4.18.1 | **MIT OR Apache-2.0** | Linux 走 DBus；macOS 需 bundle 内通知授权 |
| 原生文件对话框 | `rfd` | 0.17.2 | **MIT** | |
| Windows 服务 | `windows-service` | 0.8.1 | **MIT OR Apache-2.0** | §26/§38 的 service 侧 |

全部 permissive，**与 §42 的再分发要求相容**（仍需保留 copyright/notice 声明）。

**未被证实的一项（不要写成事实）**：托盘/菜单栏要在 iced 的 winit 事件循环里跑，两边都要拥有事件循环。这是**已知的集成风险点，尚无实测结论** → §51 Phase 3 第一件事应是**一个最小 spike**：同一个 `main` 里起 iced 应用 + `tray-icon`，验证 tray 回调能变成 iced `Message`。spike 不过就要退回「菜单/托盘由独立线程 + 通道驱动」的方案。

系统主题检测**已证实可用**：`iced_winit-0.14.0/src/lib.rs` 中 `ThemeChanged` 出现 **5 次**（含 `subscription::Event::ThemeChanged`）⇒ §32 的「system theme detection」在 0.14 是**事件驱动**的，不是轮询。这同时满足 §52「不要靠轮询疯狂刷新状态」。

## 5. §42 license 台账的落点

Himari 需要一份 `THIRD-PARTY.md`，条目为 `name / version / license / provenance(commit pin) / 用途 / 是否随包分发`。
覆盖对象至少：`sni-spoofing-rust`(MIT)、`sni-gate`(MIT OR Apache-2.0)、`iced` 及其 `iced_*` 子 crate(均 MIT)、上表 5 个平台 crate、以及 **Xray / WinDivert / Wintun 三个第三方二进制**。

⚠️ 唯一有实质风险的是 **WinDivert（LGPLv3 / 商业双授权）**：不能想当然把 `.sys/.dll` 装进安装包。见 `00-index-and-open-decisions.md` D-7（默认口径：不捆绑，首次运行下载并校验哈希）。

## 6. 上游可复用的公开 API 面（§51 Phase 2 的真实起点）

`sni-spoofing-rust/src/lib.rs` 全文只有 14 行，逐字如下：

```rust
pub mod config;  pub mod error;  mod handler;  mod listener;
pub mod packet;  mod proto;      pub mod proxy; mod relay;
pub mod scan;    mod shutdown;   mod sniffer;   pub mod xray;
pub use proxy::{platform_privilege_hint, run_proxy_until_signal, start_proxy, RunningProxy};
```

⇒ **对外可用**：`config`（含 `validate`、自环检测）、`start_proxy` + `RunningProxy{request_stop,wait,stop,is_running}`、`run_proxy_until_signal`、`platform_privilege_hint`、`scan`（`scan_all`/`default_snis`/`ProbeOutcome`）、`xray`（分享链接解析）、`packet`（CH 构造/SNI 解析）、`error`。

⇒ **对外不可用**（私有 mod）：`sniffer`（含 `RawBackend` 与三平台 backend）、`proto`、`handler`、`listener`、`relay`、`shutdown`。

结论：**Phase 2 的「把现有功能通过正式 API 暴露给 Himari」今天就能起步**——`start_proxy` 已够撑起「start/stop/restart/status」与全部代理功能；但 **§25/§26/§27（TrafficBackend / Strategy Engine）确实被 A18 卡住**，必须走 D-4（向上游放开 `sniffer`/`proto` 可见性）。这条区分很重要：**不要被 D-4 拖住 Phase 2 的其余部分**。
