# 性能基线

所有数据在 **release** 构建下测得。debug 数据无效，不要拿来对比。

- 硬件：Intel Alder Lake-P Iris Xe（集显）
- 合成器：niri（纯 Wayland）
- 测量脚本：`scripts/measure.py`（启动 = spawn 到 niri 报告窗口出现；内存 = `smaps_rollup` 的 `Pss`）
- 测量日期：2026-10-08
- 二进制：`rust/target/release/filemanager`，24 MB（`strip = true` + `lto = "fat"`）

## 冷启动与稳态内存

`python3 scripts/measure.py --app-id filemanager --repeat 5 -- ./rust/target/release/filemanager`

| 指标 | 值 |
|---|---|
| 启动耗时（5 次） | 平均 **151 ms** / 最快 117 ms / 最慢 205 ms |
| 稳态 Pss | **36.2 MiB**（5 次采样区间 36.1 – 36.5 MiB） |

启动方差较大（117–205 ms）来自 niri 侧的窗口映射与工作区切换，不是应用本身不稳定。

## 渲染器对比

显式指定 `SLINT_BACKEND` 可以避免 GPU 初始化失败时静默回落到软件渲染。

| `SLINT_BACKEND` | 启动耗时（3 次均值） | 稳态 Pss | 实际图形路径 |
|---|---|---|---|
| **未设置（默认）** | **151 ms** | **36.2 MiB** | wgpu → Vulkan（Iris Xe） |
| `winit-femtovg-wgpu` | 125 ms | 36.4 MiB | wgpu → Vulkan |
| `winit-femtovg` | **622 ms** | **161.8 MiB** | OpenGL / EGL → Mesa gallium |

结论：

1. **默认就是最快的路径**，不需要额外配置。本配置（显式启用 `renderer-femtovg-wgpu`）下，
   不设 `SLINT_BACKEND` 时 Slint 选中的是 wgpu/Vulkan 渲染器。
2. `winit-femtovg` 走的是 OpenGL/EGL + Mesa gallium 那条路，**多花 470 ms 启动时间、
   多占 125 MiB 内存**（`libEGL`、`libgallium`、`libgbm` 被拉进来）。本项目不使用它。
3. 判定"实际走了哪条路径"的办法是看 `/proc/<pid>/maps` 是否加载了 `libvulkan`
   还是 `libEGL` / `libgallium`。只看启动快慢会被 niri 的噪声误导。

## 静止时的 CPU

要求：界面静止时 CPU 稳定在 0.0%。

测量方法：让窗口停留 50 秒不动，每 10 秒读一次 `ps -o time -T`（各线程**累计** CPU 时间）。

```
启动 6 秒后的累计 CPU 时间: 0.00 s
  第 1 个 10 秒窗口: 累计增长 0.00 s → 平均单核 0.00%
  第 2 个 10 秒窗口: 累计增长 0.00 s → 平均单核 0.00%
  第 3 个 10 秒窗口: 累计增长 0.00 s → 平均单核 0.00%
  第 4 个 10 秒窗口: 累计增长 0.00 s → 平均单核 0.00%
  第 5 个 10 秒窗口: 累计增长 0.00 s → 平均单核 0.00%
```

**静止 50 秒 CPU 增量为 0**（时钟粒度 10 ms，即全程低于可测下限）。线程数稳定为 5：
主线程 + winit 事件循环 + `async-io` + `zbus::Connection`（accessibility 的 D-Bus 连接）。

> **测量陷阱**：不要用 `ps -o %cpu` 判断静止占用——它是**自进程启动以来的生命周期平均值**
> （总 CPU 时间 ÷ 存活时间），启动阶段的开销会永久混在里面，读出来像 1–2%，
> 而进程其实完全空闲。要用累计 `TIME` 的差值，或 `/proc/<pid>/stat` 的 utime+stime 差值。

## 尚未验证 / 下一步

- **中文输入法（最高风险）**：需要在搜索框里用 fcitx5 + rime 实际打一句中文。
  失败则按 `AGENTS.md` 的约定改用 `backend-qt`（改动前必须先告知用户）。
- **分数缩放**：需要在 niri 里把缩放设为非 1.0，检查文字锐利度与布局。
- **字体扫描对启动的影响**：本机装了 noto 全家桶 + CJK。当前 151 ms 已经达标，
  所以暂不动；若以后要压低，第一站就是这里（考虑 `SLINT_FONT_PATH` 之类的收敛手段）。
- **大目录**：还没测过上万条目的目录，`ListView` 只实例化可见行，理论上没问题，
  但 `State::all` 会一次性持有全部 `Metadata`，值得实测。
- **常驻功耗**：可以用 `intel_gpu_top` 确认动画期间 GPU 真有活动、静止时确实归零。
