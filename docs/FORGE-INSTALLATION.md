# Forge 安装与修复

在实例“自动安装”中填写新实例名称（可选），再安装 Forge。版本留空时读取官方推荐版，
没有推荐版时使用该 Minecraft 版本的最新发布；也可填写明确的 Forge 版本。
新实例默认使用独立游戏目录，原实例的世界、模组和配置不会复制或改写。

当前接受现代安装格式：官方 `spec=1`、BootstrapLauncher 启动类，以及基础游戏要求 Java 17 或更高。
旧版 Forge 尚未接入。NeoForge 现已提供后端接口，见 [NeoForge 后端说明](NEOFORGE-INSTALLATION.md)。
`.mrpack` 的 Forge／NeoForge 依赖尚不支持，整合包入口仍只接受原版／Fabric／Quilt。

安装器使用单独的 Java 进程；启动器本身仍无需 Java。沿用该实例的 Java 选择、架构检查与自动下载设置，
隐藏的 Java 不参与自动选择。Java 版本不匹配或禁止自动下载且没有所需环境时会明确报错。

## 文件处理

1. 从官方 Forge Maven 下载安装器及 SHA-1 校验值，校验成功后才执行。
2. 读取安装信息并核对 Minecraft 版本、实例标识、格式及依赖校验值。
3. 创建独立暂存目录，复制已核对的客户端和可复用依赖，写入空的临时启动配置。
4. 强制无界面运行官方安装器，保留输出到任务日志，提供取消及执行时限。
5. 仅导入声明的客户端依赖和生成产物，验证官方哈希；不导入临时账户／启动配置。
6. 完成依赖准备后排他发布新实例配置，已有同名实例不会覆盖；清理本次暂存目录。

新安装发现原版客户端内容被修改或缺失时拒绝继续，提示用户单独校验原版，不在安装过程中自动替换它。
安装期间再次核对客户端，发现变化则不发布新实例。已成功校验的依赖缓存可以复用；取消不会删除已有实例。

实例中记录 Forge 版本、安装器校验值和生成产物校验清单。启动准备发现缺失或损坏时提示校验，
日志诊断也提供对应入口。显式校验会固定原版本，在暂存副本中重新执行官方安装器，修复依赖而保留实例配置字节。
必要时显式校验也会恢复缺失的基础游戏文件；这与“新安装不替换原版客户端”分开处理。

## 验证

独立测试目录中已实际执行官方 Forge 47.4.10／Minecraft 1.20.1 安装和损坏产物修复，
检查原版客户端、已有启动配置及 Forge 实例配置不变，70 个安装产物通过校验，暂存目录清理完成。
已验证 Forge 模组环境识别和真实 Modrinth 搜索。

提交 `8d14a46` 的最终本地检查：68 项核心检查、36 项界面检查和 8 项控制台组件检查通过。
实际安装使用新的独立目录 `forge-online-proof-NQOl8d`，完整走过首次安装、损坏产物修复、
观测安装器存活后取消、确认子进程退出，以及 JEI 安装；原版及已有配置保留，暂存目录已清理。
安装器 SHA-1 为 `66bfea9963bfa60d88bab6b2750e74a958392715`；
JEI 文件 `jei-1.20.1-forge-15.56.0.205.jar` 的 SHA-1 为 `7f64b7f8fde7f001ef054dabe3618a8780ba2650`。
此记录是 Windows 隔离测试证据，尚未验证三端真实游戏运行。

同一提交的 [远端构建](https://github.com/tachy001/rust-hmcl/actions/runs/39244940477)
已通过 Windows、Linux、macOS ARM／Intel 四个新版 Tauri 构建。
Windows 的 21 项原生检查在远端及本机隐藏窗口均通过，且无外置 WebView2Loader DLL 导入。
下载产物的 GitHub SHA-256 已核对；远端与本机证据均对应同一个 EXE，SHA-256 为
`a9541d8ac79e6e64cf012ddab953ddc8fece26e75c7ce984cc97a75898d1deb5`。

```sh
node scripts/test-forge-online.mjs --game-dir <安装验证目录中的minecraft> --java <Java可执行文件>
node scripts/test-forge-online.mjs --fixture .tools/forge-online-proof-xxxxxx --repair-corruption --check-cancel --search --install-mod
```

该脚本会执行官方 Java 安装器、版本检测及 `--dry-run` 虚拟机验证，不执行游戏主函数。
取消检查要求先观测到活的安装器进程，再请求取消并确认进程退出；未进入运行阶段不会计为通过。
可选的模组检查将 JEI 下载到测试实例并核对 Modrinth 返回的版本、加载器与 SHA-1，不执行模组代码。
结果写入 `dist/forge-online-proof.json`。普通界面与原生脚本只做无效输入、资源和 IPC 检查，不执行 Java。
尚不能据此声称真实游戏渲染、认证或三端游戏实机运行已经完成。

实现流程参考 [Forge 官方安装器入口](https://github.com/MinecraftForge/Installer/blob/2.0/src/main/java/net/minecraftforge/installer/SimpleInstaller.java)
及 [客户端安装行为](https://github.com/MinecraftForge/Installer/blob/2.0/src/main/java/net/minecraftforge/installer/actions/ClientInstall.java)。
Java 验证模式见 [Java 17 `--dry-run` 说明](https://docs.oracle.com/en/java/javase/17/docs/specs/man/java.html)。
不复制其 Java 源码，不将 Forge 安装器或游戏 JAR 打入发行包。
