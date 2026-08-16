# springcat-ai-v1 实现步骤

> 文档版本：v1.0  
> 日期：2026-08-13  
> 对应规格：[项目描述.md](./项目描述.md)  
> 产品名：`springcat-ai`  
> 本版目标：按步骤实现全部规划功能；**先做工作模式，宠物模式后置**

本文是实现手册，不是产品愿景文档。每完成一步，勾选该步的任务和验收，再进入下一步。不要跳步，尤其不要在工作模式跑通前做 Rive / 宠物形象。

---

## 0. 怎么用这份文档

1. 从上到下按 **S00 → S16** 实施。`S00–S09` 是工作模式 MVP，`S10–S14` 是 v1.0，`S15–S16` 是后续候选。
2. 每一步都有：目标、涉及文件、任务清单、验收标准、禁止事项。
3. 一步没通过验收，不要开始下一步。
4. 每步结束记录四项数据：安装包大小、空闲内存、空闲 CPU、事件到 UI 延迟。
5. 规格与本文冲突时，以 [项目描述.md](./项目描述.md) 的产品约束为准，以本文的实施顺序为准。

### 全局禁止

- 不在 S09 完成前引入 Rive、`.riv`、宠物皮肤或宠物窗口。
- 不把 Codex / Cursor / Grok 的原始字段直接传给 UI。
- 不轮询聊天窗口、不截屏识别、不开放本地 HTTP 端口。
- 不创建全屏透明窗口，不使用挡住整屏的模态遮罩。
- 不默认创建三个长期 WebView。
- 不保存完整对话、源代码、API Key、Cookie。
- 不在规格里锁死会过期的依赖版本号；以开始实施时的稳定版 + 锁文件为准。

### 技术栈（实施时锁定稳定版）

```text
桌面外壳        Tauri 2
前端            Svelte 5 + TypeScript + Vite
工作面板        HTML / CSS
宠物动画        Rive Canvas Lite（仅 S10+）
本地核心        Rust
存储            SQLite
文件监听        notify
日志            tracing + rolling file
测试            Vitest + Playwright + cargo test
```

---

## 1. 功能总表

按实现顺序编号。状态列在实施时勾选：`未开始` / `进行中` / `已完成`。

### 1.1 工作模式 MVP（必须先做完）

| ID | 功能 | 步骤 | 状态 |
|---|---|---|---|
| F01 | 创建 Tauri 2 + Svelte 5 工程、锁文件、目录骨架 | S00 | 已完成 |
| F02 | 领域模型：`TaskEvent` / `TaskItem` / `SurfaceState` / `AppSettings` | S01 | 已完成 |
| F03 | 工作面板三态布局：收起 / 探出摘要 / 展开列表 | S02 | 已完成 |
| F04 | 五种业务状态视觉：idle / working / waiting / completed / failed | S02 | 已完成 |
| F05 | 透明无边框常驻窗口、置顶、不进任务栏（Windows） | S03 | 已完成 |
| F06 | 系统托盘与托盘菜单 | S03 | 已完成 |
| F07 | 拖动窗口 | S04 | 已完成 |
| F08 | 吸附到顶部 / 左侧 / 右侧，避开任务栏 | S04 | 已完成 |
| F09 | 吸附位置记忆（按显示器） | S04 | 已完成 |
| F10 | 鼠标靠近探出、离开后延迟收起 | S04 | 已完成 |
| F11 | 单击展开/收起任务列表；Esc 关闭 | S05 | 已完成 |
| F12 | 任务列表：来源、标题、状态、开始/完成时间、未读 | S05 | 已完成 |
| F13 | 右键快捷菜单 | S05 | 已完成 |
| F14 | `springcat-bridge` 命令行桥接 | S06 | 已完成 |
| F15 | 本地 inbox 文件事件通道（一事件一文件） | S06 | 已完成 |
| F16 | 事件规范化与未知字段兼容 | S06 | 已完成 |
| F17 | SQLite 任务仓库；历史默认 7 天 | S06 | 已完成 |
| F18 | 重复 / 乱序 / 损坏事件容错 | S06 | 已完成 |
| F19 | 面板消费 `SurfaceState`，事件到 UI < 500 ms | S06 | 已完成 |
| F20 | Codex CLI notify / hooks 适配器 | S07 | 已完成 |
| F21 | Cursor hooks 适配器 | S07 | 已完成 |
| F22 | Grok CLI hooks 适配器 | S07 | 已完成 |
| F23 | 适配器隔离：一个失败不影响其他 | S07 | 已完成 |
| F24 | 通知优先级：waiting > failed > completed-unread > running > idle | S08 | 已完成 |
| F25 | 执行中不弹摘要；完成约 5 秒收起；等待/失败不自动消失 | S08 | 已完成 |
| F26 | 短时间多任务完成合并为“N 个任务已完成” | S08 | 已完成 |
| F27 | 未读点；点击标记已读 | S08 | 已完成 |
| F28 | 来源跳转（先打开来源应用，有 deepLink 则优先） | S08 | 已完成 |
| F29 | 静音 1 小时 | S08 | 已完成 |
| F30 | 专注模式 | S08 | 已完成 |
| F31 | 独立设置窗口（按需创建，关闭销毁） | S09 | 已完成 |
| F32 | 开机启动开关 | S09 | 已完成 |
| F33 | 置顶开关 | S09 | 已完成 |
| F34 | 历史保留时长：1 / 7 / 30 天 / 不保存 | S09 | 已完成 |
| F35 | 展示模式选项占位（宠物模式显示即将推出） | S09 | 已完成 |
| F36 | 减少动态效果 | S09 | 已完成 |
| F37 | Windows 高 DPI（125% / 150% / 200%）位置正确 | S09 | 已完成（逻辑像素；需本机缩放实测） |
| F38 | Windows EXE / MSI 打包 | S09 | 已完成 |

