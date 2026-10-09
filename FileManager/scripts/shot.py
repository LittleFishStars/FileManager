#!/usr/bin/env python3
"""在 niri/Wayland 下给应用窗口截图，用于改完界面后自查显示效果。

实现方式：优先用 niri 自己的 `screenshot-window` 动作（它精确知道窗口边界），
配合 `--write-to-disk false` 只送剪贴板、不往 ~/Pictures 落文件，再用 wl-paste 取回 PNG。
比起「抓全屏再猜窗口位置」，这条路不依赖任何坐标字段，也不会污染用户的图片目录。

依赖：niri（提供 screenshot-window 动作）、wl-clipboard（wl-paste）。

用法：
    python3 FileManager/scripts/shot.py
    python3 FileManager/scripts/shot.py --out FileManager/shot.png --wait 6
    python3 FileManager/scripts/shot.py --backend winit-femtovg   # 对比不同渲染器

注意：会先杀掉已有的同名应用进程，避免截到上一个实例；结束后结束自己拉起的进程。
"""
from __future__ import annotations

import argparse
import os
import pathlib
import subprocess
import sys
import time

REPO = pathlib.Path(__file__).resolve().parent.parent          # FileManager/
DEFAULT_BIN = REPO / "rust" / "target" / "release" / "filemanager"
APP_ID = "filemanager"

# 会被截图结果影响的源文件。用来判断要截的二进制是不是比源码旧。
SOURCE_GLOBS = ("rust/src/*.rs", "rust/ui/*.slint", "rust/Cargo.toml", "rust/build.rs")


def stale_binary(bin_path: pathlib.Path) -> str | None:
    """二进制比源码旧时返回一句提示，否则 None。

    为什么要检查：这个脚本默认截 release，而开发时常用的 `dev.sh build` 只构建 debug。
    结果就是「改了代码、跑的是旧二进制」，量出来的数字**完全不随改动变化**——
    看起来像「我的修改没生效」，实际上是在量一个几小时前的产物。
    这种测量结果异常稳定本身就是最强信号，所以宁可在脚本里直接报警。
    """
    if not bin_path.exists():
        return f"二进制不存在：{bin_path}（先跑 scripts/dev.sh release 或 build）"
    bin_mtime = bin_path.stat().st_mtime
    newer: list[str] = []
    for pattern in SOURCE_GLOBS:
        for path in REPO.glob(pattern):
            if path.stat().st_mtime > bin_mtime:
                newer.append(str(path.relative_to(REPO)))
    if not newer:
        return None
    shown = ", ".join(sorted(newer)[:4])
    more = "" if len(newer) <= 4 else f" 等 {len(newer)} 个"
    return (f"注意：要截的二进制比这些源文件旧 -> {shown}{more}\n"
            f"      你现在看到的可能是改动前的画面。先重新构建，"
            f"或用 --bin 指向刚构建的 debug 产物。")


def find_window_id(pid: int) -> int | None:
    """按 pid 找 niri 窗口 id；找不到再按 app_id 兜底。"""
    import json
    proc = subprocess.run(["niri", "msg", "--json", "windows"],
                          capture_output=True, text=True, timeout=5, check=False)
    if proc.returncode != 0:
        return None
    try:
        windows = json.loads(proc.stdout)
    except json.JSONDecodeError:
        return None
    for win in windows:
        if win.get("pid") == pid:
            return win.get("id")
    for win in windows:
        if (win.get("app_id") or "") == APP_ID:
            return win.get("id")
    return None


def main() -> int:
    ap = argparse.ArgumentParser(description="给应用窗口截图")
    ap.add_argument("--bin", default=str(DEFAULT_BIN), help="要启动的可执行文件")
    ap.add_argument("--out", default=str(REPO / "shot.png"), help="输出 PNG 路径")
    ap.add_argument("--wait", type=float, default=5.0, help="启动后等待窗口渲染的秒数")
    ap.add_argument("--backend", help="覆盖 SLINT_BACKEND，用于渲染器对比")
    args = ap.parse_args()

    for tool in ("niri", "wl-paste"):
        if subprocess.run(["sh", "-c", f"command -v {tool}"],
                          capture_output=True).returncode != 0:
            print(f"缺少 {tool}", file=sys.stderr)
            return 2

    # 先提示二进制是否过期：这是最容易导致「看起来没生效」的坑
    stale = stale_binary(pathlib.Path(args.bin))
    if stale:
        print(f"[警告] {stale}", file=sys.stderr)

    # 清掉同名旧进程，避免截到上一个实例。
    # 用进程名精确匹配（pkill -x）而不是 pkill -f：后者会匹配到自己的命令行，
    # 把执行本脚本的 shell 一起杀掉。
    subprocess.run(["pkill", "-x", os.path.basename(args.bin)], capture_output=True)
    time.sleep(0.5)

    env = dict(os.environ)
    if args.backend:
        env["SLINT_BACKEND"] = args.backend

    proc = subprocess.Popen([args.bin], env=env,
                            stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    try:
        time.sleep(args.wait)
        if proc.poll() is not None:
            err = (proc.stderr.read() or b"").decode(errors="replace")
            print(f"应用提前退出（{proc.returncode}）:\n{err}", file=sys.stderr)
            return 1

        win_id = find_window_id(proc.pid)
        if win_id is None:
            print("niri 里找不到该应用窗口", file=sys.stderr)
            return 1

        # 滚动平铺下窗口可能不在可视区，先聚焦让它显示出来
        subprocess.run(["niri", "msg", "action", "focus-window", "--id", str(win_id)],
                       capture_output=True)
        time.sleep(1.2)

        shot = subprocess.run(
            ["niri", "msg", "action", "screenshot-window",
             "--id", str(win_id), "--write-to-disk", "false"],
            capture_output=True, text=True, timeout=15, check=False,
        )
        if shot.returncode != 0:
            print(f"niri 截图失败: {shot.stderr.strip()}", file=sys.stderr)
            return 1
        time.sleep(1.0)

        paste = subprocess.run(["wl-paste", "--type", "image/png"],
                               capture_output=True, timeout=15, check=False)
        if paste.returncode != 0 or not paste.stdout:
            print("从剪贴板取不到 image/png", file=sys.stderr)
            return 1

        out = pathlib.Path(args.out)
        out.write_bytes(paste.stdout)

        # 顺手报告尺寸，便于确认截的是不是整窗
        try:
            from PIL import Image
            with Image.open(out) as im:
                size = f"{im.width}x{im.height}"
        except Exception:
            size = "?"
        print(f"已保存: {out}  ({size})")
        return 0
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()


if __name__ == "__main__":
    raise SystemExit(main())
