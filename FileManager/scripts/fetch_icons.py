#!/usr/bin/env python3
"""下载文件/文件夹图标（Material Icon Theme）与界面图标（Tabler Icons）。

两个上游：
  * 文件/文件夹图标：material-extensions/vscode-material-icon-theme
        https://github.com/material-extensions/vscode-material-icon-theme/tree/main/icons
  * 界面图标：tabler/tabler-icons
        https://github.com/tabler/tabler-icons

产出的 SVG 直接随源码入库（`rust/icons/`），编译期用 include_bytes! 打进二进制，
运行时不依赖任何外部文件，也不增加启动时的磁盘 IO。

为什么用 `gh api` 而不是直接 curl：
  raw.githubusercontent.com 在本机被 hosts 劫持，取不到；api.github.com 虽然通，
  但匿名配额只有 60 次/小时，下一套图标就会用光。gh 已登录，走独立配额（5000/小时）。

关于着色：
  Tabler 的 outline 图标用 `fill="currentColor"`，而 SVG 解析器不认识 currentColor，
  会退化成黑色——在深色主题下等于看不见。所以下载时统一把它换成白色，
  运行时再用 Slint 的 `colorize` 按主题色上色（colorize 把图片当 alpha 蒙版）。
  Material 的图标本身是彩色的，不做归一化，也不着色。

用法：
    python3 FileManager/scripts/fetch_icons.py            # 下载缺失的图标
    python3 FileManager/scripts/fetch_icons.py --force    # 全部重下
    python3 FileManager/scripts/fetch_icons.py --list     # 只打印清单，不下载
"""
from __future__ import annotations

import argparse
import base64
import json
import pathlib
import re
import subprocess
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent          # FileManager/
ICON_ROOT = REPO / "rust" / "icons"

MATERIAL_REPO = "material-extensions/vscode-material-icon-theme"
TABLER_REPO = "tabler/tabler-icons"

# ---------------------------------------------------------------------------
# 清单：图标名 -> 是否必需
#
# 命名与上游文件名一致，方便日后对着上游核对；Rust 侧的扩展名映射用的是这些名字。
# ---------------------------------------------------------------------------

# 文件与文件夹图标（Material Icon Theme，取 icons/<name>.svg）
MATERIAL_ICONS = [
    # 通用
    "document", "textlint", "settings", "tune", "lock", "key", "license", "readme",
    "console", "log", "diff", "template", "credits", "authors", "citation",
    # 图片 / 音视频 / 字体
    "image", "svg", "audio", "video", "subtitles", "font", "lottie",
    # 压缩 / 镜像 / 磁盘
    "zip", "disc", "exe", "dll", "hex", "lib", "database", "table", "xml", "epub",
    # 文档
    "word", "powerpoint", "pdf", "typst", "tex",
    # 数据 / 配置
    "json", "yaml", "toml", "editorconfig", "buildkite", "hosts",
    # 语言
    "rust", "python", "python-misc", "javascript", "typescript", "react", "react_ts",
    "c", "h", "cpp", "hpp", "csharp", "go", "java", "javaclass", "jar", "kotlin",
    "ruby", "php", "swift", "lua", "powershell", "shellcheck", "slint", "webassembly",
    "makefile", "cmake", "docker", "git", "markdown", "vue", "svelte", "dart", "elixir",
    "haskell", "clojure", "scala", "perl", "r", "julia", "zig", "nim", "vlang",
    # 文件夹专用（按目录名匹配）
    "folder-src", "folder-node", "folder-git", "folder-github", "folder-vscode",
    "folder-config", "folder-dist", "folder-public", "folder-resource",
    "folder-images", "folder-docs", "folder-test", "folder-temp", "folder-target",
    "folder-lib", "folder-scripts", "folder-python", "folder-rust", "folder-javascript",
    "folder-typescript", "folder-java", "folder-go", "folder-css", "folder-views",
    "folder-components", "folder-home", "folder-download", "folder-audio",
    "folder-video", "folder-audio", "folder-desktop", "folder-project", "folder-repository",
    "folder-trash", "folder-archive", "folder-backup", "folder-include",
]

# 界面图标（Tabler，取 icons/outline/<name>.svg）
TABLER_ICONS = [
    "arrow-left", "arrow-right", "arrow-up", "refresh",
    "folder-plus", "trash", "eye", "eye-off",
    "search", "home", "file", "folder", "x",
]


def gh_content(repo: str, path: str) -> bytes | None:
    """用 gh api 取一个文件，返回原始字节。"""
    proc = subprocess.run(
        ["gh", "api", f"repos/{repo}/contents/{path}", "--jq", ".content"],
        capture_output=True, text=True,
    )
    if proc.returncode != 0:
        return None
    text = proc.stdout.strip()
    if not text:
        return None
    return base64.b64decode(text)


