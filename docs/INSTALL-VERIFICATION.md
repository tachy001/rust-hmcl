# 安装、实例名称与加载器

自动安装页可以填写加载器实例名称；留空时使用 Fabric／Quilt 的默认名称。
新实例继承当前游戏的基础客户端；不会覆盖已有同名实例配置。
已有实例应使用校验功能修复，或指定另一个名称安装新实例。

复制或重命名原版实例时记录其原 Minecraft 版本。目录名称与客户端 JAR 名仍随实例变化，
游戏列表、模组环境和加载器 API 使用保留的 Minecraft 版本；加载器继承实际存在的本地客户端目录。
继续复制／重命名不会把别名误当作游戏版本。继承配置不能覆盖解析器计算出的身份字段。
此记录从本次复制／改名操作开始写入；历史版本已丢失原始版本信息的配置不会根据名称猜测补写。

加载器配置在依赖准备完成后排他提交。安装前拒绝已有完整配置，提交阶段再次使用
不覆盖目标的原生文件操作，防止并发安装覆盖先完成的配置。失败会清理本次独有的临时配置文件。

## 可选联网验证

常规测试不下载完整游戏。需主动运行：

```sh
cargo build -p rust-hmcl --bin hmcl-service --no-default-features --locked
node scripts/test-install-online.mjs
# 继续核验同一个、由脚本创建的独立目录：
node scripts/test-install-online.mjs --fixture .tools/install-online-proof-xxxxxx
# 仅破坏该测试目录中的客户端副本，再验证真实修复：
node scripts/test-install-online.mjs --fixture .tools/install-online-proof-xxxxxx --repair-corruption
```

脚本只在项目 `.tools/install-online-proof-*` 中工作。完整下载约需数百 MB 磁盘和网络，
不会启动 Java、Minecraft 或可见窗口。调用与桌面共用的 Service；`launch_plan` 是
JSON-lines 开发工具的只读命令，调用与真实启动相同的参数生成函数，不创建游戏进程。

本轮在 Windows x64 实际完成 Mojang 官方 Minecraft 1.20.1 安装、Fabric 和 Quilt 安装。
校验了原始版本元数据、客户端、64 个本机依赖库，以及 3,575 个不重复资源对象的官方 SHA-1；
资源对象共 651,432,558 字节。原版、改名原版、Fabric 和 Quilt 的实际启动参数中，
分别有 65、65、73、79 个存在的 classpath 文件，且无未替换占位符。

同一目录中完成真实复制、重命名和自定义加载器安装，模组环境仍识别为 Minecraft 1.20.1。
重复安装默认名称与自定义名称均拒绝覆盖，保留原配置字节。
校验并修复了故意损坏的测试客户端，保留有效资源对象；健康客户端在普通校验与加载器安装中复用。
记录保存在 `dist/install-online-proof.json`，包括实际服务程序 SHA-256、资源和依赖校验明细。

该验证证明文件准备与参数生成，不证明游戏渲染、输入、认证、真实模组运行或三端实机启动。
普通 UI 检查仍使用手工样本；二者不能混为同一种证据。

程序提交 `b6898c2` 的 [远端构建](https://github.com/tachy001/rust-hmcl/actions/runs/39203924906)
已核实 Windows、Linux、macOS ARM／Intel 四个新版 Tauri 构建通过。
Windows 单 EXE 导入检查及 20 项原生检查通过；产物的 GitHub 校验值与远端记录已核对，
同一程序又在本机隐藏窗口完成 20 项原生检查。源码核心检查 62 项通过、4 项联网检查跳过，
常规界面检查 35 项通过。实际官方下载是另外主动执行的流程，不包含在被跳过的联网单元检查中。
