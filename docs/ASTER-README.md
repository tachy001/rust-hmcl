# 星屿 Aster Launcher · 开发预览版

解压全部文件后运行 AsterLauncher.exe。请保留同目录的 WebView2Loader.dll；Windows 需要 WebView2 Runtime。

启动器本身无需 Java。Minecraft 仍需要 Java，可在启动器的 Java 管理中检测或下载。

首页星图上的“游戏库”和“发现新世界”可直接跳转。环境动效跟随页面活动，设置中的“减少动画和动态效果”及系统减少动画偏好会关闭首页动态效果。

Java 管理支持添加已有可执行文件、设为默认和隐藏／恢复；只有主动检测或添加时才会读取本机 Java 版本。下载优先提供 JRE，缺少对应包时使用 JDK。

首次使用：点击侧栏底部账户添加离线用户名，在“下载”安装游戏，然后回首页选择并启动。已有游戏可在设置中选择游戏目录，再到游戏库管理。

模组：管理 Fabric／Quilt 实例，进入“模组管理 → 发现模组”搜索并安装。结果按当前游戏环境筛选，包含必需依赖；本地模组可筛选、启用和禁用。

本地导入：模组管理支持选择或拖入 .jar，资源包管理支持 ZIP 和资源包文件夹。待导入列表可移除项目，再点击导入。原文件会保留；同名目标不会覆盖，资源包导入后需手动启用。

整合包：在侧栏选择或拖入 .mrpack，预览并填写新实例名后安装。支持原版、Fabric、Quilt；可选客户端文件默认不安装。新实例使用独立目录，中断后可用同一文件、同一实例名继续。

目前支持离线账户；Microsoft 登录、第三方认证及 Forge／NeoForge 尚未提供。

源代码与反馈：https://github.com/tachy001/rust-hmcl

Aster 是独立项目。原创图形说明见 ASTER-ATTRIBUTION.md，保留使用的 HMCL 素材来源见 ATTRIBUTION.md，项目许可见 LICENSE。构建修订号与文件校验值见 build-info.json。
