# FileFlow

Windows 文件管理器，Rust + [gpui](https://github.com/zed-industries/zed)（Zed 编辑器的 GPU 加速 UI 框架）构建。

单文件绿色程序，无需安装，无需 .NET / Java 等运行时。

## 功能

- **双栏分屏**浏览，标签页，前进/后退历史
- **五种视图**：详情 / 列表 / 分栏 / 中·大·超大图标
- **全视图虚拟化渲染**——上万文件的目录依然流畅滚动（uniform_list）
- 文件夹大小**后台异步计算**并显示
- 图片 / 视频**缩略图**（image crate 直解 + Windows Shell 管线覆盖 heic/psd 等）
- 系统文件关联图标（zip/exe/pdf 真实图标，按扩展名缓存）
- **拖拽**：应用内移动/复制（Ctrl）、拖出到其他程序（CF_HDROP + Shell IDataObject）
- 拖框选择、Ctrl/Shift 多选、键盘导航、批量重命名（`名称_001` 序号模式）
- **排序**：名称/类型/修改日期/大小，底栏点击循环切换、Ctrl+点击翻转升降序
- Ctrl+Z 撤销（删除/重命名/新建）
- 右键菜单合并 **Windows Shell 原生菜单**（第三方扩展可用）
- Win+E 接管（可选）、系统托盘、单实例
- 标题栏拖动窗口、双击最大化/还原
- 网络邻居（SMB 共享浏览）

## 下载
![Uploading 图片.png…]()



从 [Releases](../../releases) 下载 `FileFlow-vX.Y-win64.zip`，解压后双击 `FileFlow.exe`。

要求 Windows 10/11 64 位。VC 运行库已随包附带。

## 构建

```bash
git clone https://github.com/<你的用户名>/FileFlow.git
cd FileFlow
cargo build --release
```

- 需要 Rust stable（MSVC 工具链）与 Windows SDK（gpui 的 shader 编译用到 `fxc.exe`）
- 产物：`target/release/FileFlow.exe`（图标已内嵌，字体编译期打包）
- 分发：把 `FileFlow.exe`、`FileFlowExplorer.exe`、`Assets/` 与 `vcruntime140*.dll` 一起打包（参考 `dist/` 布局）

## 技术要点

- **gpui 0.2**：uniform_list 虚拟化四种视图；行/列绝对像素定位保证表格式对齐；UniformListScrollHandle + 行高×index 数学推算实现拖框选择
- **WndProc subclass** 接管非客户区消息：标题带拖动/双击最大化（阈值拖动状态机）、托盘消息解码（`lparam & 0xffff`）
- 缩略图管线：后台单线程 + 磁盘缓存（LRU 512 个 / 192MB）+ 失败黑名单防 CPU 风暴
- Shell 集成：`IShellItemImageFactory` 真缩略图、`SHGetFileInfoW` 类型图标、`DoDragDrop` OLE 拖放、`IContextMenu` 合并菜单（模态循环挂起后台任务防 RefCell 重入）

## License

MIT
