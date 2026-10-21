# NeoForge 后端安装接口

NeoForge 已接入后端版本目录、独立安装器对象、实例识别、模组环境与修复流程。
本轮不新增页面控件；前端可通过现有调用封装接入，默认界面尚无 NeoForge 安装入口。

## 对象职责

- `LoaderCatalog`：读取官方版本，按真实 Minecraft 版本精确筛选，推荐最新稳定版。
- `InstallerFamily`：封装 Forge／NeoForge 的版本约束、官方仓库地址和安装记录名称。
- `OfficialInstaller`：组合 Worker 的网络、Java、进度及取消能力，负责隔离安装与修复。
- `Service`：调度任务、记录所用 Java、选择新实例并默认开启实例独立目录。

旧 `Worker::install_forge` 保留为兼容入口，委托同一个安装器对象；新代码不往 Worker 添加 NeoForge 安装方法。
两种加载器使用各自的缓存、暂存目录和元数据记录。

当前版本映射支持 Minecraft 1.20.2 及以上的 1.x 正式版本，安装包必须使用
`spec=1` 和 BootstrapLauncher 格式，并满足基础游戏所需的 Java 版本。
旧 1.20.1 NeoForge 分支、Minecraft 26.x 与其他新安装格式尚不支持。
整合包 `.mrpack` 的 NeoForge／Forge 依赖仍未接入。

## 接入

前端：`api.loaderVersions(id, 'neoforge')` 和 `api.start({ kind: 'neoforge', id, name, version })`。
name 和 version 可省略，分别使用官方实例名和目录推荐版本。

JSON-lines 开发入口调用同一个 Service：

```json
{"method":"loader_versions","id":"My Vanilla 1.20.4","loader":"neoforge"}
{"method":"start_job","job":{"kind":"neoforge","id":"My Vanilla 1.20.4","name":"My NeoForge","version":"20.4.251"}}
{"method":"start_job","job":{"kind":"repair","id":"My NeoForge"}}
```

版本必须与基础游戏对应，例如 20.4.x 只能用于 1.20.4。
非法版本和不匹配的组合在开始任务或执行 Java 前报错；已有同名实例拒绝覆盖。
新实例继承实际本地客户端目录，保留原 Minecraft 版本用于模组筛选。

## 文件与进程

从固定官方仓库下载安装器及 SHA-1，校验后才执行。复用实例 Java 选择、架构、隐藏与自动下载策略。
启动器本身不依赖 Java。新安装发现原客户端被修改或缺失时拒绝继续，不自动替换原客户端。

安装器面对独立暂存副本和空的临时启动配置，以无界面参数运行。
只导入声明的客户端依赖和生成产物，不导入临时账户或启动配置。
依赖按官方 SHA-1 校验；生成产物有官方预期哈希时核对，并记录最终产物清单。
没有官方哈希的生成文件由已验证的官方安装器生成，不能把其本地记录的哈希称为官方哈希。

支持安装进程取消、执行时限和日志大小限制，结束后清理本次暂存目录。
`rustHmclNeoForge` 记录版本、安装器哈希与产物清单。
启动准备检测产物损坏时提供修复入口；修复固定原版本重建，保留实例配置字节。
同一实例同时记录 Forge 和 NeoForge 时拒绝修复，避免混用安装器。

## 验证

使用专用目录中的 Minecraft 1.20.4、NeoForge 20.4.251 和 Temurin 17 JRE。
安装器 SHA-1 为 `8b80db7063076a31be74dc3df3506185cbb370b9`。
实际完成 90 个产物校验、生成客户端损坏修复、观察进程存活后取消并确认退出，
以及 Modrinth 兼容搜索和 JEI 下载；保留原客户端、既有实例和启动配置，清理暂存目录。
结果保存于 `dist/neoforge-online-proof.json`。

```sh
# 新建专用 1.20.4 游戏目录；可复制已有专用目录的资源与依赖缓存。
node scripts/test-install-online.mjs --minecraft 1.20.4 --seed-game-dir .tools/install-online-proof-xxxxxx/minecraft
# 使用上一命令报告的游戏目录，执行官方 Java 安装器检查：
node scripts/test-forge-online.mjs --neoforge --game-dir <专用minecraft目录> --java <Java路径> --repair-corruption --check-cancel --search --install-mod
```

执行 Java 安装器、版本检测及不执行主函数的 JVM `--dry-run`，没有启动游戏或模组代码，没有操作桌面。
产物与参数检查不能证明游戏渲染、认证、模组实际运行或三端游戏实机运行。

2026-10-21 最终本地回归：76 项核心检查、37 项界面检查通过；另主动查询四种官方目录，
NeoForge 1.20.4 当时有 245 项，推荐 20.4.253。实际安装验证固定为 20.4.251，不将目录可查询等同于每个版本都已安装验证。
同一服务程序在新的 `neoforge-online-proof-TuNqZ7` 和 `forge-online-proof-ksuhUP` 目录分别通过完整回归，
服务程序 SHA-256 为 `ee949e983e8c3769bc57fc21b85a19e69a06affef11830f3d82dc93bd9a50859`。

参考：[NeoForge 用户说明](https://docs.neoforged.net/user/docs/client/)、
[官方客户端安装实现](https://github.com/neoforged/LegacyInstaller/blob/main/src/main/java/net/minecraftforge/installer/actions/ClientInstall.java)、
[官方命令入口](https://github.com/neoforged/LegacyInstaller/blob/main/src/main/java/net/minecraftforge/installer/SimpleInstaller.java)。
没有复制其 Java 源码，安装器和游戏 JAR 不进入发行包。