### 1.2 v1.0（工作模式稳定后再做）

| ID | 功能 | 步骤 | 状态 |
|---|---|---|---|
| F39 | 工作模式 / 宠物模式切换，重启后记住 | S10 | 未开始 |
| F40 | 宠物形象（小灯兽）与单一 `.riv` | S10 | 未开始 |
| F41 | Rive 状态机：idle / working / waiting / success / error | S10 | 未开始 |
| F42 | 宠物左右吸附、探头、收起 | S11 | 未开始 |
| F43 | 宠物消息气泡 | S11 | 未开始 |
| F44 | 宠物空闲 settle，不持续 60 FPS | S11 | 未开始 |
| F45 | 多显示器位置记忆（工作面板 + 宠物分开记） | S12 | 未开始 |
| F46 | 自定义面板/宠物大小 | S12 | 未开始 |
| F47 | 声音主题与提醒强度 | S12 | 未开始 |
| F48 | 更完善的 session / deep link 跳转 | S12 | 未开始 |
| F49 | 自动更新 | S12 | 未开始 |
| F50 | macOS 窗口、Dock、菜单栏、Spaces | S13 | 未开始 |
| F51 | macOS 签名、公证、DMG | S13 | 未开始 |
| F52 | Windows + macOS 性能基线与泄漏修复 | S14 | 未开始 |

### 1.3 后续候选（v1 不做也可列出，便于以后开分支）

| ID | 功能 | 步骤 | 状态 |
|---|---|---|---|
| F53 | 多套宠物皮肤与季节主题 | S15 | 未开始 |
| F54 | 专注统计与每日任务总结 | S15 | 未开始 |
| F55 | 手机端通知桥接 | S15 | 未开始 |
| F56 | 更多 CLI / IDE 适配器 | S15 | 未开始 |
| F57 | 本地插件 SDK | S15 | 未开始 |
| F58 | 原生 macOS 宠物层（为 App Store 预留） | S16 | 未开始 |
| F59 | 本机网络状态指示（可选增强，非核心通道） | S16 | 未开始 |

---

## 2. 建议仓库结构

实施 S00 时按此创建。宠物相关文件在 S10 之前只保留空目录或 README，不要写实现。

