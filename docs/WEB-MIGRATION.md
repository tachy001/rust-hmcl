# Rust 核心与 Web 界面迁移

用户已将目标调整为原创启动器“星屿 Aster”，见 `ASTER-DESIGN.md`。
当前使用 Rust + Tauri 2 + Vue 3 / TypeScript / CSS，迁移完成前保留
Tauri/Vue 入口是当前唯一桌面入口。不能把界面更新或一次构建成功视为全部功能验收。

当前开发分工调整为后端优先：按对象职责逐步拆分，前端仅做必要的类型与调用接入。
加载器版本目录的对象边界和接口契约见 [后端结构与接口](BACKEND-ARCHITECTURE.md)。

任务日志已升级为信号台式日志中心，支持筛选、暂停跟随、任务报告和保守启动诊断，
详见 `LAUNCH-DIAGNOSTICS.md`。组件合成样本与真实 Service、原生 IPC 验证分开记录。

实际 Mojang／Fabric／Quilt 安装、改名身份保留、客户端损坏修复与文件校验已另行执行，
见 `INSTALL-VERIFICATION.md`；文件准备和启动参数通过不代表已启动并验收游戏窗口。

2026-10-18：世界星图新增轨道流光、分层视差、坐标快捷入口及操作动效。
已通过真实 Service 的 Web 验证（包含轨道时间采样、视差复位、快捷导航和减少动画），
以及 Windows 发布程序的 17 项隐藏 WebView2 检查。54 项核心测试通过，4 项联网测试跳过。
本次 Windows 发布使用 Tauri CLI 嵌入资源；普通 Cargo 发布构建不能代替这一打包步骤。

Java 管理提供手动路径注册、全局默认及隐藏／恢复，下载校验后暂存提交，
过滤架构不匹配环境。独立的 `node scripts/test-java-online.mjs` 会主动联网并读取 Java 版本；
它与常规不执行 Java 的 UI 测试分开运行。当前真实 Temurin 17 JRE 登记与重复安装复用验证通过，
未启动 Minecraft，结果在 `dist/java-online-proof.json`；这不替代各平台真实游戏启动验收。

## 结构

- `src/lib.rs`：共用 Rust 核心，默认构建不依赖桌面 UI 框架。
- `src/service.rs`：配置、实例目录、Java、任务状态、取消、模组和资源包的命令边界。
- `src-tauri/`：Tauri 桌面入口，调用同一个 Service；阻塞操作放入工作线程。
- `frontend/src/`：Aster 原创布局、主题、品牌与浮岛场景，保留有来源记录的 HMCL 游戏图标和皮肤等素材。
- `src-tauri/src/main.rs`：Tauri 桌面入口与命令边界。
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

Windows 单 EXE 发行必须使用 MSVC 工具链。上游 `webview2-com-sys` 在 MSVC 下静态链接
WebView2 加载器，GNU 下仍动态导入 DLL。`scripts/verify-windows-exe.mjs` 检查普通和延迟
PE 导入表，发现外置加载器则拒绝发行；打包不再复制 DLL，并移除输出目录中残留的旧加载器。
原生验证将 EXE 单独复制到隔离目录，在该目录启动隐藏窗口并检查正常 IPC，避免构建目录
中的 DLL 掩盖依赖。此变化不包含系统 WebView2 Runtime。

从已验证 CI 产物打包时可传 `-BinaryRevision <构建提交>`，清单分别记录程序源码提交与
打包脚本提交，避免将旧二进制误标为新构建。`-OutputName` 可指定新的发行目录，保留正在运行的旧程序。

Windows 构建后可运行 `./scripts/package-windows.ps1`；使用父目录缓存时
传入 `-TargetDir ../target`。脚本将程序及存在时必需的 GNU WebView2Loader.dll
一起放入 `dist/aster-windows-x64/`，程序命名为 `AsterLauncher.exe`，
生成带修订号的 ZIP、源代码修订号和文件校验记录。
不能单独搬走 GNU 构建的 EXE 而遗漏加载器 DLL。

## 当前迁移覆盖

前端接入真实 Rust 命令的页面包括：首页、游戏列表、离线账户、官方版本列表、
Java 探测与下载、全局设置、游戏任务与日志、Fabric / Quilt、实例模组启禁用、
资源包列表与兼容性确认启禁用。后台沿用现有下载校验与启动逻辑。

模组中心新增本地名称筛选、启用数量与空状态，Modrinth 搜索及后台安装。
先从显式实例解析 Minecraft／Fabric／Quilt 环境，在构建网络请求前拒绝原版、
非法实例名、空查询与非法项目 ID；安装复用既有依赖、下载校验和任务取消流程。
独立组件在离开页面后忽略迟到的搜索响应，实例切换时重建搜索状态。

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

