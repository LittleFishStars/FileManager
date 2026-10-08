#!/usr/bin/env python3
"""在 niri/Wayland 下测量 GUI 应用的启动耗时与稳态内存占用。

用法示例:
    python3 scripts/measure.py --app-id filemanager -- ./rust/target/release/filemanager

启动时间  从 spawn 到 niri 报告窗口出现之间的耗时
稳态内存  读取 /proc/<pid>/smaps_rollup 的 Pss
          （Pss 把共享库与字体缓存按比例分摊，比 RSS 更接近真实增量）

换渲染器做 A/B 对比（按项目约定必须显式指定，避免静默回落到软件渲染）:
    SLINT_BACKEND=winit-femtovg      python3 scripts/measure.py --repeat 5 -- ./rust/target/release/filemanager
    SLINT_BACKEND=winit-femtovg-wgpu python3 scripts/measure.py --repeat 5 -- ./rust/target/release/filemanager
"""
from __future__ import annotations

import argparse
import json
import pathlib
import shlex
import subprocess
import sys
import time


def niri_windows() -> list[dict] | None:
    """取 niri 当前窗口列表；niri IPC 不可用时返回 None。"""
    try:
        proc = subprocess.run(
            ["niri", "msg", "--json", "windows"],
            capture_output=True, text=True, timeout=2, check=False,
        )
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return None
    if proc.returncode != 0:
        return None
    try:
        return json.loads(proc.stdout)
    except json.JSONDecodeError:
        return None


def window_visible(pid: int, app_id: str | None) -> bool:
    """窗口是否已出现：优先按 pid 匹配，回退到 app_id。"""
    windows = niri_windows()
    if windows is None:
        return False
    for win in windows:
        if win.get("pid") == pid:
            return True
        if app_id and (win.get("app_id") or "") == app_id:
            return True
    return False


def pss_kib(pid: int) -> int | None:
    """读进程的 Pss（KiB）。"""
    try:
        text = pathlib.Path(f"/proc/{pid}/smaps_rollup").read_text()
    except OSError:
        return None
    for line in text.splitlines():
        if line.startswith("Pss:"):
            return int(line.split()[1])
    return None


def main() -> int:
    argv = sys.argv[1:]
    if "--" not in argv:
        print("用法: measure.py [选项] -- <命令> [参数...]", file=sys.stderr)
        return 2
    split = argv.index("--")
    own_args, cmd = argv[:split], argv[split + 1:]

    ap = argparse.ArgumentParser(description="测量 GUI 启动耗时与稳态内存")
    ap.add_argument("--app-id", help="niri 报告的 app_id（可省略，则只按 pid 匹配）")
    ap.add_argument("--timeout", type=float, default=20.0, help="等待窗口的上限秒数")
    ap.add_argument("--settle", type=float, default=5.0, help="判定内存稳定前的等待秒数")
    ap.add_argument("--repeat", type=int, default=1, help="重复次数（冷启动噪声大，建议 5）")
    args = ap.parse_args(own_args)

    if not cmd:
        print("错误: `--` 之后需要给出要启动的命令", file=sys.stderr)
        return 2

    starts: list[float] = []
    for i in range(1, args.repeat + 1):
        print(f"[{i}/{args.repeat}] 启动: {shlex.join(cmd)}")
        t0 = time.monotonic()
        proc = subprocess.Popen(cmd)

        startup = None
        while time.monotonic() - t0 < args.timeout:
            if proc.poll() is not None:
                print(f"  进程提前退出，返回码 {proc.returncode}", file=sys.stderr)
                return 1
            if window_visible(proc.pid, args.app_id):
                startup = time.monotonic() - t0
                break
            time.sleep(0.01)

        if startup is None:
            print(f"  {args.timeout}s 内未检测到窗口（niri IPC 可用吗？）", file=sys.stderr)
            proc.terminate()
            return 1
        starts.append(startup)
        print(f"  首窗口可见: {startup * 1000:.0f} ms")

        time.sleep(args.settle)
        samples = []
        for _ in range(10):
            value = pss_kib(proc.pid)
            if value is not None:
                samples.append(value)
            time.sleep(0.1)
        if samples:
            print(f"  稳态 Pss: {samples[-1] / 1024:.1f} MiB"
                  f"（本次峰值 {max(samples) / 1024:.1f} MiB）")
        else:
            print("  读不到 smaps_rollup", file=sys.stderr)

        proc.terminate()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
        time.sleep(0.5)

    if starts:
        best, worst = min(starts), max(starts)
        avg = sum(starts) / len(starts)
        print(f"\n启动耗时: 平均 {avg * 1000:.0f} ms / 最快 {best * 1000:.0f} ms"
              f" / 最慢 {worst * 1000:.0f} ms（{len(starts)} 次）")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