```text
springcat-ai/
├─ src/
│  ├─ app/
│  │  ├─ App.svelte
│  │  ├─ routes/
│  │  └─ stores/
│  ├─ components/
│  │  ├─ work-panel/
│  │  ├─ pet/                 # S10 前不实现
│  │  ├─ task-drawer/
│  │  └─ settings/
│  ├─ domain/
│  │  ├─ task-event.ts
│  │  ├─ task-item.ts
│  │  ├─ surface-state.ts
│  │  └─ notification-policy.ts
│  ├─ services/
│  ├─ styles/
│  └─ assets/
├─ src-tauri/
│  ├─ src/
│  │  ├─ lib.rs
│  │  ├─ surface_state.rs
│  │  ├─ windows.rs
│  │  ├─ docking.rs
│  │  ├─ event_collector.rs
│  │  ├─ normalizer.rs
│  │  ├─ repository.rs
│  │  ├─ settings.rs
│  │  ├─ adapters/
│  │  └─ platform/
│  ├─ migrations/
│  ├─ capabilities/
│  └─ tauri.conf.json
├─ bridge/
│  ├─ src/main.rs
│  └─ README.md
├─ tests/
│  ├─ fixtures/
│  └─ e2e/
└─ docs/
    ├─ 项目描述.md
    └─ springcat-ai-v1.md      # 本文
```

应用数据目录：

```text
{app_data}/springcat-ai/
  ├─ inbox/{timestamp}-{uuid}.json
  ├─ tasks.sqlite
  ├─ settings.json
  └─ logs/
```

---

## S00. 工程脚手架

**目标：** 能在 Windows 上 `pnpm tauri dev` 打开一个空白透明窗口。  
**覆盖功能：** F01

### 任务

- [x] 安装 Rust 稳定版、Node LTS、Windows WebView2 运行时
- [x] 用 Tauri 2 + Svelte + TypeScript 模板创建项目，应用名 `springcat-ai`
- [x] 提交 `package-lock` / `pnpm-lock` 与 `Cargo.lock`
- [x] 按上一节建好目录，宠物目录放 `README.md` 说明“S10 前不实现”
- [x] `tauri.conf.json`：无边框、透明、置顶、跳过任务栏；窗口先给 `360×48`
- [x] 配置最小 capabilities：窗口位置/大小（托盘 / fs 留到 S03/S06 再加插件）
- [x] 前端 `html, body` 背景透明
- [x] 写 `docs/dev-notes.md` 记录本机工具链版本

### 验收

- [x] 前端 `pnpm build` 与 Rust `cargo test` 通过；用 `pnpm tauri dev` 本地看窗口
- [x] 窗口配置为无边框、透明、置顶、跳过任务栏
- [x] 仓库中没有 `.riv`、没有 `@rive-app` 依赖

### 禁止

- 不要同时接事件、SQLite、托盘、吸附
- 不要引入 Electron 或 React（除非明确改规格）

---

## S01. 领域模型（纯逻辑，先不接 UI）

**目标：** 任务事件、任务项、展示状态成为唯一真相。  
**覆盖功能：** F02

### 任务

- [x] 实现 `TaskEvent`、`TaskItem`、`SurfaceState`、`AppSettings` 类型（TS + Rust 各一份，字段对齐）
- [x] 实现纯函数 `deriveSurfaceState(tasks) → SurfaceState`
- [x] 优先级写死为：`waiting > failed > completed-unread > running > idle`
- [x] `summary` 裁剪长度、去掉控制字符
- [x] 未知 JSON 字段必须忽略
- [x] Vitest / cargo test 覆盖：空列表、单任务、多任务抢优先级、未读完成 vs 已读完成
- [x] `AppSettings.presentationMode` 默认为 `"work"`；读到 `"pet"` 且宠物未实现时回退 `"work"`

### 关键类型（不要改字段名）

```ts
type PresentationMode = "work" | "pet";
type DockSide = "top" | "left" | "right";
type TaskSource = "codex" | "cursor" | "grok-cli" | "gemini-cli" | "workbuddy" | "unknown";
type TaskStatus = "running" | "waiting" | "completed" | "failed" | "cancelled";

type SurfaceState =
  | { kind: "idle" }
  | { kind: "working"; task: TaskItem }
  | { kind: "waiting"; task: TaskItem }
  | { kind: "failed"; task: TaskItem }
  | { kind: "completed"; task: TaskItem; unread: boolean; mergedCount?: number };
```

