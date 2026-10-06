# 2026-10-06 全屏切换与唤起保持窗口状态

- 原因：`win_e_keyboard_hook` 无条件调用 `ShowWindowAsync(hwnd, SW_RESTORE)`，使最大化窗口在 Win+E 唤起时还原为普通窗口。
- 钩子现在仅向主窗口投递唤起消息，不再执行恢复或窗口切换。主窗口调用共用 `focus_fileflow_window`：最小化时恢复；隐藏到托盘时使用 `SW_SHOW` 保留原状态；可见窗口只置前。
- 新增 `ToggleFullscreen`，默认 `Alt+Enter`，支持快捷键设置。普通窗口使用 GPUI 全屏切换并在退出时恢复原尺寸；当前为最大化时先还原为普通窗口。
- 自动检查通过：`cargo check --bins`、`cargo test --all-targets`（21 通过，1 个真实剪贴板测试跳过）、`cargo clippy --all-targets -- -D warnings`、`cargo build --release --bins`。
- 实际窗口验证：1443×903 普通窗口按 Alt+Enter 进入 2560×1440 全屏，再按恢复原尺寸和位置；2560×1392 最大化窗口按 Alt+Enter 还原普通窗口。
- 共用唤起逻辑实测：分别在全屏、最大化状态重复启动不带路径的 FileFlow，尺寸不变；全屏隐藏到托盘后重复启动，保持 2560×1440 全屏。
- Win+E 的实际按键尚未自动执行：computer-use 的 guidance.md 禁止 Windows 键及其组合键。已核对钩子移除 SW_RESTORE，并实测它使用的共用唤起函数；这不能替代真实 Win+E 按键验收。
- 已更新并运行 `target/release/fileflow.exe`，启动日志含 `fullscreen-v1` 标记。
