# Himari 终端界面（`sni-gate-tui`）— **已搁置**

> **状态（2026-10-04）**：搁置。代码留着，但**默认不参与编译**——`src/lib.rs` 里整个模块是
> `#[cfg(feature = "tui")]`，`Cargo.toml` 里 1 个 bin + 3 个 test target 全是
> `required-features = ["tui"]`，CI 三步（fmt / clippy / test）都不带这个 feature。
> 门禁 `tests/tui_surfaces.rs::ci_does_not_build_the_shelved_ui` 会钉住这件事：
> 谁往 workflow 里塞 `--features tui`，测试就红。
>
> 要接着做：`cargo test --features tui && cargo build --features tui --bin sni-gate-tui`。
> 视觉语言规范在 `docs/design-language.md` §6（含它为什么长成这样、以及三条被实测推翻的口径）。

命名沿用本仓口径：产品叫 **Himari**，命令、配置文件、被监督的进程仍叫 **sni-gate**。

## 它是什么

终端里的前端，新粗野主义，**鼠标和键盘等权**。它**不内嵌网关**：把真正的 `sni-gate`
二进制作为子进程拉起、读它自己写的日志、报它的 pid / 退出码 / 终止信号。理由记在
`src/tui/gate.rs` 头上：钉住的上游核心没有计数器，进程边界是唯一能量出真东西的地方。

```sh
cargo build --features tui --bin sni-gate-tui
target/debug/sni-gate-tui --config sni-gate.example.toml
```

| 参数 | 作用 | 默认 |
|---|---|---|
| `--config <路径>` | 展示的配置，也是交给网关的 `--config` | `sni-gate.toml` |
| `--gate-bin <路径>` | 要监督的网关二进制 | PATH 上的 `sni-gate`，再退到同 `target/<profile>/` 的兄弟 |
| `--dark` | 起始深色板 | 浅色；进去后按钮或 `t` 随时切 |
| `--accent <#RRGGBB>` | 强调色，**只染投影**不染按钮面色；乱值直接报错退出 | 未设（accent 是项目待拍板项，§6.4） |
| `--tick-ms <n>` | 事件轮询间隔，同时是收割子进程与排日志的节奏（下限 20） | 200 |

不是 tty 时**拒绝启动**并说明原因（它要切备用屏、抓鼠标）。

## 鼠标 → 行为

| 动作 | 结果 |
|---|---|
| 左键按下 | 只进按压态：投影消失，面色滑进投影原来占的那 2×1 格 |
| 在同处松开 | 触发 |
| 在别处松开 | 取消（落在**另一个**控件上也不触发那个控件） |
| 移动 | hover：投影多推 1 格 |
| 面板内拖拽 | 滚动该面板，**与滚轮同向**、按增量算 |
| 滚轮上/下 | 滚**光标底下**那块面板，不是"当前聚焦"那块 |
| 滚轮左/右 | 横向滚列 |
| 右键 | 次级动作：配置节=其余全折叠；日志页=清屏（网关继续写） |
| 中键 | 跳到该面板的最新/末尾 |
| 双击 | 同一热区 400ms 内两次按下（crossterm 0.29 不报双击，自己判） |

热区登记在**未按压**的矩形上：按下瞬间画面上控件"跑"了 2 格，命中区不动，否则松开就丢。

方向口径：**"下"= 往最新 / 往文档末尾**。日志页存的是"距最新行几行"的**反向**偏移，
配置树和状态页存的是"从顶部跳过几行"的正向偏移，所以方向按面板查
（`app.rs::forward_increases` / `pane_start` / `pane_end`）：`End` 在日志页落到偏移 0，
`Home` 反而落到最大偏移。`tests/tui_mouse.rs` 里
`the_log_pane_and_the_text_panes_run_in_opposite_directions` 与
`end_and_home_mean_the_document_ends_not_the_number_zero` 钉这条——两边都只是 `usize`，
写反了编译器不会响。

## 键盘（等权通路，不是降级）

