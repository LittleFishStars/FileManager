#!/usr/bin/env bash
# 构建封装：绕过 rustup shim，并把 CARGO_HOME 指向工作区内目录。
#
# 为什么需要它：在 DSH 沙箱会话里 ~/.local/share/cargo 与
# ~/.local/share/rustup 都是只读的。这带来两个坑：
#
#   1. cargo / rustc 都是 rustup 的代理（shim），一旦 rustup 需要写
#      $RUSTUP_HOME/tmp 或更新 channel，就以 "Read-only file system" 失败。
#      例如某个依赖声明 rust-version = "1.83" 时，代理会去同步该 channel 然后崩掉。
#   2. RUSTUP_HOME 不能指向空目录 —— rustup 会找不到默认 toolchain，
#      直接报 "could not choose a version of cargo to run"。
#
# 因此这里把 cargo / rustc / rustdoc 全部钉到真实 toolchain 里的二进制，
# 只把 CARGO_HOME（下载缓存 + 镜像配置）重定向到 rust/.cargo-home/。
# 不在仓库里放 rust-toolchain.toml，避免真实 cargo 试图下载别的 toolchain。
#
# 在普通终端里直接用 `cargo build --release` 即可，不需要本脚本。
#
# 用法：
#   scripts/dev.sh check            # 只做类型检查，最快
#   scripts/dev.sh build            # debug 构建（opt-level = 1）
#   scripts/dev.sh release          # release 构建（性能结论只允许来自这里）
#   scripts/dev.sh run [release]    # 运行
#   scripts/dev.sh style material   # 用 Material M3 风格构建并运行
#   scripts/dev.sh clean

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUST_DIR="$ROOT/rust"

# --- 找到真实 toolchain ------------------------------------------------------
find_toolchain_bin() {
    # 显式优先 stable / nightly：有些 toolchain 目录存在但缺二进制（本机 1.95.0 就是），
    # 按字母序盲挑会挑到残缺的那个。
    local candidate version
    for version in stable nightly; do
        candidate="$HOME/.local/share/rustup/toolchains/${version}-x86_64-unknown-linux-gnu/bin"
        [[ -x "$candidate/rustc" && -x "$candidate/cargo" ]] && { echo "$candidate"; return; }
    done
    for candidate in "$HOME"/.local/share/rustup/toolchains/*/bin \
                     "$HOME"/.rustup/toolchains/*/bin; do
        [[ -x "$candidate/rustc" && -x "$candidate/cargo" ]] && { echo "$candidate"; return; }
    done
    echo ""
}
TOOLCHAIN_BIN="$(find_toolchain_bin)"

if [[ -n "$TOOLCHAIN_BIN" ]]; then
    export RUSTC="$TOOLCHAIN_BIN/rustc"
    export RUSTDOC="$TOOLCHAIN_BIN/rustdoc"
    CARGO="$TOOLCHAIN_BIN/cargo"
    : "${RUSTUP_HOME:=$HOME/.local/share/rustup}"
    export RUSTUP_HOME
else
    echo "警告：没找到真实 toolchain，回退到 PATH 里的 cargo（沙箱下可能失败）" >&2
    CARGO="cargo"
fi

# --- CARGO_HOME：只在系统缓存目录不可写时才重定向 ---------------------------
SYSTEM_CARGO_HOME="${SYSTEM_CARGO_HOME:-$HOME/.local/share/cargo}"
if ! mkdir -p "$SYSTEM_CARGO_HOME" 2>/dev/null \
   || ! touch "$SYSTEM_CARGO_HOME/.dsh-write-probe" 2>/dev/null; then
    export CARGO_HOME="$RUST_DIR/.cargo-home"
else
    rm -f "$SYSTEM_CARGO_HOME/.dsh-write-probe"
fi
mkdir -p "${CARGO_HOME:-$SYSTEM_CARGO_HOME}"

# 重定向之后系统 cargo 的镜像配置（rsproxy 等）就读不到了，补一份到工作区。
# 之后要改镜像只需要改 rust/.cargo-home/config.toml。
if [[ -n "${CARGO_HOME:-}" && "$CARGO_HOME" == "$RUST_DIR/.cargo-home" ]]; then
    if [[ ! -f "$CARGO_HOME/config.toml" && -f "$SYSTEM_CARGO_HOME/config.toml" ]]; then
        cp "$SYSTEM_CARGO_HOME/config.toml" "$CARGO_HOME/config.toml"
        echo "已把镜像配置复制到 $CARGO_HOME/config.toml"
    fi
fi

cd "$RUST_DIR"
ACTION="${1:-build}"
shift || true

case "$ACTION" in
    build)   exec "$CARGO" build "$@" ;;
    release) exec "$CARGO" build --release "$@" ;;
    check)   exec "$CARGO" check "$@" ;;
    test)    exec "$CARGO" test "$@" ;;
    clean)   exec "$CARGO" clean "$@" ;;
    run)
        if [[ "${1:-}" == "release" ]]; then
            shift
            exec "$CARGO" run --release "$@"
        fi
        exec "$CARGO" run "$@"
        ;;
    style)
        STYLE="${1:?用法: scripts/dev.sh style <fluent|material|cupertino|cosmic|qt|native>[-dark|-light]}"
        shift
        export SLINT_STYLE="$STYLE"
        echo "SLINT_STYLE=$SLINT_STYLE"
        exec "$CARGO" run --release "$@"
        ;;
    *)
        echo "未知动作：$ACTION" >&2
        echo "可用：check | build | release | run [release] | style <名称> | test | clean" >&2
        exit 2
        ;;
esac