### 验收

- [x] 领域层零 UI、零 Rive、零 Tauri Window 依赖
- [x] 单测能证明多任务时只产出一个最高优先级 `SurfaceState`

### 禁止

- 不要在领域层出现“耳朵”“跳跃”“胸灯”等宠物字段

---

## S02. 工作面板视觉（可先用浏览器演示页）

**目标：** 五种状态在页面里能切换，布局符合规格。  
**覆盖功能：** F03、F04

### 任务

- [x] `WorkPanel.svelte`：收起条、探出摘要、展开列表
- [x] `StatusChip.svelte`：状态色条 / 状态点
- [x] 顶边形态约 `360×48`；侧边窄条约 `48×160` 或侧边卡片 `280×48`
- [x] 探出摘要约 `360×88`；列表约 `420×520`
- [x] 颜色：idle 低亮度、working 暖黄、waiting 琥珀、completed 薄荷绿、failed 珊瑚红
- [x] working 状态点可缓慢呼吸；系统“减少动态效果”时改为静态色
- [x] 演示页用按钮切换五种状态和顶边/侧边
- [x] 文案示例：`Codex 已完成：修复登录页测试`

### 验收

- [x] 不启动 Tauri 也能在浏览器里看完所有状态
- [x] 没有宠物图形，没有 Rive

### 禁止

- 不要在这一步做真实吸附或系统窗口 API

---

## S03. 桌面窗口与托盘

**目标：** 面板作为常驻桌面窗口出现，托盘可退出。  
**覆盖功能：** F05、F06

### 任务

- [x] Rust `windows.rs`：创建唯一主窗口
- [x] 透明、无边框、置顶、Windows 跳过任务栏
- [x] 默认位置：当前显示器工作区顶部右侧
- [x] 真实窗口矩形贴合可见面板，减少透明区域挡点击
- [x] 托盘图标
- [x] 托盘菜单：查看所有任务、静音 1 小时、专注模式、置顶、设置、退出  
  （后几项可先占位，S08/S09 再接逻辑）
- [x] 单击托盘：显示/聚焦面板
- [x] 设置窗口本步不要创建

### 验收

- [x] 启动后桌面右上出现工作面板，没有宠物
- [x] 托盘能退出应用
- [x] 只有一个常驻 WebView

### 禁止

- 不要做全屏点击穿透窗口
- 不要在这一步实现左右吸附算法

---

## S04. 拖动与吸附

**目标：** 面板可拖到顶部或左右侧并吸住，离开后收起。  
**覆盖功能：** F07、F08、F09、F10

### 任务

- [x] 面板可拖动（`startDragging` 或自管鼠标位移）
- [x] 距边缘 48–72 逻辑像素出现吸附预览光带
- [x] 松手吸附到 `top | left | right`，时长 200–300 ms
- [x] **不支持底部吸附**
- [x] 使用 `monitor.workArea`，避开任务栏 / 菜单栏
- [x] 多显示器：以面板当前所在屏的工作区为准
- [x] 记住每个显示器的边和沿该边的位置
- [x] 顶边吸附 → 列表向下展开；侧边吸附 → 列表向桌面内展开
- [x] 空闲自动收成窄条/小卡片（200–250 ms）
- [x] 鼠标靠近 120–180 ms 探出；离开后延迟 600–1000 ms 再收
- [x] 高 DPI 用物理像素计算，再换逻辑像素写窗口

### 验收

- [x] Windows 125% / 150% 缩放下能吸到顶边和侧边
- [x] 吸附后不长期挡住大块桌面
- [x] 拖到另一块屏后，以那块屏的工作区重新吸附

### 禁止

- 不要在 JS 里硬编码屏幕分辨率
- 不要为了吸附引入第二个常驻窗口

---

## S05. 任务列表与面板交互

**目标：** 面板能展示模拟任务并完成基本操作。  
**覆盖功能：** F11、F12、F13

### 任务

