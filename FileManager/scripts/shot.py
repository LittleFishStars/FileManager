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
