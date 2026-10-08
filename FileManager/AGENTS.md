# AGENTS.md — FileManager 项目约定

给在本仓库工作的 AI / 协作者的硬性约束。**开始写代码前先读完本文。**

## 技术选型（已定，不要重新论证）

**Rust 2024 edition + Slint 1.18**（`slint` + `slint-build`）。

- 唯一**官方内置** Fluent 与 Material M3 皮肤、且改配置就能切换的纯 Rust GUI 方案
- 无 WebView / 无 JS 引擎 / 无 GC：`.slint` 编译期变成原生 Rust 代码
- 默认同时启用 Wayland 与 X11 后端

**不要推荐或迁移到** Tauri、Qt/`cxx-qt`/`qmetaobject-rs`、egui/eframe、iced、Dioxus、makepad、GPUI。
若 Slint 确实无法满足某个具体需求，**先明确指出来并说明代价**，由用户决定，不要默默换掉。

唯一的备选路径：**`slint` 的 `backend-qt` 后端**，且仅在默认 winit 后端的中文输入法
在 Wayland 下实测不可用时才启用（本机已装 `qt6-base`/`qt6-declarative`/`qt6-wayland`）。
**换后端前必须先告诉用户。**

## 运行环境

| 项目 | 值 |
|---|---|
| 发行版 | Arch Linux |
| 合成器 | niri（纯 Wayland，滚动平铺，**无 SSD 窗口装饰**，无内置 XWayland） |
| 显卡 | Intel Alder Lake-P Iris Xe（`mesa` + `vulkan-intel`） |
| 工具链 | `rustc` / `cargo` 1.98.1 |

**沙箱构建**：DSH 会话里 `~/.local/share/cargo` 与 `~/.local/share/rustup` 只读，
但 `RUSTUP_HOME` **不能**指向空目录（rustup shim 会报找不到默认 toolchain）。
正确做法是把 `cargo` / `rustc` / `rustdoc` 全部钉到真实 toolchain 二进制，
只把 `CARGO_HOME` 重定向到 `FileManager/rust/.cargo-home/`（已 gitignore）。
`FileManager/scripts/dev.sh` 已经封装好这件事。

## 硬性规则

### 性能（第一优先级）

1. **动画只能用 `animate`，禁止 Timer 手动插值。** Slint 的 `animate` 编译成渲染线程上的
   时间线，按需产帧、动画结束立即停止重绘；Timer 逐帧改属性会持续唤醒事件循环，永不空闲。
2. **不要用 Timer 轮询数据**，用事件驱动或 `slint::spawn_local`。
3. **只动 `opacity` 与颜色，不要动 `width` / `height`**：后者每帧触发重新布局。
4. **界面静止时 CPU 必须稳定在 0.0%**。若测得不是 0，说明某处引入了周期性唤醒，必须定位消除。
5. **保持 vsync 开启**，不要为追帧率关掉它。
6. **大列表用 `ListView` + 数据模型**，不要一次性实例化所有 delegate。
7. **任何性能结论都必须在 release 构建下得出。** debug 数据一律无效。
8. 冷启动测量**必须 `--repeat 5` 取平均**，单次结果没有参考价值。

### 视觉

9. **颜色一律走 `Palette.*` / `Theme.*`，禁止硬编码色值**——硬编码会让明暗切换和风格切换失效。
10. 风格是**编译期**由 `SLINT_STYLE`（或 `build.rs` 的 `with_style`）决定的，
    切换风格后需要重编译；`build.rs` 已声明 `rerun-if-env-changed`。
11. 调界面用 `slint-viewer` 预览，不必反复 `cargo run`。

### Wayland

12. **不要用 Skia 渲染后端**（已知上下文问题 slint-ui/slint#7845，且它一旦编译进去会被优先选中）。
13. 性能测试时**显式指定渲染器**（`SLINT_BACKEND=winit-femtovg` 或 `winit-femtovg-wgpu`），
    避免 GPU 初始化失败时静默回落到软件渲染。
14. **不要假设中文输入法能用**——必须在 Wayland 下用 fcitx5 实测，界面上方的搜索框就是入口。
15. 窗口是无边框的（niri 不做 SSD）。要标题栏就必须自绘。

### 工程

16. **始终用中文回答，代码注释与文档也用中文。**
17. **每次修改完成后执行 `git commit` + `git push`**，不要攒着。
18. **任务完成后清理临时产物**：构建日志、调试输出、`__pycache__`、临时测试文件等，保持工作树干净。
19. **需要第三方依赖时直接安装**，不要为"少一个依赖"而手写替代实现。
20. **临时脚本放在项目目录内**（如 `scripts/`），不要放 `/tmp`（沙箱内 bash 看不到 write 工具写的 /tmp）。
21. **辅助脚本用 Python 编写**（除非有明确理由用别的）。
22. GitHub 推送若失败：`github.com` 被 Watt Toolkit 劫持，走 SSH 的 443 通道
    （`ssh://git@ssh.github.com:443/...`）。

## 代码结构约定

- 界面全部放 `rust/ui/app.slint`；Rust 侧只做接线，不拼界面。
- 纯逻辑（目录读取、排序、格式化）放 `rust/src/filemodel.rs`，**不依赖 UI 类型**，便于单测。
- 回调闭包**只捕获 `slint::Weak`**，绝不捕获强句柄——否则形成 `组件 → 回调 → 组件` 的引用环，
  窗口永不释放。应用状态统一放 `main.rs` 的 `STATE` thread_local，通过 `with_state` 访问。
- 过滤 / 排序只操作展示模型，**不重新读盘**；`State::all` 保存当前目录的全量条目。
