# 星屿 Aster：桌面平台

启动器使用 Rust + Tauri，本身不需要 Java。启动 Minecraft 或运行 Forge / NeoForge 安装器仍需要对应版本的 Java，可由启动器下载并管理。

| 系统 | 架构与最低目标 | 分发形式 |
| --- | --- | --- |
| Windows | Windows 10 / 11，x64 | ZIP 内的 `AsterLauncher.exe`，静态链接 WebView2 Loader，无需附带 Loader DLL；系统仍需 Microsoft Edge WebView2 Runtime |
| macOS | macOS 13 及以上，Apple Silicon / Intel 分别构建 | `.tar.gz` 内的 `Aster Launcher.app`，解压后移到 Applications |
| Linux | x64，Ubuntu 22.04 及以上作为构建基线 | `.deb` 和便携 `.tar.gz`；依赖 GTK 3、WebKitGTK 4.1 及系统图形环境 |

macOS 应用目前使用临时签名，未做 Apple Developer ID 签名及公证，系统可能要求在隐私与安全性设置中批准打开。请使用支持保留 Unix 权限与符号链接的解压工具，不要在 Windows 解压后再复制应用目录。

Debian / Ubuntu 可用 `sudo apt install ./aster-launcher-linux-x64-<版本>.deb` 安装并解析依赖。Linux 便携包不会内置系统 WebKit / GTK，也不代表所有发行版、桌面环境或 Wayland 组合均已验证。

## 平台处理

后端的 `Platform` 对象统一处理系统名称、处理器架构、Java 下载目标、可执行文件名称和 classpath 分隔符。Java 发现包括 Windows 常见发行商目录、macOS 系统及用户 JDK / Homebrew 目录、Linux 常见 JDK 目录，以及 SDKMAN / asdf。macOS 的 `/usr/bin/java` 是系统提示入口，不作为可用 Java 执行。

配置沿用各平台系统数据目录；`RUST_HMCL_DATA_DIR` 可显式指定独立数据目录。已有游戏目录不因平台适配而迁移或删除。

## 构建与验证

安装 Node.js、Rust 及 Tauri 对应平台的系统构建依赖后，执行 `npm ci`，再执行：

- Windows：`npm run tauri build -- --no-bundle`，使用 MSVC 工具链生成可单 EXE 分发的版本。
- macOS：`npm run tauri build -- --bundles app`。
- Linux：`npm run tauri build -- --bundles deb`。

流水线分别在 Windows、macOS ARM64、macOS Intel 和 Ubuntu 22.04 原生构建。除后端测试外，还运行真实隐藏 WebView 的页面加载、资源读取、常规 IPC、配置读写、系统架构及右键菜单抑制检查，并实际下载和探测匹配平台的 Java。Linux 隐藏窗口检查使用 Xvfb，不操作用户桌面。

`npm run test:platform` 可对 `HMCL_DESKTOP_BIN` 指定的生产构建运行独立配置检查；`npm run package:platform` 在干净源码树上打包，要求待分发程序与原生检查记录的 SHA-256 一致。各平台产物及校验和保存在 `dist/distributions`，流水线提供 `aster-distribution-*` 和 `platform-proof-*` 下载。

测试记录才代表对应提交的实际结果。编译成功不等于图形界面实测；隐藏窗口检查也不覆盖可见合成效果、实体输入、游戏渲染、游戏启动或账号认证。这仍是开发预览版，GitHub 已发布版本不会因源码更新自动替换。
