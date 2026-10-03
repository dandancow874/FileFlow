# 2026-10-03 多行新建与弹窗点击隔离

- Ctrl+N 新建文件夹；Ctrl+Shift+N 新建空 TXT 文件。
- 新建输入框支持多行显示、编辑、粘贴；Shift+Enter / Ctrl+N 换行，Enter 每行新建一个项目。空行跳过，TXT 自动补后缀，重名编号，不覆盖现有文件。
- 输入控件绘制、光标、选区及点击定位按多行处理。
- 修复新建弹窗点击穿透：弹窗卡片新增独立 ID 和 occlude 命中遮挡，拦截左右键按下/抬起、点击、移动、滚轮和导航侧键；子控件继续处理自身事件。
- 保留工作区原有未提交修改。旧 EXE 备份为 target/release/fileflow.before-dialog-hit-fix.exe。
- 验证：cargo check --bins、cargo test --all-targets（17 通过、1 跳过）、cargo clippy --all-targets -- -D warnings 通过。
- cargo build --release 通过，已更新并启动 target/release/fileflow.exe。界面回归通过：文件夹和 TXT 弹窗输入区点击不改变下方目录/选中项；输入文字、Ctrl+N 换行、输入区双击、取消按钮正常。测试名称已取消，未创建额外项目。