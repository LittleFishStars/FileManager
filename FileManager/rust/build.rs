//! 构建脚本：在编译期把 .slint 界面编译成原生 Rust 代码。
//!
//! 视觉风格（SLINT_STYLE）是**编译期烘焙**进二进制的，所以必须显式声明这两个
//! rerun 条件，否则改环境变量或改界面文件时 cargo 不会重跑本脚本。

fn main() {
    println!("cargo:rerun-if-env-changed=SLINT_STYLE");
    println!("cargo:rerun-if-changed=ui");

    let style = std::env::var("SLINT_STYLE").unwrap_or_else(|_| "fluent".to_string());
    println!("cargo:warning=SLINT_STYLE = {style}");

    slint_build::compile_with_config(
        "ui/app.slint",
        slint_build::CompilerConfiguration::new().with_style(style),
    )
    .unwrap();
}
