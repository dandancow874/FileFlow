# FileFlow GPUI 开发文档

## 项目目标

FileFlow 当前处于 Rust + GPUI 迁移阶段，目标是做一个轻量、快速、低资源占用的 Windows 文件管理器。视觉和交互参考 File Pilot；GPUI 用于承载标签页、双栏、列表和动画交互。

当前目标是把旧 WPF / egui 版的可见功能完整迁移到 GPUI，而不是重新定义产品范围。用户已确认的 File Pilot 风格行为都以本文件为验收清单；后续 Agent 不应要求用户重复描述。

## 技术栈

- 语言：Rust 2024 edition
- UI：GPUI（Zed 团队的 GPU 加速 UI 框架）
- 时间格式化：chrono
- 配置序列化：serde / serde_json
- 目录变更监听：notify（非递归监听当前可见目录）
- Windows 文件操作：`IFileOperation` / Shell COM
- 字体：`SarasaUiSC-Regular.ttf`，通过 `include_bytes!` 内嵌到 exe，避免中文显示方框
- 单实例：single-instance

## 目录结构

```text
C:\Users\Administrator\FileFlow
├─ Cargo.toml
├─ Cargo.lock
├─ DEVELOPMENT.md
├─ SarasaUiSC-Regular.ttf
└─ src
   ├─ main.rs              # GPUI 应用状态、文件视图与命令
   ├─ text_input.rs        # 原生文本输入组件（筛选、地址栏复用）
   └─ egui_legacy.rs       # 旧 egui 实现备份，仅作功能迁移参考，不参与编译
```

当前 GPUI 代码仍集中在 `src/main.rs`。后续建议拆成：

```text
src/
├─ main.rs
├─ app.rs          # FileFlowApp 状态和主 update
├─ pane.rs         # Pane / FileEntry / 枚举 / 排序
├─ ui/
│  ├─ toolbar.rs
│  ├─ sidebar.rs
│  ├─ pane_view.rs
│  └─ dialogs.rs
├─ fs_ops.rs       # 复制、移动、删除、重命名、回收站
└─ config.rs       # AppConfig 持久化
```

## 构建与运行

开发检查：

```powershell
cargo check
```

Release 构建：

```powershell
cargo build --release
```

### Windows / GPUI 构建前置条件

GPUI 在 Windows 发布构建时需要 Windows SDK 的 `fxc.exe`。本机可用路径是：

```text
C:\Program Files (x86)\Windows Kits\10\bin\10.0.22621.0\x64
```

若命令行未自动找到它，先在当前 PowerShell 会话执行：

```powershell
$env:PATH = "C:\Program Files (x86)\Windows Kits\10\bin\10.0.22621.0\x64;$env:PATH"
```

运行文件：

```text
C:\Users\Administrator\FileFlow\target\release\fileflow.exe
```

## 当前已实现

### GPUI 迁移版

