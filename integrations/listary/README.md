# Listary Quick Switch

这个插件让 Listary 6 读取 FileFlow 当前活动栏的目录。打开 Affinity 等软件的文件选择窗口后，切换到 FileFlow，再切回文件选择窗口，Listary 可跳转到 FileFlow 的当前目录。

插件使用 Listary 官方 `IFileAppPlugin / IGetFolder` 接口，通过 FileFlow 的本机进程通信读取活动栏快照，不读取剪贴板，不模拟按键，不触发 FileFlow 导航。此电脑、网络邻居和回收站等虚拟页面不返回文件系统路径。

## 安装

需要已安装 Listary 6、Windows .NET Framework 4.8，以及包含目录查询接口的新版 FileFlow。

在仓库根目录执行：

```powershell
.\integrations\listary\install.ps1
```

脚本使用已安装的 Listary SDK 编译插件，安装到 `C:\Program Files\Listary\FileAppPlugins\FileFlow`，备份并更新当前用户的 Listary 配置，然后重启原先运行中的 Listary。插件启用 Quick Switch 目录来源，关闭它在 FileFlow 中的即时键入搜索和鼠标双击/中键接管，以保留 FileFlow 的既有交互。其他配置保持原值。

如果 Listary 安装在其他位置，传入 `-ListaryDirectory`。只有插件需要 .NET Framework，FileFlow 主程序仍是 Rust 原生程序。Listary 升级后如果自定义插件目录被移除，重新运行安装脚本。

## 验证

Listary 设置的「集成」页面应显示并启用 FileFlow。真实验收须检查：左/右栏各自浏览目录、切换标签后进入文件选择窗口、中文/空格路径、关闭 FileFlow、虚拟页面，以及当前 Listary Quick Switch 设置。

FileFlow 的 TCP 查询请求为 `fileflow-query:current-directory:v1`，向 `127.0.0.1:47719` 发送 UTF-8 文本并关闭发送方向。响应为带 `version`、`window_handle`、`path` 的 JSON；插件核对窗口句柄，避免把其他窗口的目录当作当前目录。

官方接口与开发指南：

- https://github.com/listary/Listary.FileAppPlugin
- https://github.com/listary/Listary.FileAppPlugin/blob/master/docs/Getting%20Started.md
