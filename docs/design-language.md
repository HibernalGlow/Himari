# Himari 视觉语言 — Neobrutalism

来源：<https://www.skillsui.app/skills/neobrutalism>（记录日期 2026-10-03）
适用范围：Himari 的 Iced GUI。上游 sni-gate 的 `README.md` 不属于本规范对象。

---

## 1. 三条主张

- **Honesty over decoration** — 界面元素直接暴露结构，不靠装饰遮掩。
- **Contrast does all the work** — 层级由对比产生，不由阴影/透明度/留白暗示产生。
- **Personality is a feature** — 明确拒绝「默认企业后台」长相。

## 2. 规范条目（逐条来自来源）

### 配色
- 基线：**cream + black + 一个 bold 主色**（下限，不是上限）。
- 至少一个强 accent 色。
- 反例：**只有纯黑纯白、没有彩色，会被读成「没做完」**。

### 描边
- 粗边框（heavy borders）。
- 只加在**可交互元素**上：card / button / input。
- **文字段落不要加框**。
- 陷阱：border overload — 全都加框等于没有重点。

### 阴影
- 粗偏移阴影（bold offset shadows），制造物理纵深。
- 按压态：**阴影消失 + 元素位移**。

### 字体
- grotesque / condensed 无衬线，不带「软」感。
- 避免：圆润友好的 sans。
- 排版不为自己的体积道歉（大字号、直接）。

### 圆角与间距
- 锐角为主（"stop hiding the structure"）。
- 粗边框要求非交互文本有更多呼吸空间。

### 动效
- 必须有：click 时阴影消失 + 位移。缺了它「按钮看起来很怪」。

### 禁用场景（来源明列）
- 不适合以「可信/正式」为首要信号的企业场景。
- **不适合密集数据表**：粗边框会让人 overwhelmed。
- 不用 soft transition、不用 gentle gradient。

## 3. 与 Himari 需求书 §32 的冲突与调和（重要）

需求书 §32 要求：「现代、干净、低噪音、接近现代系统应用」。Neobrutalism 是**高对比、粗描边、有性格**的路线，两者不能同时在全屏最大化。

调和口径（按下述执行，不要两边各打五折）：

| 界面部位 | 采用 | 理由 |
|---|---|---|
| 首页状态卡、Start/Turn Off 主按钮、策略选择、section card | **Neobrutalism** | 可交互元素本来就是风格主战场 |
| Scanner 结果表、Logs 视图、Advanced 配置树 | **收敛版**：保留描边/字重层级，去掉逐项粗边框 | 来源自己禁止密集表格；§32 也要低噪音 —— 此处两者结论一致 |
| 状态指示（Running / Failed / Degraded） | 用 accent + 成功/失败色，靠**颜色对比**而非图标装饰 | §35「GUI 只渲染状态」 |
| 深浅色 | 两套都要做（§32 light/dark + 系统主题检测） | 现有 sni-spoofing-rust UI 已有 light/dark 双 palette，行为对等基线 |

未定项（需要用户拍，不要替他定）：**accent 具体色值**；是否保留 cream 底色于 dark theme。

## 4. 落地约束（已实测，详见 `audits/06-toolchain-and-crates.md`）

- 目标 GUI = **iced 0.14.0**（当前稳定版，MIT，edition 2024，MSRV 1.88）。
- iced 0.14 的 `Palette` 只有 6 个扁平色：`background / text / primary / success / warning / danger`；`Extended` 由 `Theme::custom_with_fn` 自定义生成。
- **`Palette` 里没有 border 色，也没有 shadow 色**（逐字检查 `iced_core-0.14.0/src/theme/palette.rs`，`border` 0 命中）。
  ⇒ 本风格的**粗边框与偏移阴影必须在 widget style 层实现**，不能指望调色板下发；Himari 自己维护一层 token（ink / surface / accent / offset-shadow / border-width）。
- 系统主题检测**可用且是事件驱动**（`iced_winit` 有 `ThemeChanged` 事件）→ 满足需求书 §52「不要轮询刷状态」，不要写成定时器。
- 新增门禁：必须有测试断言 **card 面色 ≠ 窗口底色**，否则 cream-on-cream 会让对比消失，直接违反「contrast does all the work」。

## 5. 仍未定（需用户拍，不要替他定）

