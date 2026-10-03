# 2026-10-03 空白双击与快捷方式图标修复

- 空白点击统一在 `pane-hit-left/right` 处理，移除详情/列表内层重复入口。图标、分栏、详情、列表共用双击返回上级；条目点击继续停止冒泡。
- `.lnk` 使用实际路径向 Shell 查询图标（不使用 SHGFI_USEFILEATTRIBUTES），加入快捷方式箭头；内存与磁盘缓存按快捷方式完整路径区分，不复用旧扩展名通用缓存。
- 修改前完整源码备份：`target/main.rs.before-blank-double-click`。保留原有未提交修改。
- 验证：cargo check --bins、cargo test --all-targets（15 通过，1 跳过）、cargo clippy --all-targets -- -D warnings 均通过。
- 新增真实 Shell 快捷方式回归测试：枚举两个生成的 .lnk，检查独立缓存、非透明图像及自定义图标像素不同。
- 用户后续补充“桌面这些 .lnk 都看不到”，尚待确认指图标还是整个条目；当前枚举代码不排除 .lnk。真实界面点击与用户桌面显示仍待验证。