- 真正的标签页状态：新建标签不再打开双栏；标签可切换和关闭
- 独立双栏开关：双栏仅由工具栏按钮控制
- GPUI 原生顶部标签带、两栏路径头、文件列表/图标网格和底部状态栏
- Details / List / Columns / M Icons / L Icons / XL Icons 六种查看状态
- 文件夹可进入，面包屑与上级导航结构已就位
- 双栏可独立进入目录和返回上级；双击目录进入，单击选择
- GPUI 本地图片管线：M/L/XL 图标视图加载 jpg/png/webp/bmp/gif/tiff，加载失败时稳定回退到文件图标
- 底栏文件夹按钮支持四态循环：仅文件夹、混合、文件夹优先、文件优先
- GPUI 快捷键：`F5` / `Ctrl+R` 刷新、`Alt+Up` / `Backspace` 上级、`Ctrl+B` 双栏、`Ctrl+N` 新建文件夹、`Ctrl+T` 新建标签、`Ctrl+W` 关闭标签、`Ctrl+Shift+T` 恢复关闭的标签、`Ctrl+H` 显示隐藏项目、`Ctrl+1/2` 在双栏间复制/移动所选项目
- 在文件区按 `Ctrl+滚轮` 切换六种查看方式，GPUI 用 140ms 淡入过渡处理视图切换
- 左侧栏、顶部标签、面包屑、底部筛选输入与状态条均已切到 GPUI 原生布局；文件夹图标不再依赖字体字形
- 面包屑可以逐级点击跳转；`Ctrl+L` 和地址栏图标进入完整路径编辑，`Enter` 进入路径，`Esc` 返回普通面包屑
- `Ctrl+F` 聚焦底部筛选输入；筛选变更会即时过滤文件区，`Esc` 清空并退出
- 本地及网络目录在 GPUI 后台执行器中枚举；使用 generation token 丢弃过期结果，快速切换路径不会被旧结果覆盖
- 缩略图解码进入有界后台队列，同一路径自动去重，UI 渲染阶段只读取已有缓存
- 左右当前目录由 `notify` 非递归监听，变更事件经 140ms 合并后自动刷新
- 复制、移动和删除统一使用 Windows `IFileOperation`；删除进入回收站并写入系统撤销记录
- 快捷键页支持点击录制、冲突检测、即时重绑及 `settings.json` 持久化

## GPUI 迁移验收清单

### 已迁移并需要持续回归

- [x] 无控制台窗口的 Windows GUI 入口
- [x] 标签页的新建、切换和关闭（关闭最后一页不关闭程序）
- [x] 双栏独立路径与同宽布局
- [x] 左侧位置栏、磁盘入口、顶栏和底栏
- [x] Details / List / Columns / M / L / XL Icons 与 Ctrl+滚轮切换动画
- [x] 目录进入、上级、可点击面包屑、路径文本输入
- [x] 基础 WebP / jpg / png 等图像绘制与等比 `Contain` 布局
- [x] 底部筛选控件、`Ctrl+F`、`Esc` 清空
- [x] Space 文件预览：单栏时文件区与预览区 1:1 分栏，图片保持原始比例

### 正在迁移，完成前不得从旧版移除

- [ ] 地址栏下拉建议、历史路径灰显与逐级目录列表
- [ ] 独立左右栏的前进/后退历史、活动栏与 Tab 切换
- [ ] 多选、Shift 范围选择、输入字符定位与字符高亮
- [ ] 新建文件夹弹窗：多行、Shift+Enter、Enter 提交
- [ ] 右键菜单、重命名、批量重命名、回收站删除、系统剪贴板文件操作
- [ ] 文本预览、缩略图缓存/后台解码，以及按住 Space 的临时预览行为
- [ ] 设置页：Sarasa 默认字体、界面/文件字体大小、间距、动画、圆角、隐藏文件、缩略图策略、扩展名、单双击、非活动栏亮度
- [x] 独立快捷键页与用户自定义快捷键保存
- [ ] 配置持久化与旧 `%LOCALAPPDATA%\\FileFlowEgui\\settings.json` 兼容迁移
- [ ] 单实例与可选 Win+E 接管；再次 Win+E 只激活现有窗口
- [x] 后台目录扫描、目录变更监听与缩略图有界队列

### 旧 egui 参考实现