- **accent 具体色值**；默认先留 TODO。
- **dark theme 是否保留 cream 底**（还是 dark 用中性深灰 + 同一 accent）。终端版按现状跟进：dark 用 `DARK_SURFACE`，不保留 cream（`himari-ui/src/tokens.rs` 已这么做了，§6.3 记录的是另一处偏离）。
- 字体选型：来源要求 grotesque/condensed，需确认跨 macOS/Windows 都能随包分发且许可证允许的字体（当前未选定，属未验证项）。终端侧的同一问题见 §6.1「字体」行——迁移不过去，只剩字重。

## 6. 终端版（`sni-gate-tui`，ratatui）

同一个视觉语言的第二个前端：**新粗野主义 + 鼠标交互**，落在 `src/tui/`（整个 `tui` feature，默认构建不链 ratatui，上游核心一个字节都不动）。颜色值真身是 `crates/himari-ui/src/tokens.rs`，由 `tests/tui_visual_language.rs::terminal_palette_values_are_the_gui_palette` 钉住——GUI 调色板一动，这边就红，两个前端不许各长一套色。

### 6.1 GUI 规则 → 单元格规则的映射

| 规范条目 | GUI（iced） | 终端（ratatui） | 为什么这么折 |
|---|---|---|---|
| 粗边框 | `Border{width:2.0}` | **实心 1 格环**：整块填边框色，再把内缩 1 格的矩形填面色（`paint::frame_ring`） | 终端没有 2px 边；箱线字形太"细"且依赖字体。实心色块在同样格预算下最重 |
| 偏移阴影 | `Shadow{offset:3px, blur:0}` | **右 2 格 × 下 1 格的 L 形色带**（`paint::shadow`），`SHADOW_COLS=2 / SHADOW_ROWS=1` | 单元格高≈宽的 2 倍，2×1 是网格上最近的"方形偏移"；2×2 会把阴影挤出窄窗口 |
| 按压态 | 阴影消失 + `snap` 位移 | 阴影**不画**，面色矩形整体画到 `(x+2, y+1)`——即"按钮滑进自己原来的影子里" | 物理读法一致：抬起=远，按下=贴平 |
| hover | （GUI 未定义） | 阴影多推 1 格（`HOVER_LIFT_COLS`） | 鼠标必须有回读路径；用影宽表达"离你更近"，不引新颜色 |
| 字体 grotesque/condensed | 自选字体 | 只剩字重：所有绘制 style 一律 `Modifier::BOLD`，由 `every_painted_style_is_bold` 钉住 | 终端字体由用户决定，这条规范能迁移的部分只有重量 |
| 锐角 | `radius:0` | 天然满足；**禁用 `BorderType::Rounded`** | — |
| 不加框的段落 | 文本区 | `prose_style` 用页面底色、不画环 | 规范原文：文字段落不要加框 |
| 密集表格不适用 | 收敛版 | Scanner/Logs/Config 行**不逐行画环**，选中=整行反色带，hover=左侧标记列 | 与 §3 的收敛版结论一致 |
| 深浅双套 | `theme(dark)` | `Palette::new(dark, accent)`，两套都要过对比门禁 | — |

### 6.2 对比度这把尺

`tokens::contrast()` 用 WCAG 相对亮度，阈值 4.5（正文级）。它不是装饰：

- `card ≠ page` 是 §4 那条门禁的加强版——不只要求不相等，要求 `distinguishable(card, page)` 在两套主题都成立；
- 尺自己也要有阳性对照：同一对色自比必须判为"不可区分"，测试里写了这一条。

### 6.3 一处**记录在案的**偏离，不是复制错误

`crates/himari-ui/src/tokens.rs::frame()` 两套主题都返回 `INK`。`#111111` 画在 `#1A1917` 上对比度约 1.1:1，等于没有边框——在"对比产生层级"的语言里这是自相矛盾。终端版在深色主题把边框/阴影色取 `DARK_TEXT`，并由 `dark_frame_colour_is_visible_where_the_ones_ink_would_not_be` 同时钉两件事：**GUI 那个组合必须量出来 < 4.5**（前提一旦变，测试先红），以及终端选的色必须 ≥ 4.5。

> 同一条结论对 GUI 侧也成立：那应该改，但 §6 不改别人的代码，只把这个不对称写清楚。

### 6.4 accent 仍未拍板

§3/§5 说 accent 由用户拍，终端版**不替他定**：`--accent <#RRGGBB>` 是这个旋钮，默认 `None`，`None` 时按钮面色就是 §6.1 表里的黑块（与现 GUI 行为对等）。`accent_is_parsed_only_when_it_is_a_real_hex_triplet` 保证乱值不会静默变成一个颜色。

