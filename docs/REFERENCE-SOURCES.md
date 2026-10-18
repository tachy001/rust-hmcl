# 参考源码索引

当前目标仍是 Rust 重构并完整还原 HMCL：Windows、Linux、macOS 可用，启动器本身无需 Java，游戏使用独立 Java 环境。新增 PCL 作为补充参考，不自动改变 HMCL 的界面与功能验收目标。

## 本机来源

- 主参考：项目相邻目录 `../HMCL-main/`；具体还原进度见 `HMCL-PARITY.md`。
- 补充参考：用户提供的 `../PCL-main/`，即 `D:\FTP\codex\rust-hmcl\PCL-main`。
- PCL 来源说明：其 README 指向 `https://github.com/Meloong-Git/PCL/`，作者为龙腾猫跃及相关贡献者。本轮只阅读、建立索引，没有移入 PCL 源码、图标或其他素材。

## PCL 对照入口

以下路径相对于 `PCL-main/Plain Craft Launcher 2/`。

| 范围 | 入口 | 关注点 |
| --- | --- | --- |
| 窗口外壳 | `FormMain.xaml`、`FormMain.xaml.vb` | 窗口结构、导航和状态切换 |
| 按钮反馈 | `Controls/MyButton.xaml`、`MyButton.xaml.vb` | 悬停、按压、释放与移出时恢复状态 |
| 卡片与列表 | `Controls/MyCard.vb`、`MyListItem.xaml.vb` | 折叠、布局变化及选中反馈 |
| 输入控件 | `Controls/MyTextBox.vb`、`MyCheckBox.xaml.vb` | 输入校验、焦点与状态反馈 |
| 动画调度 | `Modules/Base/ModAnimation.vb` | 动画分组、替换、取消与初始化门控 |
| 任务调度 | `Modules/Base/ModLoader.vb` | 后台任务状态与前台反馈 |
| 启动与 Java | `Modules/Minecraft/ModLaunch.vb`、`ModJava.vb` | Java 发现、选择、启动流程与错误处理 |
| 下载与实例 | `Modules/Minecraft/ModDownload.vb`、`Pages/PageDownload/PageDownloadInstall.xaml.vb`、`Pages/PageInstance/PageInstanceOverall.xaml.vb` | 安装流程、进度、失败反馈和实例入口 |

对照时分别核验交互目的和边界情况；原版 HMCL 的尺寸、曲线与行为仍以 HMCL 自身源码为依据。不能直接把 PCL 的按压或回弹参数当作 HMCL 的原版参数。

## 内容使用记录

PCL 的 `LICENCE` 标注《PCL 分发有限许可》，并链接完整许可，另附《PCL 存储库合理使用指南》。它不是本项目 HMCL 参考源码的同一套许可。

若后续实际引用 PCL 内容，需先核对适用条款并记录具体文件、引用范围、作者及署名位置；不能把本次只读审阅当作已取得无限制复用许可。当前项目没有因增加参考目录而变更名称或许可，也没有将该目录纳入 Git 仓库和发布包。