外壳改为透明原生窗口和透明网页底色，统一圆角裁切包含浮层；Windows 关闭会
产生 1px 白边及系统二次圆角的原生阴影。最大化或全屏时使用直角，恢复窗口后
恢复圆角，通过原生尺寸变化事件同步。macOS 已启用透明窗口所需的 Tauri 特性。
四个渲染角的外侧像素均为透明，边缘包含抗锯齿过渡；可见合成器效果及角落命中
区域仍需实际窗口验收。最大化／全屏状态切换使用事件样本验证，未显示原生窗口。

分页曲线取自原 `Motion.EASE_IN_OUT_CUBIC_EMPHASIZED`：新页面 400 ms
淡入和按视口高度 20% 上滑，旧页面 200 ms 退出；过渡通过 HTML `inert`
阻止输入，不以灰化控件实现。减少动画设置和系统减少动画偏好均生效。

尚未完成迁移的已有功能包括：截图、资源包图标与批量选择／
批量启禁用、Java
登记隐藏等。原项目还未补齐的 Microsoft／第三方认证、Forge／NeoForge、
世界／原理图管理和完整导出等也仍待补齐。原先 HMCL 还原范围保存在历史文档中。

Web 集成测试通过真实 Service 写入独立手工样本，检查账户和设置持久化、
模组文件改名、资源包确认与 options.txt 写入。它代替 Tauri IPC 使用 stdio
传输，所以不能作为原生 WebView / IPC / 安装器 / 三端运行的验收证据。

当前 Aster 的 Windows GNU Tauri 发布构建、Web 类型检查和生产资源构建通过；
UI 无依赖的 Rust 核心测试 51 项通过、4 项联网测试跳过。31 项 Web
集成检查通过，新增收藏持久化与筛选、场景动画、版本选择键盘交互。

17 项真实 Tauri / WebView2 检查通过：整合包缓存安装、本地导入、模组筛选与环境校验、首页收藏、隐藏窗口、实际资源和 CSP、IPC、
错误输入不落盘、账户与全局设置保存、主题、模组启禁用、资源包确认以及原生后台
复制任务及完成刷新。截图已查看；测试使用手工样本，未启动 Java 或游戏。
本轮 150% DPI 下原生客户区 1598×994，页面 1066×663 CSS 像素。
验证还记录真实字体渲染结果，本机中文侧栏使用 Microsoft YaHei、字号 13px。
隐藏窗口的渲染不能证明可见窗口的边框、圆角和最大化行为。

本地导入已接入系统文件选择及 Tauri 拖入事件、待导入列表和后台任务。
Web 检查模拟选择器返回值，原生检查通过合成事件验证真实 IPC 与文件复制；
系统选择窗口和物理拖动尚未实际操作，验证过程中没有显示选择器或发送桌面输入。
复制、取消及批次部分完成的行为见 `LOCAL-IMPORTS.md`。

整合包已接入预览、后台安装、独立实例设置与暂存清理；样本从不执行 Java，
远程游戏／加载器下载与实际游戏启动仍待验证，详见 `MODPACKS.md`。

提交 `b1d668a` 的 [远端构建](https://github.com/tachy001/rust-hmcl/actions/runs/39103992750)
已核对 Windows、Linux、macOS ARM／Intel 四个 Tauri 构建与旧入口构建通过；
Windows 原生验证因 CDP 调试器就绪时尚无网页目标而失败。当前验证脚本已改为
等待同一存活进程与连接的目标出现，修复后的远端结果需独立核验，不能用本机通过代替。

可选联网验证命令为 `node scripts/test-web-ui.mjs --online-modrinth`：
通过完整界面在隔离的手工 Fabric 1.20.1 样本上搜索官方 Modrinth，
安装 Fabric API，并核对登记文件、实际字节与 SHA-1。已实际通过；本轮下载
`fabric-api-0.92.12+1.20.1.jar`，2,137,232 字节，SHA-1 为
`3e9cdd3e2f827ca9a259df9eb8e31949437b6bd4`。结果及普通／窄窗口截图
存于 `dist/web-ui-proof/`。常规测试不联网搜索、不下载模组，联网结果字段
`modrinthProof` 为空；此验证没有启动 Java 或执行下载的模组。

本轮联网回归曾遇到 CDN TLS 握手中断，正常证书校验保持开启；检查服务恢复后
复验成功。成功摘要另保存在 `dist/modrinth-online-proof.json`，以免后续常规
不联网验证覆盖该次下载结果。

提交 `9bce410` 的 [Windows CI](https://github.com/tachy001/rust-hmcl/actions/runs/39033146363/job/117397546262)
已完成发布构建和同一原生验证脚本，全部通过，并上传原生 WebView2 截图与记录。

提交 `b590705` 的 [GitHub 构建](https://github.com/tachy001/rust-hmcl/actions/runs/39032592512)
已确认新版 `web-desktop` 在 Windows、Ubuntu 24.04、macOS ARM 和 Intel 的核心测试、
前端构建与 Tauri 发布构建全部通过。这是构建证据，Linux/macOS 实际运行仍待验证。