- Sarasa UI SC 中文字体内嵌
- 浅色高密度界面风格
- 左侧栏：筛选、常用位置、磁盘
- 顶部工具栏：后退、前进、上级、双栏、新建、预览、查看模式、设置；复制、剪切、粘贴、重命名、删除移入右键菜单并保留快捷键
- 面包屑地址栏：点击层级可跳转，箭头可打开该级目录下拉
- 地址弹窗：`Ctrl+L` 打开，支持完整路径输入和列表过滤
- 双栏：左右可独立路径
- 查看模式：Details（完整元数据列）/ List（紧凑单列）/ Columns（多列名称流）/ M Icons / L Icons / XL Icons；`Ctrl+Shift+1~6` 切换
- 快捷键：`Ctrl+Shift+1~6` 切换查看模式，`F5` / `Ctrl+R` 刷新，`Ctrl+N` 新建文件夹，`Ctrl+B` 双栏，`Tab` 切换活动栏，`Alt+方向键` 后退/前进/上级，`Enter` 打开，`Backspace` 上级，`Ctrl+H` 显示隐藏文件
- 筛选：每个文件栏底部都有筛选输入，`Ctrl+F` 直接聚焦；普通输入会定位同前缀的首个项目并高亮匹配字符
- 多选：普通点击单选，Ctrl 点击增减选择，Shift 点击范围选择，Ctrl+A 全选当前筛选结果
- 排序：名称、类型、修改日期、大小
- 基础文件操作：复制、剪切、粘贴、删除到回收站、重命名、新建文件夹；新建文件夹支持多行批量创建，Shift+Enter 换行、Enter 创建
- 双栏传送：`Ctrl+1` 复制所选项目到另一栏，`Ctrl+2` 移动所选项目到另一栏；另一栏路径相同会阻止操作
- 右键菜单：打开、新建、排序、刷新、复制路径、命令提示符、资源管理器中显示、属性
- 右键菜单：支持“在另一栏打开”，会自动启用双栏并把目标路径放到另一侧
- 右键菜单：文件夹可手动“计算文件夹大小”，默认不自动扫描，避免拖慢大目录和网络目录
- 设置浮窗：界面字体大小、文件列表字体大小、行距、缩略图、交替行、圆角、动画、单击打开、多选框、非活动栏亮度等均实时生效
- 设置浮窗：系统集成区，包含单实例状态和 Win+E 接管安全开关
- 设置持久化：`%LOCALAPPDATA%\FileFlowEgui\settings.json`
- 独立快捷键页面：工具栏 `⌨` 打开，可修改新建、筛选、刷新、双栏、复制/移动到另一栏的快捷键并保存
- 预览面板：图片和文本预览，支持 webp 图片预览
- 缩略图：详情和 M/L/XL Icons 模式下对 jpg/png/webp/bmp/gif/tiff 生成小尺寸缓存缩略图
- 命令面板：`Ctrl+Shift+P`
- 批量重命名：对当前筛选结果中的文件按前缀和序号重命名
- 在文件列表区域按 `Ctrl+滚轮`：按 Details、List、Columns、M Icons、L Icons、XL Icons 的顺序切换查看方式，不影响侧栏、地址栏和工具栏
- 底部状态栏：显示当前面板状态、选中数量、已知选择大小、查看方式、排序方式
- 设置项：显示隐藏文件、显示/隐藏文件扩展名
- 单实例：重复启动 FileFlow 时，新进程会自动退出，避免同时出现多个 FileFlow 窗口

## 重要行为说明

- `Ctrl+Z` 支持撤销 FileFlow 内执行的拖动移动、剪切粘贴、复制粘贴、双栏传送。同一次批量操作为一条记录；移动按 Shell 回调记录的实际路径移回原位置，复制只将新生成的副本移入回收站。重名自动改名后的路径也会记录；原位置被占用时不覆盖，失败项目保留记录供重试。文件操作进行中暂不允许撤销。记录上限 32 条，仅在本次运行期间有效，不能追溯升级前未记录的操作；外部应用执行的操作不属于本栈。