### 6.5 鼠标交互的口径

`EnableMouseCapture` 由 TUI 自己开、自己关（`Drop` 里还原，异常路径也还原）。事件语义照真 GUI 走，不是"点到就动"：

- `Down` 只进按压态（阴影消失、面色位移），`Up` 落回同一个热区才**触发**；落在别处=取消。
- `Moved` 驱动 hover；`Drag` 在视口里=拖动滚动；`ScrollUp/Down/Left/Right` 滚**光标底下那个**视口，不是"当前聚焦的那个"——这是鼠标该起的作用。
- 双击 crossterm 0.29 不给，按"同一热区 400ms 内两次 Down"自己判。
- 热区**登记在未按压的那个矩形**上；面色位移只改绘制，不改命中。否则按下瞬间控件"跑掉"，release 就丢了。
- 右键=次级动作（折叠/展开整节），中键=滚回底部，都写进页脚那行说明，不做隐藏手势。

键盘通路必须与鼠标**等价**（`activate(id)` 是唯一入口，鼠标和按键都走它），否则 §30/§31 的验收在没有鼠标的终端里直接不可测。

### 6.6 三条实测把规范本身改了（第一次编译，2026-10-04）

`cargo test --features tui` 第一次真跑起来时，`tests/tui_visual_language.rs` 有 3 条红。结论是**规范里有三处把阈值套错了对象**，不是代码凑不出绿色：

| 原来断言的 | 实测 | 为什么错 | 现在的口径 |
|---|---|---|---|
| card 面色 vs page 底色 ≥ 4.5:1 | **1.13:1**（cream `#F4F1EA` vs 白 `#FFFFFF`） | 4.5 是**文本**级；GUI 那侧同一对色也是 1.13，靠 2px 边框撑 | 面色只要求**不相等**（§4 原文）；**边界**改用非文本对比 ≥3:1（`tokens::boundary`），量 frame↔card 与 frame↔page |
| 状态色上放 cream 字（"深底配亮字"） | WARNING 上 cream = **3.11:1** | 别处都用 4.5，唯独 pill 达不到 | `text_on_status()` 改成**在 INK/CREAM 里量最大值**（WARNING→INK 4.83:1）；测试同时钉住"cream 在琥珀上必须不合格"，这条捷径再也回不来 |
| hover 抬影"多 1 格" | 断言坐标写歪 1 格，阳性对照**用了同一个坐标** | 该条不可能红（假对照）；实测 `left=None right=Some(INK)` 才暴露 | 两侧夹住：带内最后一格有色 + 带外第一格无色。抬影 0 格和 2 格都会红 |

**留给用户的一件事（不替你定）**：cream + 白卡 = 1.13:1 说明**面本身不携带层级**，层级完全由实心边框带承担。终端里没问题（我画的就是 1 格实色环）；GUI 侧若哪天去掉边框，卡会糊进页面。要么接受这层依赖，要么给 card 换一个能自己站住的面色——`card_face_differs_from_page_and_the_frame_carries_the_boundary` 里那条 `fill_gap < 1.5` 就是这件事的哨兵：值一动这条测试会叫，不会静默。

### 6.7 证据等级（截至同一轮）

**已经是实测**：

- `tests/tui_mouse.rs`：**23 条全绿（最后一次实测 `.local/g6.log`：23+7+11=41 条、0 warning）**。压放同热区才触发、release 落别处取消、release 落在另一个控件也不触发那个控件、双击 399/400 两侧、拖拽增量且只在按下的那块区域内、滚轮命中"光标底下"而非"当前聚焦"、横滚有方向、右键/中键语义且都不进按压态、空矩形永不可命中、**按压时热区不随面色位移**（新覆盖格不可命中 + 原矩形让出的格仍可命中，双向）、`MouseCmd` 每个 variant 都有生产者（`tag()` 穷尽 match，加变体不写 tag 就编译不过）。
- `cargo check --locked`（默认 feature）rc=0 ⇒ **网关本体的构建没被牵连**；`Cargo.lock` 与 manifest 已一致（`sni-gate` stanza 不再列 `crossterm`，实测读回该行已消失）。
- lib + 两个测试目标 **0 warning**（ratatui 0.30 把 `Buffer::get` 标了 deprecated，测试层换成 `Buffer::cell((x, y))` 后清零）。

**还没是实测**：

