# Tauri / 系统服务

S03 起逐步填充：窗口命令封装、deep link、事件订阅。

前端不得直接假设 HWND / NSWindow 细节，一律经 `invoke` / event 与 Rust 通信。
