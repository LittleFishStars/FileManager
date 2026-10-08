# FileManager

Fluent Design / Material Design 3 风格的 Linux 原生文件管理器，用 **Rust + Slint 1.18** 写成。

界面在编译期被编译成原生 Rust 代码：没有 WebView、没有 JS 引擎、没有 GC。
目标是冷启动快、常驻内存低、界面静止时 CPU / GPU 占用为 0。

## 当前状态

第一阶段（跑通 + 建立性能基线）已完成：

- 双栏文件管理器：侧边栏 + 文件列表（名称 / 类型 / 大小 / 修改时间）
- 导航：后退 / 前进 / 上级 / 侧边栏跳转，历史栈独立于展示模型
- 交互：单击选中、双击进入目录或交给 `xdg-open`
- 操作：新建文件夹、重命名（右侧输入框回车）、删除（移入回收站）
- 侧边栏分两区：**固定入口**（主文件夹 / 回收站 / 根目录 / 临时文件 / 外部设备）
  与**家目录文件夹**（按机器实际情况扫描，常见目录排前面）。
  第一区写死、第二区扫描，换一台机器第二区会自然跟着变
- **回收站可浏览**：条目名从 `info/*.trashinfo` 还原（含 URL 百分号解码，中文名正常显示），
  在回收站里按删除 = 永久删除
- 搜索：按文件名实时过滤当前目录，无需重新读盘
- 排序：按名称 / 大小 / 修改时间 / 类型，可切升降序
- 隐藏项开关（顶部 `◉ / ◎`）
- 状态栏实时显示条目数与当前编译期风格名
- 列头即排序控件（点字段切换、再点切升降序），列宽与列表严格对齐
- 文件/文件夹图标用 Material Icon Theme，界面图标用 Tabler Icons（见下节）
- 支持 `filemanager [目录]` 指定启动目录

**尚未实现**：面包屑地址栏可编辑、右键上下文菜单、多选、缩略图预览、
剪贴板（复制 / 剪切 / 粘贴）、标签页、侧边栏的「最近 / 收藏 / 网络」、
挂载点上的回收站（`<挂载点>/.Trash-<uid>`，目前只支持家目录那一个）。
这些都在路线图里，但不属于第一阶段。

## 构建

```bash
# 普通终端（在 MyLinux/ 目录下）
cargo build --release --manifest-path FileManager/rust/Cargo.toml

# DSH 沙箱会话（~/.local/share/cargo 只读）
FileManager/scripts/dev.sh release
```

> **沙箱下的工具链处理**：DSH 会话里 `~/.local/share/cargo` 与 `~/.local/share/rustup`
> 都是只读的，rustup 的 shim 一写锁文件就失败。但 `RUSTUP_HOME` 又**不能**指向空目录
> （rustup 会找不到默认 toolchain 而报 "could not choose a version of cargo"）。
> 因此 `FileManager/scripts/dev.sh` 直接调用真实 toolchain 里的 `cargo`，只把 `CARGO_HOME`
> 重定向到 `FileManager/rust/.cargo-home/`（纯下载缓存，已在 `.gitignore` 中）。
> 项目里没有 `rust-toolchain.toml`，所以真实 cargo 不会尝试下载 toolchain。

运行：

```bash
./FileManager/rust/target/release/filemanager
```

## 风格切换

风格由 `SLINT_STYLE` 在**编译期**决定（见 `rust/build.rs` 的 `with_style`）：

| 值 | 说明 |
|---|---|
| `fluent` / `fluent-light` / `fluent-dark` | 微软 Fluent Design，未指定时的默认值 |
| `material` / `material-light` / `material-dark` | Material Design 3 |
| `cupertino` / `cosmic` / `qt` / `native` | 其他内置风格 |

```bash
SLINT_STYLE=material cargo run --release --manifest-path FileManager/rust/Cargo.toml
touch FileManager/rust/build.rs && SLINT_STYLE=material-dark cargo run --release --manifest-path FileManager/rust/Cargo.toml
```

`FileManager/rust/build.rs` 声明了 `cargo:rerun-if-env-changed=SLINT_STYLE`，改环境变量会自动重编译。