- [x] 单击面板：展开 / 收起列表
- [x] `Esc`：关闭列表或摘要
- [x] 双击：打开最近一个待处理任务（先用模拟 deepLink）
- [x] 列表字段：来源、标题、状态、持续时间、开始/完成时间、未读
- [x] 默认只渲染最近 50 条
- [x] 右键菜单：查看所有任务、切换宠物模式（禁用+即将推出）、静音、专注、置顶、设置、退出
- [x] 前端 store 先喂 fixtures，不接真实 hook

### 验收

- [x] 用 3 条模拟任务能看出优先级最高的那条反映在面板标题上
- [x] 展开列表不遮挡到无法关闭（Esc 或再次单击可关）

---

## S06. 事件通道与 SQLite

**目标：** 命令行写入一个事件文件，面板在 500 ms 内更新。  
**覆盖功能：** F14–F19

这是工作模式的第一条垂直切片，必须跑通。

### 任务

- [x] `bridge/`：`springcat-bridge emit --source codex --event completed`
- [x] 从 stdin 读 JSON；临时文件写入 → flush → 原子改名为 `.json`
- [x] inbox 路径：`{app_data}/springcat-ai/inbox/`
- [x] Rust `event_collector.rs`：`notify` 监听创建事件，禁止轮询目录
- [x] `normalizer.rs`：转成 `TaskEvent`；损坏文件记日志并隔离，不崩溃
- [x] `repository.rs`：SQLite 增改查；`eventId` 去重
- [x] 规范化成功后删除或归档 inbox 文件
- [x] 不把完整终端输出、源代码写入库；`raw` 仅诊断且默认不展示
- [x] Tauri event 推到前端 → store 更新 → `deriveSurfaceState` → 面板
- [x] fixtures：正常、重复、乱序、损坏、未知字段
- [x] 历史默认 7 天清理任务

### 手工验证

```text
echo {json} | springcat-bridge emit --source codex --event task.started
echo {json} | springcat-bridge emit --source codex --event task.completed
```

### 验收

- [x] 模拟完成事件后，面板变薄荷绿并出现一行摘要
- [x] 同一 `eventId` 写两次不会生成两条任务，也不会播两次完成提示
- [x] 损坏 JSON 不导致进程退出
- [x] 应用没启动时写入 inbox，启动后能补读

### 禁止

- 不要开本地 HTTP / WebSocket 服务收事件
- 不要在这一步接真实 Codex/Cursor

---

## S07. 三个工具适配器

**目标：** Codex、Cursor、Grok CLI 的 hook 都能变成同一 `TaskEvent`。  
**覆盖功能：** F20–F23

### 任务

- [x] `adapters/codex.rs`：对接 Codex notify / hooks，输出 `TaskEvent`
- [x] `adapters/cursor.rs`：对接 Cursor `stop` 等 hooks
- [x] `adapters/grok_cli.rs`：对接实际使用的 Grok CLI 发行版 hooks
- [x] 每种来源独立 fixture 目录：`tests/fixtures/{codex,cursor,grok-cli}/`
- [x] 适配器互不影响：一个 panic/错误只记日志
- [x] 文档：`docs/adapters.md` 写清每种工具的 hook 安装步骤
- [x] `title` / `summary` 继续走规范化，禁止把完整对话入库

### 验收

- [x] 三个来源的“任务完成”都能更新同一工作面板
- [x] 关掉其中一个适配器，另外两个仍可用

### 禁止

- 不要为了 Grok 改统一 `TaskEvent` 字段
- 不要读取用户项目源码来“猜任务状态”

---

## S08. 通知策略、静音、专注、跳转

**目标：** 提醒符合“环境式”原则，重要的不丢、普通的不吵。  
**覆盖功能：** F24–F30、F28

### 任务

- [x] `notification-policy.ts` + Rust 侧策略一致
- [x] `working`：持续探出当前会话标题，不自动收回
- [x] `waiting`：探出，摘要不自动消失
- [x] `failed`：探出，摘要不自动消失或明显更久
- [x] `completed`：探出，约 5 秒收起，保留未读绿点
- [x] 时间窗口内多个 completed 合并文案：`3 个任务已完成`
- [x] 高优先级可打断低优先级动效
- [x] 点击摘要 / 任务行：有 `deepLink` 则打开；否则打开来源应用
- [x] 静音 1 小时：不探出、不播完成动效，状态色仍更新；到期自动恢复
- [x] 专注模式：仅 `waiting` / `failed` 主动探出，`completed` 只留未读点
- [x] 托盘菜单接上静音和专注

