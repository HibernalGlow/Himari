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
- **dark theme 是否保留 cream 底**（还是 dark 用中性深灰 + 同一 accent）。
- 字体选型：来源要求 grotesque/condensed，需确认跨 macOS/Windows 都能随包分发且许可证允许的字体（当前未选定，属未验证项）。