界面里**没有任何硬编码颜色**，全部走 `Palette.*`（包括明暗判断用的 `Palette.color-scheme`），
所以明暗主题与风格切换不需要改一行界面代码。

单独预览界面（不必反复 `cargo run`）：

```bash
cargo install slint-viewer
SLINT_STYLE=material slint-viewer FileManager/rust/ui/app.slint
```

## 改完界面怎么看效果

不要靠反复启动肉眼比对，直接截图：

```bash
python3 FileManager/scripts/shot.py                          # release 版
python3 FileManager/scripts/shot.py --bin FileManager/rust/target/debug/filemanager
```

它用 niri 自己的 `screenshot-window` 动作（精确知道窗口边界），配 `--write-to-disk false`
只送到剪贴板、不往 `~/Pictures` 落文件，再用 `wl-paste` 取回 PNG 存到
`FileManager/shot.png`。比「抓全屏再猜窗口位置」可靠，也不会污染你的图片目录。

**改界面时必须实际看一眼**：本项目踩过的坑里，有一半是编译和逻辑都过、但画面明显不对的
（列头整块被涂成选中色、名称列被压成 0 宽、箭头小到看不清）。

## 性能

当前基线（release，niri/Wayland）：**冷启动 150 ms 量级，稳态 Pss 36–38 MiB，
静止 50 秒 CPU 增量为 0**。完整数据与「跨会话不可比」的说明见 [`docs/performance.md`](docs/performance.md)。
**所有性能结论必须来自 release 构建。**

```bash
# 冷启动 + 稳态内存（必须 --repeat 5 取平均，单次无参考价值）
python3 FileManager/scripts/measure.py --app-id filemanager --repeat 5 -- ./FileManager/rust/target/release/filemanager

# 渲染器 A/B。显式指定可以避免 GPU 初始化失败时静默回落到软件渲染
SLINT_BACKEND=winit-femtovg      python3 FileManager/scripts/measure.py --repeat 5 -- ./FileManager/rust/target/release/filemanager
SLINT_BACKEND=winit-femtovg-wgpu python3 FileManager/scripts/measure.py --repeat 5 -- ./FileManager/rust/target/release/filemanager
```

实测结论：**默认（不设 `SLINT_BACKEND`）就是最快的路径**，走 wgpu/Vulkan；
显式写 `winit-femtovg` 反而会掉到 OpenGL/EGL + Mesa gallium，多花 470 ms 启动、
多占 125 MiB 内存。渲染器 A/B 时建议同时用 `/proc/<pid>/maps` 确认实际走的是哪条路。

> 别用 `ps -o %cpu` 判断静止占用：它是自进程启动以来的**生命周期均值**，
> 会把启动开销永久摊在里面，读出来像 1–2%。要看累计 `TIME` 的差值。


界面里**没有 Timer**：所有过渡都是 `animate`（编译成渲染线程上的时间线，动画结束即停止重绘），
只动 `background` 与 `opacity`，不动 `width` / `height`，因此静止时不会触发重新布局。

### 调优配置（等基线量完再启用）

`FileManager/rust/Cargo.toml` 目前是完整默认 feature 链（含软件渲染兜底）。确认渲染稳定后可精简：

```toml
slint = { version = "1.18", default-features = false, features = [
    "std",
    "backend-winit-wayland",   # 只要 Wayland
    "renderer-femtovg-wgpu",
    "compat-1-2",              # .slint 语言兼容层，不能省
    "accessibility",
] }
```

注意：精简后不再包含 `renderer-software` 兜底，GPU 初始化失败会直接起不来。

## Wayland / niri

- 默认 feature 链同时启用 `backend-winit-wayland` 与 `backend-winit-x11`，niri 下走 Wayland 原生协议。
- niri 不提供窗口装饰，所以窗口是无边框的；自绘标题栏尚未做（平铺场景下通常也确实不需要）。
- 启动时调用 `slint::set_xdg_app_id("filemanager")`，niri 才能按 app-id 认领窗口。
- 分数缩放下会打印实际缩放因子到状态栏（`就绪 · 窗口缩放 1.50x` 之类）。

**中文输入法已实测通过**（2026-10-09）：在 niri + fcitx5(rime) 下用默认 winit 后端
打中文正常，因此**不需要**启用备选的 `backend-qt`。界面上方的搜索框就是验证入口，
以后换渲染器或升 Slint 版本时可以在这里复测。

