# 2026-10-06 键盘定位、网格导航与外部路径标签

- 字母/数字定位和方向键滚动统一将条目序号转换成虚拟列表的行号。Columns、M、L、XL 按实际列数换算，Details/List 表头在列表外，不再额外偏移一行。
- 左右键按显示顺序移动一项，行尾向右进入下一行，行首向左进入上一行；上下键按实际列数移动，目录首尾不循环。
- Columns 渲染与键盘导航共用 `virtual_grid_geometry`，避免忽略列间距时列数不一致。
- 外部路径命令先在左栏新增标签，再导航到目录；已有标签保留。未携带路径的普通窗口激活逻辑不变。
- 自动验证：`cargo check --bins`、`cargo test --all-targets`（19 通过、1 个真实剪贴板测试跳过）、`cargo clippy --all-targets -- -D warnings`、`cargo build --release --bins`、`git diff --check` 通过。新增回归测试覆盖六种视图的滚动行号和四种网格模式的跨行/首尾导航。
- 实际界面：含 241 个文件夹的专用测试目录中，XL 模式输入 `m` 后 M000 可见；M000 位于行尾，按右箭头进入下一行 M001，按左箭头回到 M000。
- 实际外部启动：向运行中的程序传入测试目录路径，新增测试标签且保留原标签；随后外部打开 MuMu 共享目录也新增标签。
- 其他视图的完整人工检查尚未完成（用户同时使用窗口）；自动测试通过不等同于这些视图的 GUI 验收。
- 已更新运行文件：`target/release/fileflow.exe`。启动日志标记：`2026-10-06 keyboard-nav-v1 external-new-tab-v1`。
- 修改前备份：`target/main.rs.before-keyboard-navigation-20261006`、`target/release/fileflow.before-keyboard-nav-20261006.exe`；专用测试目录：`target/keyboard-nav-check-20261006`。