def normalize_tabler(svg: bytes) -> bytes:
    """把 currentColor 换成一个具体颜色。

    Slint 的 SVG 解析不认识 `currentColor`，会当成黑色；深色主题下等于看不见。
    换成白色后由 Slint 的 `colorize` 按主题色重新上色。
    """
    text = svg.decode("utf-8")
    text = text.replace("currentColor", "#ffffff")
    return text.encode("utf-8")


def clean_material(svg: bytes) -> bytes:
    """去掉 XML 声明与多余空白；Material 图标是彩色，不动颜色。"""
    text = svg.decode("utf-8")
    text = re.sub(r"<\?xml[^>]*\?>", "", text)
    text = re.sub(r"<!--.*?-->", "", text, flags=re.S)
    return text.strip().encode("utf-8")


def upstream_icon_names(repo: str) -> set[str] | None:
    """取上游 icons/ 下的全部图标名（不含扩展名）。失败返回 None。"""
    proc = subprocess.run(
        ["gh", "api", f"repos/{repo}/git/trees/main?recursive=1", "--jq",
         '.tree[].path'],
        capture_output=True, text=True,
    )
    if proc.returncode != 0:
        return None
    names = set()
    for line in proc.stdout.splitlines():
        if line.startswith("icons/") and line.endswith(".svg"):
            rel = line[len("icons/"):-4]
            # Tabler 分 outline/filled 两套，这里统一按最后一段记名
            names.add(rel.split("/")[-1])
    return names


def verify_names() -> list[str]:
    """下载前先校验清单里的名字在上游是否真的存在。

    上游会改图标名（例如 folder-music 已被 folder-audio 取代），静默改名会让
    某类文件突然退回通用图标却毫无提示。与其逐个下载到 404 才发现，不如先比一次清单。
    """
    problems = []
    for repo, names, kind in [
        (MATERIAL_REPO, MATERIAL_ICONS, "material"),
        (TABLER_REPO, TABLER_ICONS, "tabler"),
    ]:
        have = upstream_icon_names(repo)
        if have is None:
            print(f"  (跳过 {kind} 名校验：取不到上游清单)")
            continue
        missing = [n for n in names if n not in have]
        if missing:
            problems.extend(f"{kind}:{m}" for m in missing)
    return problems


def fetch_all(force: bool, dry_run: bool) -> int:
    MATERIAL_ICONS_DIR = ICON_ROOT / "material"
    TABLER_ICONS_DIR = ICON_ROOT / "tabler"

    jobs: list[tuple[str, str, pathlib.Path, str]] = []
    for name in MATERIAL_ICONS:
        jobs.append((MATERIAL_REPO, f"icons/{name}.svg", MATERIAL_ICONS_DIR / f"{name}.svg", "material"))
    for name in TABLER_ICONS:
        jobs.append((TABLER_REPO, f"icons/outline/{name}.svg", TABLER_ICONS_DIR / f"{name}.svg", "tabler"))

    if dry_run:
        print(f"将下载 {len(jobs)} 个图标：")
        for repo, path, dest, kind in jobs:
            print(f"  [{kind:8}] {dest.relative_to(REPO)}   <- {repo}/{path}")
        return 0

    MATERIAL_ICONS_DIR.mkdir(parents=True, exist_ok=True)
    TABLER_ICONS_DIR.mkdir(parents=True, exist_ok=True)

    done, skipped, failed = 0, 0, []
    for repo, path, dest, kind in jobs:
        if dest.exists() and not force:
            skipped += 1
            continue
        svg = gh_content(repo, path)
        if svg is None:
            failed.append(f"{kind}:{path}")
            print(f"  ✗ {path}", file=sys.stderr)
            continue
        data = normalize_tabler(svg) if kind == "tabler" else clean_material(svg)
        dest.write_bytes(data)
        done += 1
        print(f"  ✓ {dest.relative_to(REPO)}  ({len(data)} 字节)")

    print(f"\n新下载 {done}，已存在跳过 {skipped}，失败 {len(failed)}")
    if failed:
        print("失败清单：")
        for f in failed:
            print("   ", f)
        return 1
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description="下载文件/文件夹与界面图标")
    ap.add_argument("--force", action="store_true", help="已存在也重新下载")
    ap.add_argument("--list", action="store_true", help="只列清单，不下载")
    args = ap.parse_args()

    if subprocess.run(["sh", "-c", "command -v gh"], capture_output=True).returncode != 0:
        print("需要 gh CLI（raw.githubusercontent.com 在本机被劫持，匿名 API 配额也不够）", file=sys.stderr)
        return 2

    missing = verify_names()
    if missing:
        print("清单里有上游不存在的图标名（需要改名或删除）：")
        for m in missing:
            print("   ", m)
        return 1

    return fetch_all(args.force, args.list)


if __name__ == "__main__":
    raise SystemExit(main())
