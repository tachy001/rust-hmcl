# Rust HMCL 0.3.0

参考 HMCL 的 Rust 原生桌面启动器。当前目录是独立的 Rust 项目根目录；本机参考用的原 Java 源码保留在相邻的 `../HMCL-main/`。启动器无需 Java 或 WebView，Minecraft 仍由单独的 Java 进程运行。

## 项目结构

```text
rust-hmcl/
├── .github/workflows/  三端构建配置
├── assets/hmcl/        界面图标、壁纸、配色和来源说明
├── src/               Rust 启动器与单元测试
├── dist/              本地发布包（Git 忽略）
├── Cargo.toml         项目与依赖配置
├── Cargo.lock         锁定依赖版本
├── CHANGELOG.md        更新记录
└── LICENSE            许可证
```

构建生成的 `target/`、本地发布包与日志不纳入 Git。源码、资源、锁定文件和 CI 配置由仓库管理。

## 运行和使用

运行 dist/v0.3.0/rust-hmcl.exe，或解压发行包后运行其中的 rust-hmcl.exe。

1. 在“账户”添加或切换离线用户名。
2. 在“下载游戏”安装正式版或快照，在“首页”选择版本并启动。
3. Java 路径留空时自动检测合适环境；启用自动下载后，缺少 Java 会下载 Temurin。
4. 在“版本管理”为原版安装 Fabric 或 Quilt，可指定加载器版本。
5. 在“模组管理”导入 .jar、启用/禁用模组，或搜索当前 Minecraft/加载器兼容的 Modrinth 模组。在线安装包含必需依赖，不自动安装可选依赖。
6. 在“整合包”导入原版、Fabric 或 Quilt 的 .mrpack，指定新的实例名称；可选择是否安装可选客户端文件。
7. 在“设置”选择游戏目录、切换记住的目录，设置内存、窗口、JVM 参数、并发和深色主题。

有效缓存文件直接复用，支持已安装依赖的离线校验与启动；缺少文件、加载器配置、Java 或模组时仍需网络。离线账户不能进入正版认证服务器。

## 功能

- 复用 HMCL 原始 SVG、PNG 游戏图标、Logo 和壁纸；采用原主题蓝色，分组侧栏、右下角启动区、无边框窗口和浅色/深色主题。
- 多离线账户、正确的离线 UUID、多个游戏目录。
- 官方版本列表和缓存回退，原版安装、修复和快照筛选。
- 1–32 路资源并发，大文件进度、速度、取消、重试和 SHA-1 校验。
- Fabric/Quilt 官方配置、继承解析、Maven 依赖和基础客户端 JAR 复用。
- Temurin Java 下载、SHA-256 校验，JAVA_HOME/PATH 和受管 Java 检测。
- 本地模组导入、无损启用/禁用，Modrinth 搜索、兼容版本及必需依赖下载。
- .mrpack 客户端文件、通用/客户端 overrides、可选文件和中断恢复。
- 日志筛选、跟随、复制、打开文件、中文输出和结束游戏。
- 配置迁移与备份，原生文件选择和 Windows Java 路径修复。

## 0.3.0 界面更新

图标路径直接取自 HMCL 的 SVG.java，PNG 和壁纸来自原 resources/assets/img，来源记录见 assets/hmcl/ATTRIBUTION.md。新增按钮悬停渐变、按压反馈、点击水波纹、侧栏选中动画、开关滑动和 220 ms 页面淡入/位移。资源内嵌在程序中，不需要联网下载。自定义窗口标题栏支持拖动、双击最大化、最小化与关闭。

## 数据目录

配置位于系统用户数据目录中的 rust-hmcl/：Windows 通常为 %APPDATA%/rust-hmcl，Linux 为 $XDG_DATA_HOME/rust-hmcl 或 ~/.local/share/rust-hmcl，macOS 为 ~/Library/Application Support/rust-hmcl。

config.json 为配置文件，保存前备份为 config.json.bak；java/ 为下载的 Java，game.log 为完整日志。隔离游戏目录为 versions/<版本>/game；已有根目录存档不会自动迁移。整合包始终隔离。

RUST_HMCL_DATA_DIR 环境变量可指定独立数据目录。本机此前的隔离验证目录保留在 `../.tools/smoke/`。

## 构建

使用 Rust stable：

```sh
cargo run --locked
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo build --release --locked
```

Windows MSVC 开发需要 C++ Build Tools，GNU 开发需要 MinGW-w64。Linux 开发依赖 libxkbcommon-dev、libwayland-dev、libgl1-mesa-dev；文件选择需桌面 portal，中文显示推荐 fonts-noto-cjk。macOS 需 Xcode Command Line Tools。发布程序无需开发工具链，仍需系统图形库。

本机已有 Windows 工具链保留在相邻的 `../.tools/`。从当前项目目录运行 PowerShell：

```powershell
$taskTools = [IO.Path]::GetFullPath((Join-Path $PWD '../.tools'))
$env:RUSTUP_HOME = Join-Path $taskTools 'rustup'
$env:CARGO_HOME = Join-Path $taskTools 'cargo'
$env:PATH = "$taskTools/cargo/bin;D:/mingw64/bin;$env:PATH"
cargo run --locked
```

调试程序提供 --install、--fabric、--quilt、--java、--launch、--mod、--pack、--list；执行 target/debug/rust-hmcl.exe --help 查看。Windows 发布版为 GUI 程序，脚本调用时应显式等待进程结束。

## 验证

Windows 开发/发布构建、严格静态检查、18 项本地测试通过。Minecraft、Adoptium、Fabric/Quilt、Modrinth 的四项联网测试通过；执行 cargo test --locked -- --ignored 可重复联网测试。

实际安装 Minecraft 1.20.1、3,575 个资源、Java 17 和 Fabric Loader 0.19.5。游戏成功打开初次启动界面，Fabric、LWJGL、音频和资源初始化正常。Fabric API 已从 Modrinth 下载并校验。实际导入测试 .mrpack，验证继承和客户端 overrides 优先级。

0.3.0 首页、图标、壁纸和侧栏已检查；其余页面巡检因用户按 Escape 停止窗口操作而未继续。Linux/macOS 已配置独立构建，尚未在真实系统实测。Quilt 尚未完成实际游戏启动验证。

## 当前边界

未实现微软登录、第三方认证、Forge/NeoForge 安装、CurseForge/HMCL 整合包格式、完整模组冲突分析、下载镜像、启动器更新、旧 HMCL 配置整体迁移。可读取现有继承配置，但不承诺 Forge 等环境可启动。

Modrinth 更新会将此前登记的同项目旧模组改名禁用并保留文件；不同内容的未登记同名文件不会被覆盖。整合包下载来源限制为 Modrinth CDN、GitHub、GitLab，overrides 解压总量限 2 GB。OS 版本正则规则尚未实现，某些旧版 ARM 游戏缺少 native 依赖，不承诺全部历史版本和 CPU 架构兼容。

## 许可与来源

Hello Minecraft! Launcher — Copyright (C) huangyuhui and contributors.
原项目：https://github.com/HMCL-dev/HMCL 。本项目使用 Rust HMCL 名称，保留版权声明，采用 GPLv3 或更新版本并保留原 LICENSE 的附加条款，详见 LICENSE。

游戏下载使用 Mojang 官方服务器，Java 使用 Adoptium API，加载器使用 Fabric/Quilt 官方元数据，模组使用 Modrinth API。相关资源许可归各自权利人。