- `Alt+Enter` 与右上角最大化按钮共用最大化/还原逻辑，使用屏幕工作区并保留任务栏，还原时恢复原窗口尺寸。`Win+E` 只唤起 FileFlow，保留最大化状态；隐藏到托盘后唤起也不还原窗口大小。
- `Ctrl+Shift+T` 按关闭时间逆序恢复本次运行最近 32 个已关闭标签，回到原栏、原位置并激活。恢复右栏标签时自动显示双栏；最后一个标签不允许关闭，无关闭记录时快捷键不执行操作。关闭记录不跨程序重启保存。
- 复制、移动、删除使用 Windows Shell `IFileOperation`。删除设置 `FOFX_RECYCLEONDELETE` 和撤销记录，不启动 PowerShell，也不直接永久删除。
- 重命名和批量新建文件夹也在 GPUI 后台执行器中完成，网络目录不会占住界面线程。
- 剪贴板优先发布 Windows Shell `IDataObject`，并保留 `CF_HDROP` 兼容回退，以便 QQ、网盘和其他 Windows 软件接收文件。
- 缩略图请求进入有界后台工作队列；视图渲染只读取缓存，不在 paint/render 路径解码图片。
- 目录加载使用左右栏独立 generation token。用户连续切换目录时，较早返回的慢网络请求不会覆盖当前路径。
- Win+E 接管通过运行时全局快捷键实现：启用且注册成功后，按 `Win+E` 会唤醒已有 FileFlow 窗口；若 Windows 保留该组合键，设置页会显示未注册状态并交还给系统。
- 设置持久化会保存双栏、查看模式、左右路径、排序方式、预览面板、字体大小、行距、缩略图策略、交替行、单击打开、多选框、隐藏文件显示、扩展名显示、Win+E 接管开关和快捷键。
- Windows 发布版使用 GUI 子系统链接，正常双击启动不会再弹出 cmd 黑窗口。

## 视觉方向

当前采用 File Pilot/FileFlow 风格：

- 背景：白色 `#FFFFFF`
- 侧栏/工具栏：浅灰 `#FAFBFC`
- 主强调色：FileFlow 蓝 `#14AEE8`
- 选中/悬停：浅蓝高亮
- 信息密度：偏高，行距小，适合文件列表
- 字体：Sarasa UI SC，优先保证中文清晰

避免方向：

- 不做大面积渐变
- 不做卡片式仪表盘
- 不做厚重阴影
- 不使用 emoji 作为核心文件图标，Windows 字体覆盖不稳定

## 下一步建议

优先级 1：

- 设置窗口持久化更多选项：行高、默认启动路径、缩略图策略
- 地址栏下拉增加过滤框真实状态，而不是每次临时空输入

优先级 2：

- 详情/列表缩略图：jpg/png/webp，本地磁盘优先，网络盘可配置
- 文件图标：调用 Windows Shell 图标缓存
- 文件夹大小：当前支持右键手动计算；后台可选扫描仍建议后续做成本地盘默认关、网络盘强烈建议默认关
- 多选：当前仍是单选，复制/删除/重命名面向单个选中项，批量重命名按当前筛选结果工作

优先级 3：

- Win+E 接管与单实例
- 配置导入/导出
- 多标签
- 预览面板：图片、文本、JSON、Markdown
- MTP/手机设备
- 真正的 Win+E 接管安装/卸载：需要受控写入注册表，并提供恢复 Windows 默认资源管理器的入口
- 单实例激活已有窗口：当前重复启动会退出，但还不会把已打开窗口拉到前台，后续可用 IPC 或 Win32 FindWindow 实现

## 给后续 Agent 的注意事项

- 不要把 WPF 版代码直接搬过来。WPF 和 egui 交互模型不同，应按 Rust 状态驱动 UI 重写。
- 文件操作要保守。涉及删除、移动、覆盖时优先做确认或回收站。
- 网络目录枚举已在后台执行；新增功能不得在 GPUI render/event 回调中直接递归扫描或解码图片。
- 字体已内嵌，不要移除 `SarasaUiSC-Regular.ttf`，否则 `include_bytes!` 会编译失败。
- 如果要拆文件，先保持行为不变，再拆模块。
- 每次修改后至少运行：

```powershell
cargo check --bins
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo build --release
```

当前自动测试覆盖外部路径/UNC 解析、框选矩形相交、快捷键合法性与冲突基础、缩略图缓存键。真实系统剪贴板测试标记为 `ignored`，需要人工独占剪贴板时执行。

## 当前运行入口

```text
C:\Users\Administrator\FileFlow\target\release\fileflow.exe
```