### 验收

- [x] 执行中桌面保持安静
- [x] 等待确认不会自己消失
- [x] 连续完成 3 个任务只出现一次合并提示，不连跳三次

---

## S09. 设置、开机启动、打包（工作模式 MVP 收口）

**目标：** 可安装、可配置、可长期挂着。  
**覆盖功能：** F31–F38

### 任务

- [x] 设置窗口按需创建，关闭后销毁 WebView
- [x] 设置项：
  - [x] 展示模式（工作 / 宠物即将推出）
  - [x] 默认吸附边：顶 / 左 / 右
  - [x] 置顶
  - [x] 开机启动
  - [x] 双击行为
  - [x] 历史保留：1 / 7 / 30 天 / 不保存
  - [x] 适配器启用开关：Codex / Cursor / Grok CLI
- [x] Windows 开机启动（Startup 文件夹或约定机制）
- [x] 减少动态效果：跟随系统，取消呼吸和弹簧，只留淡入淡出和变色
- [x] 日志滚动文件；禁止打印凭证、完整 prompt、完整工具输出
- [x] Windows 125% / 150% / 200% 实测吸附
- [x] 双屏、负坐标屏、任务栏换边实测
- [x] 打 EXE / MSI
- [x] 记录：安装包、空闲内存、空闲 CPU、事件延迟

### MVP 总验收（必须全部勾上才能进 S10）

- [x] 启动默认工作模式，无宠物、无 Rive
- [x] 可吸到顶 / 左 / 右
- [x] 执行中不弹摘要
- [x] 完成 / 失败 / 等待可区分
- [x] 等待消息不自动消失
- [x] 多任务可在列表中查看
- [x] 能返回来源应用或打开 deepLink
- [x] 托盘可静音、专注、退出
- [x] 损坏事件不崩溃；重复事件不重复建任务
- [x] 库中无完整代码和凭证
- [x] 空闲不空转 60 FPS
- [x] 设置窗口关闭后 WebView 释放

---

## S10. 宠物模式：角色与状态机

**前置：** S09 MVP 总验收全部通过。  
**覆盖功能：** F39、F40、F41

### 任务

- [ ] 增加 `@rive-app/canvas-lite`，仅宠物模式动态 import
- [ ] 单一 `.riv`，目标 < 2 MB，一张 Artboard、一套状态机
- [ ] 状态：idle / working / waiting / success / error / dragging / docked / peeking / sleeping
- [ ] 输入字段按规格：`status` `isDocked` `dockSide` `isHovered` `isDragging` `unreadCount` `success` `error`
- [ ] 文字仍由 HTML 渲染，Rive 里不嵌音频
- [ ] 设置和右键菜单可切换 `work | pet`
- [ ] 切换不中断正在跟踪的任务，只换展示层
- [ ] 工作模式路径继续不创建 Rive 实例

### 验收

- [ ] 演示输入可切换宠物五种业务状态
- [ ] 切回工作模式后 Rive 被卸载
- [ ] 事件通道零改动仍能驱动宠物

---

## S11. 宠物吸附、气泡、性能

**覆盖功能：** F42、F43、F44

### 任务

- [ ] 宠物只吸左 / 右，默认右下，避开任务栏
- [ ] 拖动倾斜、压扁回弹、藏入 55%–65%、眼睛看向鼠标再探头
- [ ] 消息气泡 150–250 ms；完成约 5 秒；等待不自动关
- [ ] 空闲眨眼 8–20 秒；呼吸 8–12 秒一次，然后 settle
- [ ] 窗口不可见 / 锁屏 / 全屏抑制时 pause
- [ ] 减少动态效果时取消跳跃和弹簧

### 验收

- [ ] 宠物和工作面板消费同一 `SurfaceState`
- [ ] 空闲 CPU 仍接近 0%，不空转 60 FPS

---

## S12. v1.0 增强（Windows）

**覆盖功能：** F45–F49

### 任务