- `tests/tui_visual_language.rs` 11 条：**11 条全绿**（§6.6 那 3 条改完口径后已复跑）；构建锁曾连续被六个其他会话占用，等待记录在 `.local/`。
- **`sni-gate-tui` 已链接**（`cargo build --features tui --bin sni-gate-tui` rc=0），并在真 PTY 里跑过：96×32 **23 项全过 / RC=0** —— 备用屏、鼠标上报、SGR 坐标、藏光标、退出还原、按 `q` 不需 SIGKILL；**点击真的触发**（回读带写出 `theme: dark`）、**滚轮把折叠在画面下的块滚进画面**。80×24 那跑报 `2 action(s) hidden for lack of room: DARK, QUIT`，是「挤掉必须报名」生效的证词不是 bug。
- 鼠标在真终端里的一次端到端注入（SGR 序列 → 画面变化）还没做。

### 6.8 落点清单

`src/tui/`（全部在 `tui` feature 内，上游文件零改动）：

| 文件 | 职责 |
|---|---|
| `tokens.rs` | 色值（与 iced tokens 同值，测试钉住）、影/压几何、状态色、`contrast`/`distinguishable`/`boundary` 三把尺 |
| `paint.rs` | 实心环、L 形偏移影、裁剪文本、宽字符推进 |
| `input.rs` | 热区表（后注册者在上）、`route_mouse` 全事件语义、`route_key` + `KeyCmd`（每个鼠标命令都有键盘孪生）|
| `widgets.rs` | `Button`（primary/secondary、按压位移、hover 抬影、accent 只染影）、`Card`（标题走边框带）、`status_band`（非交互⇒无框）、`paint_row`（收敛版行）、`paint_prose`、`paint_too_small`、`help_line`、`paint_tabs` |
| `gate.rs` | 子进程监督：TERM 优先 + `STOP_GRACE=3s` 后升级强杀、`ExitInfo` 分列 signal/code/谁让它停的、两根管道 reader 线程、`GateState` 与 `Probe` 两件事不合并、`Drop` 不留绑着端口的孤儿 |
| `snapshot.rs` | `ConfigFacts`（走网关**同一份 parser**）与 `FileTree`（文件原文分节、注释留在所属节）；不调 `load_or_generate`（开状态屏不许写盘） |
| `app.rs` | 状态机；`activate()` 是鼠标与键盘的**同一个入口**；每个效果写一行回读进 `message`；`SETTLE=1s` 用实测窗口区分 STARTING/RUNNING；`status()` 的颜色与 `status_label()` 的文字分开来源 |
| `view.rs` | 帧组合：手算 band（每层框元素下面预留影行、右边预留抬影列），四个页面，焦点 caret，太小时直说不画 |
| `mod.rs` | `run()` 事件循环 + `MouseCapture` RAII（含 panic 路径还原鼠标/光标）；非 tty 直接拒绝启动 |
| `src/bin/sni-gate-tui.rs` | 命令行：`--config/--gate-bin/--dark/--accent/--tick-ms`；`--accent` 乱值**报错退出**，不静默回退 |

### 6.9 搁置状态与恢复顺序

**2026-10-04 决定搁置**。代码留着，默认**不参与编译**：`src/lib.rs` 里 `#[cfg(feature = "tui")] pub mod tui;`；`Cargo.toml` 里 1 个 bin + 3 个 test target 全标 `required-features = ["tui"]`；ratatui 是 optional；CI 三步都不带这个 feature。实测 `cargo check --locked --all-targets` rc=0 = 默认路径确实一行都不编它。

反向哨兵：`tests/tui_surfaces.rs::ci_does_not_build_the_shelved_ui` —— 谁把 `--features tui` 塞回 workflow 谁就红，因为那是「重新开工」的决定，得说出来。恢复顺序与用法在 `docs/tui.md`。
4. ~~CI 补 `--features tui`~~ —— 按搁置口径撤回。代价：这套门禁 CI 不跑，会腐；重开时第一件事是把它接回 CI。

已实测的 lock 事实：加 ratatui 带来 +91 个 package（241→332），**既有版本一个都没被降/删**，多出 9 组共存版本（`syn 1.0/3.0`、`thiserror 1.0`、`rand 0.8`、`hashbrown 0.16`、`getrandom 0.3`、`r-efi 5.3`、`bit-vec 0.6`）——两套 proc-macro 栈是 ratatui 树的既成事实，别误读成谁降了版。
