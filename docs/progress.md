# 进度

## S00 / S01 — 2026-08-13

步骤：S00 脚手架 + S01 领域模型
通过的验收：工程骨架、领域模型单测
下一步：已完成，进入 S02

## S02 — 2026-08-13

步骤：工作面板三态布局与五种状态
日期：2026-08-13
安装包：未打包
空闲内存：N/A（浏览器演示）
空闲 CPU：N/A
事件延迟：N/A
通过的验收：
- 收起 / 探出 / 展开列表
- idle / working / waiting / completed / failed
- 文案「Codex 已完成：修复登录页测试」
- `pnpm dev` 浏览器演示，无需 Tauri
未通过 / 延期：真实吸附放到 S04
下一步：S04 拖动与左右/顶部吸附

## S03 — 2026-08-13

步骤：桌面窗口与托盘
通过的验收：
- 单一透明无边框窗口，贴合 360×48 面板
- 默认工作区右上角
- 托盘：查看所有任务、置顶、退出；左键隐藏/显示
- 设置/静音/专注为菜单占位
下一步：S04 拖动与吸附

## S04–S09 — 2026-08-13

步骤：工作模式 MVP 收口（吸附、列表、inbox、适配器、通知、设置）
日期：2026-08-13
安装包：
- NSIS `src-tauri/target/release/bundle/nsis/SpringCat_0.1.0_x64-setup.exe` ≈ 3.1 MB
- MSI `src-tauri/target/release/bundle/msi/SpringCat_0.1.0_x64_en-US.msi` ≈ 4.6 MB
- 主程序 `src-tauri/target/release/springcat-ai.exe` ≈ 12.8 MB
空闲内存：待本机挂起后目测
空闲 CPU：待本机挂起后目测
事件延迟：inbox `notify` 推送，目标 < 500ms（无轮询、无 HTTP）
通过的验收：
- 顶 / 左 / 右吸附，禁止底边；按显示器记住边和沿边位置
- 单击展开/收起，Esc 关闭，双击打开最近待处理任务
- `springcat-bridge emit` → inbox → SQLite 去重 → 面板
- Codex / Cursor / Grok CLI 适配器隔离
- working/waiting/failed 持续探出；completed 约 5 秒；可静音、专注
- 设置窗口按需创建；开机启动、置顶、历史保留、适配器开关
未通过 / 延期：
- 宠物模式 / Rive：S10，设置里显示即将推出
- 125% / 150% / 200% 与双屏：需本机手工拖一次确认
- EXE/MSI：若本机打包失败（证书/WebView2），见 `docs/dev-notes.md`
下一步：S10 仅在 MVP 手工验收通过后开始