- [ ] 多显示器位置：工作模式与宠物模式分开存储
- [ ] 自定义大小（面板高度/宽度、宠物缩放），有上下限
- [ ] 声音主题：完成 / 失败 / 等待；可调强度；静音和专注时遵守策略
- [ ] 来源跳转补 session / 工作区路径（能解析才用，失败则打开应用）
- [ ] Tauri updater：检查更新、下载、安装；失败可继续用旧版

### 验收

- [ ] 换屏、换模式后位置各自恢复
- [ ] 无声音文件时应用仍可运行

---

## S13. macOS

**覆盖功能：** F50、F51

### 任务

- [ ] 工作面板避开 Dock 与菜单栏
- [ ] 透明窗口：按 Tauri 文档启用所需配置；接受官网 DMG、不进 App Store
- [ ] Spaces / 原生全屏降级策略：进全屏空间时不强制盖住
- [ ] LaunchAgent 开机启动
- [ ] 签名 + 公证 + DMG
- [ ] Retina 下面板边缘清晰；宠物模式边缘同样清晰

### 验收

- [ ] macOS 12+ 上工作模式可吸附顶/左/右
- [ ] 公证后的 DMG 可打开，无未签名拦截（在已配置证书的前提下）

---

## S14. v1.0 收口

**覆盖功能：** F52

### 任务

- [ ] Windows / macOS 性能基线表写入 `docs/perf-baseline.md`
- [ ] 修空闲渲染、窗口残留、切换模式内存泄漏
- [ ] 回归 S09 MVP 总验收 + 宠物切换
- [ ] 输出 Windows EXE/MSI 与 macOS DMG

### v1.0 完成定义

- [ ] 工作模式与宠物模式可切换且不丢任务
- [ ] 三工具适配器在两个平台都能完成提醒
- [ ] 自动更新可用或有明确延期记录
- [ ] 性能数据已记录，空闲 CPU 常态 < 1%

---

## S15. 后续候选（非 v1 必做）

**覆盖功能：** F53–F57

每项单独开分支，不要和 MVP 混在一个 PR。

- [ ] **皮肤 / 季节主题：** 多套 `.riv` 或工作面板 CSS 主题，热切换不重载任务库
- [ ] **专注统计 / 每日总结：** 仅聚合状态与时长，不存对话内容；可在设置关闭
- [ ] **手机通知桥接：** 需单独设计鉴权与最小字段；默认关闭
- [ ] **更多适配器：** 每个新工具一个 adapter + fixtures，禁止改 `TaskEvent` 主字段
- [ ] **本地插件 SDK：** 只允许写 inbox 规范事件，不允许读数据库

---

## S16. 平台深化与可选系统能力

**覆盖功能：** F58、F59

- [ ] **原生 macOS 宠物层：** 若必须进 App Store，用 AppKit 替换透明宠物层，Svelte 任务 UI 保留
- [ ] **网络状态指示（可选）：** Rust command 读本机连通性，断网时面板显示离线；**不**作为任务事件来源，**不**扫描局域网

---

## 3. 每步结束时的记录模板

复制到 `docs/progress.md`：

```text
步骤：S0X
日期：
安装包：
空闲内存：
空闲 CPU：
事件延迟：
通过的验收：
未通过 / 延期：
下一步：
```

---

## 4. 推荐的第一条垂直切片（S00–S06 的最短路径）

如果希望尽早看到“任务完成 → 面板变绿”，按这个最小闭环做，但仍不要跳过 S01 领域模型：

```text
S00 脚手架
 → S01 deriveSurfaceState
 → S02 面板五种状态
 → S03 一个透明窗口
 → S06 inbox + SQLite + 推 UI
```

S04 吸附、S05 列表、S07 真实工具可以紧接着补。**不要**为了“先看见宠物”插入 Rive。

---

## 5. 和规格文档的分工

| 文档 | 用途 |
|---|---|
| [项目描述.md](./项目描述.md) | 为什么做、体验原则、技术选型、验收标准 |
| [springcat-ai-v1.md](./springcat-ai-v1.md) | 先做什么、后做什么、每步勾什么 |

实现时打开本文逐步打勾；改产品行为时先改项目描述，再回写本文对应步骤。
