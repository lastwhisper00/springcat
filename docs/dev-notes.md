# 工程记录

记录开始实施时的工具链稳定版。不要把这些版本写死进产品规格。

## 本机（2026-08-13）

| 工具 | 版本 |
|---|---|
| Node | v24.14.0 |
| pnpm | 11.1.3 |
| Rustc | 1.94.1 (stable-x86_64-pc-windows-msvc) |
| Cargo | 1.94.1 |
| OS | Windows 10/11 (10.0.26200) |

前端与 Tauri 依赖以根目录 `pnpm-lock.yaml`、`src-tauri/Cargo.lock` 为准。

## 当前步骤

- S00–S09 工作模式 MVP：代码已落地
- 下一步：本机手工验收吸附 / inbox / 设置后，再考虑 S10 宠物模式
- 桥接：`cargo build --manifest-path bridge/Cargo.toml --release`
- 验证事件：`echo {"title":"修复登录页测试"} | springcat-bridge emit --source codex --event task.completed`

## 性能基线

```text
步骤：S04–S09
日期：2026-08-13
安装包：NSIS 3.1 MB / MSI 4.6 MB / EXE 12.8 MB
空闲内存：待 tauri 挂起后目测
空闲 CPU：待 tauri 挂起后目测
事件延迟：notify 监听 inbox，目标 < 500ms
```
