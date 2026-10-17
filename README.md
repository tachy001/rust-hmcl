# Rust HMCL 0.3.11

参考 HMCL 的 Rust 桌面启动器，正在重构为 Rust + Tauri + Vue / TypeScript / CSS。启动器无需 Java，Minecraft 仍由独立 Java 进程运行。新界面使用系统 WebView；旧 egui 入口在迁移期间保留。本机参考 Java 源码位于相邻的 `../HMCL-main/`。

Web 迁移说明、构建命令和未完成范围见 [docs/WEB-MIGRATION.md](docs/WEB-MIGRATION.md)。下列截图、资源包和命令行功能说明主要对应保留的 egui 版本，不能视为新 Web 界面已全部完成迁移。

截图管理支持后台 72×72 缩略图、预览、图片复制、设为壁纸、多选与确认删除。离屏检查可用 `--render-ui output.png instance dark 1080 700 screenshots`，末尾追加 `preview` 或 `delete` 可检查对应弹窗，均不会实际删除文件或写入剪贴板。

资源包管理支持后台读取 ZIP / 文件夹的图标、描述和兼容性、本地搜索、ZIP／文件夹导入及单个／批量启用禁用；不兼容包启用前弹窗确认。它修改隔离或共享游戏目录的 `options.txt`，保留其他设置、已有资源包顺序和未知内容；游戏运行期间暂不允许修改。

命令行可用 `--resourcepack-list`、`--resourcepack-import <ZIP或文件夹>`、`--resourcepack-enable <文件名>`、`--resourcepack-disable <文件名>`；启用命令是明确的修改操作，会登记不兼容包。资源包页支持拖入 ZIP 或文件夹，右键“添加资源包”可选择文件夹。使用 `RUST_HMCL_DATA_DIR` 可在独立数据目录验证。资源包列表与详情离屏预览使用 `--render-ui output.png instance light 1080 700 resources`，末尾可追加 `info` 或 `warning`；预览不修改游戏设置。

## 项目结构

```text
rust-hmcl/
├── .github/workflows/  三端构建配置
├── assets/hmcl/        界面图标、壁纸、配色和来源说明
├── src/               Rust 启动器与单元测试
├── src-tauri/         Tauri 桌面入口与命令注册
├── frontend/          Vue / TypeScript / CSS 界面
├── vendor/egui/       控件库及分页输入门控补丁、上游许可证
├── dist/              本地发布包（Git 忽略）
├── Cargo.toml         项目与依赖配置
├── Cargo.lock         锁定依赖版本
├── CHANGELOG.md        更新记录
└── LICENSE            许可证
```

构建生成的 `target/`、本地发布包与日志不纳入 Git。源码、资源、锁定文件和 CI 配置由仓库管理。

未发布的分页动画修复将输入拦截与灰色外观分开，过渡时保留页面、文字和按钮颜色；真正禁用的控件仍显示禁用样式。使用固定的 egui 0.31.1 本地源码补丁，修改与升级步骤见 `vendor/egui/PATCHES.md`。

资源包描述采用原版普通文本呈现：清除 Minecraft 格式代码，列表将多行合并成单行，详情保留换行并支持双击复制。资源包名称的原始文件名保留用于启用设置，显示标题单独清除格式代码。

资源包左侧复选框控制启用，点击行正文或图标选择资源包；Ctrl／Command 切换多选，Shift 连选，Esc 取消选择。单选时右键打开详情，多选时保留选择用于批量操作。

## 运行和使用

运行 dist/v0.3.11/rust-hmcl.exe，或解压发行包后运行其中的 rust-hmcl.exe。Windows 主窗口外沿为 12 个逻辑像素圆角，随原生 DPI 与尺寸更新，最大化时取消裁剪；Linux/macOS 外沿圆角仍待实现和验证。

全局设置和实例“游戏设置”支持窗口／全屏模式；实例可继承全局，也可单独覆盖。全屏会按原 HMCL 在游戏参数末尾添加 `--fullscreen`，保留窗口尺寸配置；最大化游戏窗口模式仍待实现。`--launch-plan <已安装实例>` 可只读输出实际 Java 参数计划，采用相同的配置继承和元数据解析，不启动 Java、下载文件或保存设置。

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

## 当前界面还原

分页连续离屏帧可用 `--render-motion <空输出目录> [light|dark] [normal|reduced] [宽] [高]` 生成，默认 1080×700；它读取当前实例，实际绘制“游戏设置 → 自动安装”，输出切换前与 0、40、80、160、240、400、500 毫秒 PNG。命令不创建原生窗口、不操作系统键鼠、不启动 Java、不安装组件、不保存配置。减少动画模式立即切换；过渡控件的额外灰化仍待还原，图片不包含原生窗口圆角裁剪。

图标路径直接取自 HMCL 的 SVG.java，PNG 和壁纸来自原 resources/assets/img，来源记录见 assets/hmcl/ATTRIBUTION.md。保留按钮悬停渐变、按压反馈、点击水波纹、侧栏选中动画和开关滑动；页面采用 320 ms 淡入与缓动位移，版本菜单带展开、收起和箭头旋转。动画默认开启，可在“外观”关闭“减少动画和动态效果”以恢复动效。资源内嵌在程序中，不需要联网下载。自定义窗口标题栏支持拖动、双击最大化、最小化与关闭。

