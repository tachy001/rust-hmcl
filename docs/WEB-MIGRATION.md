# Rust 核心与 Web 界面迁移

目标仍是完整还原参考 HMCL 的功能、界面和交互。当前切换到 Rust +
Tauri 2 + Vue 3 / TypeScript / CSS，迁移完成前保留 egui 入口作为回归参照。
不能把 Web 原型的外观或一次构建成功视为完整还原。

## 结构

- `src/lib.rs`：共用 Rust 核心；`--no-default-features` 不依赖 egui。
- `src/service.rs`：配置、实例目录、Java、任务状态、取消、模组和资源包的命令边界。
- `src-tauri/`：Tauri 桌面入口，调用同一个 Service；阻塞操作放入工作线程。
- `frontend/src/`：真实 Web 界面，使用原 HMCL SVG / PNG / 壁纸。
- `src/main.rs`：迁移期间保留的旧版 egui 入口。
- `src/bin/hmcl-service.rs`：仅用于无窗口集成测试的 JSON-lines 入口，
  桌面版不启动它、不开放本地 HTTP 后端端口。

核心沿用原有配置位置和 `RUST_HMCL_DATA_DIR` 隔离方式。Minecraft 仍由
Rust 启动独立 Java 进程；新启动器使用系统 WebView，不要求 Java。
在 Windows 上使用 WebView2，Linux / macOS 使用对应系统 WebView。

## 构建

安装 Node.js 与 Tauri 平台构建前提，具体见官方说明：
https://v2.tauri.app/start/prerequisites/ 。Windows 官方建议 MSVC 工具链。

```sh
npm ci
npm run build
npm run desktop
```

`npm run desktop` 会打开可见窗口，应由用户主动运行。开发检查优先使用：

```sh
cargo check -p rust-hmcl --lib --no-default-features
cargo test -p rust-hmcl
cargo check -p rust-hmcl-desktop
cargo build -p rust-hmcl --bin hmcl-service --no-default-features
npm run test:ui
# Windows / WebView2, using the real release executable in a hidden window:
npm run test:native
```

本机使用父目录 target 缓存时，为 Cargo 命令追加 `--target-dir ../target`。
UI 测试默认从父目录 `target/debug/` 查找服务程序，也可用环境变量
`HMCL_SERVICE_BIN` 指定。Windows 测试使用 Edge 的 headless 模式；其他
平台需安装 Playwright Chromium。测试只操作浏览器内部虚拟输入，
不读取桌面、不移动用户鼠标、不启动 Minecraft。

原生验证默认使用父目录 `target/release/rust-hmcl-desktop.exe`，也可用
`HMCL_DESKTOP_BIN` 指定。它创建项目内 `.tools/native-ui-proof-*` 独立配置，
使用同一个发布程序的 `--verify-hidden-ui` 开关，在创建前关闭窗口可见性和焦点，
分配独立 WebView2 配置目录与临时调试端口。测试通过浏览器内部协议操作真实
WebView2，调用正常 Tauri 命令，不替换 IPC、不增加测试命令，不发送桌面输入。
运行记录、程序 SHA-256、实际 DPI / 客户区尺寸及渲染截图保存在
`dist/native-ui-proof/`。正常启动不会启用隐藏模式或调试端口。

打包资源构建：`npm run tauri build -- --no-bundle`。目前 bundle 关闭，
安装器、应用签名、三端可见窗口运行、原生窗口圆角和 WebView 分发均需另行验收。

Windows 构建后可运行 `./scripts/package-windows.ps1`；使用父目录缓存时
传入 `-TargetDir ../target`。脚本将程序及存在时必需的 GNU WebView2Loader.dll
一起放入 `dist/web-windows-x64/`，并生成 ZIP、源代码修订号和文件校验记录。
不能单独搬走 GNU 构建的 EXE 而遗漏加载器 DLL。

## 当前迁移覆盖

前端接入真实 Rust 命令的页面包括：首页、游戏列表、离线账户、官方版本列表、
Java 探测与下载、全局设置、游戏任务与日志、Fabric / Quilt、实例模组启禁用、
资源包列表与兼容性确认启禁用。后台沿用现有下载校验与启动逻辑。

实例独立设置编辑器已接入现有配置和启动逻辑，覆盖 Java、内存、窗口尺寸、
全屏、隔离、自动下载 Java 和 JVM 参数；关闭单独设置会继承全局值，
恢复全局设置在显式保存后才写入配置。切换主题保留尚未保存的编辑。

游戏列表的更多按钮和右键菜单已接入测试启动、管理、重命名与复制，支持方向键、
Home / End / Esc 和焦点返回。复制按原版默认不包含存档，可单独勾选；读取有效
游戏目录，副本改用独立目录，排除公共库、资源、日志与启动器账户文件。
重命名保留实例设置、收藏和存档，并更新依赖实例的继承关系；同名目标拒绝覆盖。

控件已统一为主题化的复选框、单选框、开关、填充输入框和菜单；补上开关滑动、
焦点下划线、弹窗进出、悬停反馈与保存状态。头像沿用原版默认皮肤选择规则，
分层绘制脸部与帽子。浅色、深色和 800×560 布局已通过无窗口截图检查。

分页曲线取自原 `Motion.EASE_IN_OUT_CUBIC_EMPHASIZED`：新页面 400 ms
淡入和按视口高度 20% 上滑，旧页面 200 ms 退出；过渡通过 HTML `inert`
阻止输入，不以灰化控件实现。减少动画设置和系统减少动画偏好均生效。

尚未完成迁移的已有功能包括：截图、资源包图标与批量选择／
拖放／导入、模组在线搜索及导入、整合包导入、Java
登记隐藏等。原项目还未补齐的 Microsoft／第三方认证、Forge／NeoForge、
世界／原理图管理和完整导出等也仍属于目标范围。不能因此缩小验收范围。

Web 集成测试通过真实 Service 写入独立手工样本，检查账户和设置持久化、
模组文件改名、资源包确认与 options.txt 写入。它代替 Tauri IPC 使用 stdio
传输，所以不能作为原生 WebView / IPC / 安装器 / 三端运行的验收证据。

Windows GNU 的 Tauri 发布构建与严格静态检查通过；Rust 测试合计
60 项通过、4 项联网测试跳过。Web 类型检查、生产资源构建及 22 项无窗口
集成检查通过。

新增 11 项真实 Tauri / WebView2 检查通过：隐藏窗口、实际资源和 CSP、IPC、
错误输入不落盘、账户与全局设置保存、主题、模组启禁用、资源包确认以及原生后台
复制任务及完成刷新。截图已查看；测试使用手工样本，未启动 Java 或游戏。
本机 150% DPI 下原生客户区 1620×1007，与页面 1080×672 CSS 像素匹配。
验证还记录真实字体渲染结果，本机中文侧栏使用 Microsoft YaHei、字号 13px。
隐藏窗口的渲染不能证明可见窗口的边框、圆角和最大化行为。

提交 `9bce410` 的 [Windows CI](https://github.com/tachy001/rust-hmcl/actions/runs/39033146363/job/117397546262)
已完成发布构建和同一原生验证脚本，全部通过，并上传原生 WebView2 截图与记录。

提交 `b590705` 的 [GitHub 构建](https://github.com/tachy001/rust-hmcl/actions/runs/39032592512)
已确认新版 `web-desktop` 在 Windows、Ubuntu 24.04、macOS ARM 和 Intel 的核心测试、
前端构建与 Tauri 发布构建全部通过。这是构建证据，Linux/macOS 实际运行仍待验证。