`Tab`/`Shift-Tab` 翻页 · `1..4` 直达 · `n`/`p` 在控件间移焦点（左侧 `◀` 是焦点标记）·
`Space`/`Enter` 触发焦点控件（**和点击共用同一个 `activate()`**）· `r` 次级动作 ·
`j/k`/`↑↓` 选行 · `d/u`/`PgDn/PgUp` 滚页 · `h/l`/`←→` 滚列 · `End` 跳末尾 · `Home` 跳顶 ·
`s` 启动 · `x` 关停 · `t` 换主题 · `q`/`Esc`/`Ctrl-C` 退出。

窗口小于 44×18 时不硬画，直说要多少格、以及为什么（投影与 3 行按钮要占那些格；
18 = 上面 13 行 + 底部投影与说明行 2 行 + 最小可画的卡片 3 行）。
动作按钮在一行里放不下时，**按名字报"哪几个被挤掉了"**，不静默消失。

## 四个页面

- **STATUS**：我们那个子进程的事实（状态 / pid / uptime / 退出方式分列 signal、code、
  是谁让它停的）、网关二进制与配置路径、以及**网关自己那份 parser** 读出来的有效配置
  （listener 数、route 数、CA 与 store/cache/PSL 的关键项、pools/resolvers/regexes/templates
  的名字）。另有一块 `WHAT IS NOT MEASURED`：连接数、握手数、字节数量不出来（上游核心无
  计数器，见 `docs/audits/09` Gate AO），所以写"量不出来"而不是给个漂亮的 0。
  这一页自己是个可滚动面板（`Hot::StatusPane`），溢出时最后一行写 `N above · +M below`。
- **LISTENERS**：点行选中，下面一块列出选中 listener 的 route 明细；`upstream` 省略时
  按 `config.rs` 文档写成 `(matched name, listener port)`，不编一个地址出来。
- **LOGS**：子进程 stdout/stderr 的真实行，带相对时间戳与来源管道（`tracing` 走 stderr）。
- **CONFIG**：文件原文按 TOML 表头分节的可折叠树，注释留在所属节里。

## 退出秩序

`Drop`：先 `SIGTERM` → 在 `STOP_GRACE=3s` 内轮询 → 没退再强杀并 `wait`，
所以不会把网关留在绑着端口的状态。鼠标抓取与光标隐藏也是 RAII，panic 路径同样还原。
`GateState`（我们的进程做了什么）与"端口有没有人接"**分开**，界面上永不合并成一个词。

## 已量到的（搁置前的最后一次实测，`--features tui`）

- `tests/tui_mouse.rs` 23 条、`tests/tui_visual_language.rs` 11 条、`tests/tui_surfaces.rs`
  7 条：**全绿，0 warning**（日志 `.local/g6.log`）。
- 真终端实测（PTY，96×32，`.local/pty_96.log`）**RC=0、23 项全过**：切备用屏、向终端要鼠标
  上报、SGR 坐标、藏光标、退出还原、按 `q` 退出不需 SIGKILL；**点击真的触发**（画面回读带写出
  `theme: dark`），**滚轮把折叠在画面下的块滚进画面**、溢出计数随之掉到底。
- 80×24 那一跑：DARK 与 QUIT 被挤掉，画面老实写了
  `2 action(s) hidden for lack of room: DARK, QUIT` ⇒ 探针因找不到可点目标而报"没点到东西"。
  这条**不是 bug 的证词**，是"挤掉要报名"这条设计确实生效的证词。
- `cargo check --locked`（默认 feature）rc=0；ratatui 不在默认依赖图里。

## 已知没做完的（不是"做完后搁置"，是本来就还差）

1. 80 列以下的控件排布（现在是"报名"而不是换行；换行我按"少写逻辑"的口径撤了）。
2. 状态页/日志页没有分页以外的滚动交互细化（拖条、点击滚动定位）。
3. CI 不跑这套门禁 = 它随时可能悄悄腐坏（这是"搁置"的代价，故意的）。