首页按原版 MainPage 与 root.css 恢复简洁布局和右下角 230×57 分体启动区；标题栏 40、侧栏 200，全窗口壁纸透过侧栏显示。版本管理按 GameListCell 改为单选按钮、32 像素图标、两行信息及启动/更多菜单；显示真实 Minecraft 与 Fabric/Quilt 信息，右键菜单保留已有操作。外观页支持自定义壁纸、界面缩放和启动页面；版本设置未覆盖的选项沿用全局配置。

完整还原尚未完成。参照文件、待还原功能与验收要求见 docs/HMCL-PARITY.md。用户禁止电脑操控，本轮采用内存中的 egui 布局与模拟事件测试，没有操作桌面，也没有宣称通过新界面截图验证。

当前实例入口恢复为“实例管理”，账户头像采用 HMCL 内置皮肤和原 UUID 选择规则，标题图标采用原 icon-title。离屏预览发现并修复了壁纸遮盖其他控件的图层顺序问题。

实例管理采用原版侧栏结构，提供游戏设置、自动安装、模组、资源包、世界与原理图分页，截图从管理菜单进入。独立内存、Java、JVM、窗口尺寸、隔离和自动下载设置在保存后生效；不覆盖全局配置。本地内容分页目前提供文件列表与打开操作，完整世界编辑和资源包启用排序尚待还原。

Java 管理使用紧凑环境列表，支持后台发现、添加可执行文件、选择、打开目录以及隐藏/恢复。自动选择跳过隐藏环境；隐藏保留文件，受管 Java 卸载尚未实现。可用 `--scan-java` 在命令行核验实际环境。检测 Java 时进程保持隐藏并有超时；启动器启动本身不依赖 Java。

实例菜单支持复制和重命名：复制保留独立游戏目录的数据并继承原设置，共享目录仍然共享；重命名会同步其他实例的继承引用。两种操作不覆盖同名目标，重命名失败会尝试回滚；如回滚受文件系统错误阻碍，会明确报告并保留目录供恢复。含链接或特殊文件的实例暂不支持此操作。独立配置与收藏同步迁移，原游戏 JAR 字节保持不变。

命令行可用 `--duplicate <原名称> <新名称>`、`--rename <原名称> <新名称>`；这些命令实际修改当前数据目录，资源包页支持拖入 ZIP 或文件夹，右键“添加资源包”可选择文件夹。使用 `RUST_HMCL_DATA_DIR` 可隔离验证。操作对话框可用 `--render-ui output.png versions light 1080 700 duplicate` 或末尾 `rename` 生成离屏预览，预览不执行文件操作。

无需桌面操控即可生成实际 UI 的软件预览：`cargo run -- --render-ui output.png versions dark 1080 700`。支持页面 key：home、accounts、versions、mods、packs、download、java、settings、logs、about、appearance、instance。instance 页可额外指定 settings、components、mods、resources、worlds、schematics 或 screenshots 分页，例如 `--render-ui output.png instance dark 1080 700 mods`。Java 页预览会通过隐藏进程检测真实 Java；其他页面无需执行 Java。命令只读取配置，不创建窗口、不模拟系统输入、不保存用户配置；仅输出指定图片。预览用于布局检查，不能代替真实 GPU 与三端运行验证。

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

Windows 开发/发布构建、严格静态检查、34 项本地测试通过。Minecraft、Adoptium、Fabric/Quilt、Modrinth 的四项联网测试此前通过，本轮未重复联网测试；执行 cargo test --locked -- --ignored 可重复联网测试。

实际安装 Minecraft 1.20.1、3,575 个资源、Java 17 和 Fabric Loader 0.19.5。游戏成功打开初次启动界面，Fabric、LWJGL、音频和资源初始化正常。Fabric API 已从 Modrinth 下载并校验。实际导入测试 .mrpack，验证继承和客户端 overrides 优先级。

0.3.0 首页、图标、壁纸和侧栏已检查；其余页面巡检因用户按 Escape 停止窗口操作而未继续。Linux/macOS 已配置独立构建，尚未在真实系统实测。Quilt 尚未完成实际游戏启动验证。

## 当前边界

未实现微软登录、第三方认证、Forge/NeoForge 安装、CurseForge/HMCL 整合包格式、完整模组冲突分析、下载镜像、启动器更新、旧 HMCL 配置整体迁移。可读取现有继承配置，但不承诺 Forge 等环境可启动。

Modrinth 更新会将此前登记的同项目旧模组改名禁用并保留文件；不同内容的未登记同名文件不会被覆盖。整合包下载来源限制为 Modrinth CDN、GitHub、GitLab，overrides 解压总量限 2 GB。OS 版本正则规则尚未实现，某些旧版 ARM 游戏缺少 native 依赖，不承诺全部历史版本和 CPU 架构兼容。

## 许可与来源

Hello Minecraft! Launcher — Copyright (C) huangyuhui and contributors.
原项目：https://github.com/HMCL-dev/HMCL 。本项目使用 Rust HMCL 名称，保留版权声明，采用 GPLv3 或更新版本并保留原 LICENSE 的附加条款，详见 LICENSE。

游戏下载使用 Mojang 官方服务器，Java 使用 Adoptium API，加载器使用 Fabric/Quilt 官方元数据，模组使用 Modrinth API。相关资源许可归各自权利人。
