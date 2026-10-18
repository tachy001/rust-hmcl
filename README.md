# 星屿 Aster Launcher

一个以 Rust 为核心的 Minecraft Java 版桌面启动器。原创界面使用 Tauri、Vue、TypeScript 和 CSS，支持浅色／深色主题、浮岛场景与可关闭的动画。启动器本身无需 Java；Minecraft 仍使用独立 Java 环境，可在启动器中检测或下载。

项目面向 Windows、Linux 和 macOS，目前为开发预览版。内部包名和数据目录保留 rust-hmcl，以兼容已有配置。

## 使用

Windows 发布目录为 `dist/aster-windows-x64/`。解压完整 ZIP 后运行 `AsterLauncher.exe`，请保留同目录的 `WebView2Loader.dll`。Windows 需要 WebView2 Runtime。

1. 在侧栏底部的账户入口添加离线用户名。
2. 在“下载”选择 Minecraft 版本并安装。
3. 在首页选择游戏，或进入“游戏库”管理实例。
4. 在 Java 管理中检测或下载运行环境，在设置中调整内存、目录和游戏窗口。

首页使用真实账户、已安装实例和配置，支持收藏与快捷启动。游戏库支持搜索、收藏筛选、复制、重命名和实例独立设置。界面还提供 Fabric／Quilt 安装、模组启禁用与本地筛选、Modrinth 兼容模组搜索及安装、资源包兼容性确认、任务取消及日志。

在线模组：管理一个 Fabric／Quilt 实例，进入“模组管理 → 发现模组”搜索。结果按 Minecraft 与加载器筛选，安装兼容版本及其必需依赖；原版实例会先提示安装加载器。

Microsoft 登录、第三方认证、Forge／NeoForge 以及部分旧界面功能仍待实现或迁移。离线账户不能进入正版认证服务器。自动验证使用隔离的手工样本，不代表已经完成真实游戏端到端验收。

## 开发

需要 Rust、Node.js 和对应系统的 [Tauri 构建依赖](https://v2.tauri.app/start/prerequisites/)。

```sh
npm ci
npm run desktop
```

构建与验证：

```sh
npm run build
cargo test -p rust-hmcl --lib --no-default-features --locked
cargo build -p rust-hmcl --bin hmcl-service --no-default-features
npm run test:ui
npm run tauri build -- --no-bundle
# Windows：隐藏原生窗口，不操控桌面鼠标
npm run test:native
```

本机使用父目录构建缓存时，设置 `CARGO_TARGET_DIR=../target`。Windows 打包运行 `./scripts/package-windows.ps1 -TargetDir ../target`。测试默认从父目录 target 查找二进制，详见 [架构与验证说明](docs/WEB-MIGRATION.md)。

## 项目与来源

- `src/`：Rust 核心、配置、下载、Java、实例和测试。
- `src-tauri/`：原生桌面入口。
- `frontend/src/`：Aster 界面、控件及原创 SVG 场景。
- `assets/aster/`：原创品牌素材。
- `assets/hmcl/`：保留使用的游戏图标、皮肤等素材与来源说明。
- `src/main.rs`、`vendor/egui/`：迁移期间保留的旧界面及补丁，不是 Aster 的默认桌面入口。

设计方向见 [ASTER-DESIGN.md](docs/ASTER-DESIGN.md)。HMCL 和 PCL 是研究参考，Aster 是独立项目；PCL 源码及资源没有移入项目。来源见 [参考索引](docs/REFERENCE-SOURCES.md)、[HMCL 素材说明](assets/hmcl/ATTRIBUTION.md)和 [原创素材说明](assets/aster/ATTRIBUTION.md)。项目许可证见 [LICENSE](LICENSE)。
