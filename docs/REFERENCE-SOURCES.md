# 参考源码索引

用户已将目标调整为原创启动器，当前产品为“星屿 Aster”。面向 Windows、Linux、macOS，启动器本身无需 Java，游戏使用独立 Java 环境。HMCL 与新增 PCL 作为研究参考，设计方向见 `ASTER-DESIGN.md`。

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

对照时核验交互目的和边界情况，独立实现适合 Aster 的控件和布局。不能把 PCL 的参数描述成 HMCL 原版参数，也不将 Aster 的原创布局描述成参考启动器的官方界面。

## 内容使用记录

PCL 的 `LICENCE` 标注《PCL 分发有限许可》，并链接完整许可，另附《PCL 存储库合理使用指南》。它不是本项目 HMCL 参考源码的同一套许可。

若后续实际引用 PCL 内容，需先核对适用条款并记录具体文件、范围、作者和署名位置，不能把只读审阅当作无限制复用许可。PCL 目录不纳入 Git 和发布包。Aster 的命名来自用户要求原创产品，与引入 PCL 参考无关。