## 项目结构

```
rust/
  Cargo.toml
  build.rs                 # 编译期烘焙风格
  ui/app.slint             # 界面（编译成原生 Rust）
  src/main.rs              # 导航 / 打开 / 重命名 / 删除 / 过滤的接线
  src/filemodel.rs         # 目录读取、排序、格式化（纯逻辑，无 UI 依赖）
  src/icons.rs             # 图标内嵌与「文件名 -> 图标」映射 + 自检测试
  icons/material/*.svg     # 文件/文件夹图标（Material Icon Theme）
  icons/tabler/*.svg       # 界面图标（Tabler Icons）
scripts/
  measure.py               # niri 下的冷启动与稳态 Pss 测量
  shot.py                  # 给窗口截图，改完界面后自查显示效果
  fetch_icons.py           # 从上游抓取图标（含上游改名校验）
  dev.sh                   # 构建封装（处理沙箱下只读 CARGO_HOME）
```

## 图标

两类图标，分别来自两个上游，**SVG 随源码入库**，编译期用 `include_bytes!`
打进二进制，运行时不依赖任何外部文件：

| 用途 | 来源 | 说明 |
|---|---|---|
| 文件 / 文件夹图标 | [Material Icon Theme](https://github.com/material-extensions/vscode-material-icon-theme/tree/main/icons) | 多色，按扩展名与目录名匹配，保持原色 |
| 界面图标（工具栏 / 侧边栏 / 地址栏） | [Tabler Icons](https://github.com/tabler/tabler-icons) | 单色 outline，用 Slint 的 `colorize` 换成主题前景色 |

补图标或上游更新：

```bash
python3 FileManager/scripts/fetch_icons.py          # 只下缺失的
python3 FileManager/scripts/fetch_icons.py --list   # 只打印清单
python3 FileManager/scripts/fetch_icons.py --force  # 全部重下
```

**脚本会先拿上游的文件清单校验一遍名字再下载。** 上游会改图标名（例如
`folder-music` 已被 `folder-audio` 取代），静默改名会让某类文件悄悄退回通用图标
却毫无提示，所以宁可先比一次清单、报错让人来处理。

换肤不需要第二套图标：Tabler 那批是单色的，`colorize` 直接跟随 `Palette.foreground`，
所以 Fluent ⇄ Material、浅色 ⇄ 深色都会自动适配。

`rust/src/icons.rs` 里有 5 条自检测试，其中两条覆盖最危险的静默失败：

- `every_mapped_icon_exists` —— 映射表引用的图标名必须真的内嵌了文件
- `every_icon_decodes` —— 每个内嵌图标必须真的能解码成 `Image`
  （名字对、字节在、但解码失败时界面只会悄悄退回通用图标，功能看着正常）

跑测试：`FileManager/scripts/dev.sh test`

## 数据来源与设计取舍

- **修改时间格式化**不引入 `chrono`：直接用 libc 的 `localtime_r` 拿时区偏移，
  再用 Howard Hinnant 的 `civil_from_days` 反推公历日期。省掉一整条依赖链。
- **删除**走 `trash-put` / `gio trash` 外部命令而不是 `trash` crate，理由同上；
  这两个命令本身就是 freedesktop 回收站规范的实现，行为与桌面环境一致。
  删除是**可逆**的（进回收站），因此没有做二次确认对话框。
  回收站**可以浏览**：条目名从 `info/*.trashinfo` 的 `Path=` 还原，该字段是 URL
  百分号编码的，必须解码否则中文名会显示成一串 `%E6%…`。
  在回收站里再按删除是**永久删除**（回收站本来就该有这个出口），
  这时不能拿条目的原始路径去删——它早在移入回收站时就不存在了，
  要按 trashinfo 的记录反查回收站内的存储副本。
- **图标**用几何图形加字首字母画出来，不引入图标字体或图片资源：
  字体在别的发行版上随时可能缺失，而 Slint 的 `Path` 元素 `commands`
  并不随 `width` / `height` 缩放（容易踩坐标系陷阱）。

## 许可

GPL-3.0-only。
