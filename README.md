# rust-hmcl

用 Rust 重写的 Hello Minecraft! Launcher（HMCL）。启动器本体为原生二进制，**运行无需任何 Java 依赖**，仅在启动游戏时按需检测/下载 Java。

## 架构

```
crates/
├─ hmcl-core/   # 核心逻辑（零 UI 依赖）：auth / game / launch / download / modpack / java / task / event / util
├─ hmcl-ui/     # egui 界面层：主题 / i18n / 控件 / 页面 / 3D 皮肤 / 图像
└─ hmcl/        # 启动器入口二进制
assets/         # 图片、语言包、主题包（与 HMCL 格式兼容）
```

对应原 HMCL 的结构：`HMCLCore` → `hmcl-core`，`HMCL`（JavaFX UI）→ `hmcl-ui`，`HMCLBoot`（引导下载 Java）→ 删除（原生二进制无需引导）。

## 进度

- [x] **U1 基础设施**：i18n（`.properties` 格式兼容，zh_CN/en 先行）、CJK 字体、Monet 主题（亮/暗 + 6 种强调色）、纹理缓存、APNG 解码
- [x] **U2 窗口框架**：自绘标题栏、侧边导航、页面栈、101 个 SVG 图标（完整移植 `SVG.java` + path 解析器）
- [x] **核心工具**：VersionNumber / VersionRange（Maven 版本算法逐行移植，含原版 Java 测试套件）
- [ ] U2 控件库：Dialog / Tab / List / Tooltip / Spinner / Toast
- [ ] U3 账号页 + 登录（离线 / 微软 / 皮肤站）
- [ ] U3 实例网格 + 版本列表 + 安装向导
- [ ] U4 实例详情（mod / 存档 / 资源包 / 设置）
- [ ] U5 启动器设置 / 个性化 / Java 管理
- [ ] U6 3D 皮肤预览、动画打磨、多语言
- [ ] core：auth / download / game / launch / java 模块
- [ ] 三平台打包

## 构建

```bash
cargo build            # 调试版
cargo build --release  # 发布版
cargo test             # 测试
```

## 许可

GPL-3.0（与 HMCL 一致）。图标与部分算法移植自 HMCL（Material Symbols / Apache 2.0）。
