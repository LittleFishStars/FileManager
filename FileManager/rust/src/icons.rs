//! 图标：随二进制内嵌的 SVG，以及「文件名 -> 图标名」的映射。
//!
//! 两个上游（见 `scripts/fetch_icons.py`，图标文件本身随源码入库）：
//!   * 文件/文件夹图标 —— Material Icon Theme（多色）
//!   * 界面图标       —— Tabler Icons（单色，运行时用 Slint 的 `colorize` 上色）
//!
//! 为什么内嵌而不是运行时读目录：
//!   `.slint` 里的 `@image-url` 是**编译期**解析的，路径写错会直接编译失败，
//!   这是好事；但它要求图标必须在编译期就能定位。用 `include_bytes!` 把 SVG
//!   嵌进二进制后，运行时不再依赖任何外部文件，也不增加启动时的磁盘 IO。
//!   代价是二进制变大（当前约 550 KB，可接受）。
//!
//! 为什么用符号名而不是枚举：
//!   符号名与上游文件名一致（`rust`、`folder-src`、`python`…），
//!   对照 `rust/icons/` 目录即可核对；加新类型只要加一行映射 + 一个图标文件。

use std::cell::RefCell;
use std::collections::HashMap;

/// 图标名 -> Slint 图片。按需解码并缓存。
///
/// 首次使用时才解码（SVG 解析有成本），之后直接命中缓存；
/// 缓存是纯内存查找，不引入任何周期性唤醒，静止时依旧零 CPU。
#[derive(Default)]
pub struct IconCache {
    images: RefCell<HashMap<String, slint::Image>>,
}

impl IconCache {
    pub fn get(&self, name: &str) -> Option<slint::Image> {
        if let Some(img) = self.images.borrow().get(name) {
            return Some(img.clone());
        }
        let bytes = icon_bytes(name)?;
        let image = slint::Image::load_from_data(bytes, Some("svg")).ok()?;
        self.images
            .borrow_mut()
            .insert(name.to_string(), image.clone());
        Some(image)
    }

    /// 已缓存的图标数量。目前只有测试用它核对缓存是否覆盖了全部图标，
    /// 保留是为了让测试能发现「名字存在但静默没进缓存」这类问题。
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.images.borrow().len()
    }
}

/// 图标名 -> 内嵌 SVG 字节。名字不存在时返回 None（调用方回退到通用图标）。
pub fn icon_bytes(name: &str) -> Option<&'static [u8]> {
    let b: &'static [u8] = match name {
        // ---- Material Icon Theme（文件 / 文件夹，多色，不 colorize）----
        "document" => include_bytes!("../icons/material/document.svg"),
        "textlint" => include_bytes!("../icons/material/textlint.svg"),
        "settings" => include_bytes!("../icons/material/settings.svg"),
        "tune" => include_bytes!("../icons/material/tune.svg"),
        "lock" => include_bytes!("../icons/material/lock.svg"),
        "key" => include_bytes!("../icons/material/key.svg"),
        "license" => include_bytes!("../icons/material/license.svg"),
        "readme" => include_bytes!("../icons/material/readme.svg"),
        "console" => include_bytes!("../icons/material/console.svg"),
        "log" => include_bytes!("../icons/material/log.svg"),
        "diff" => include_bytes!("../icons/material/diff.svg"),
        "template" => include_bytes!("../icons/material/template.svg"),
        "credits" => include_bytes!("../icons/material/credits.svg"),
        "authors" => include_bytes!("../icons/material/authors.svg"),
        "citation" => include_bytes!("../icons/material/citation.svg"),
        "image" => include_bytes!("../icons/material/image.svg"),
        "svg" => include_bytes!("../icons/material/svg.svg"),
        "audio" => include_bytes!("../icons/material/audio.svg"),
        "video" => include_bytes!("../icons/material/video.svg"),
        "subtitles" => include_bytes!("../icons/material/subtitles.svg"),
        "font" => include_bytes!("../icons/material/font.svg"),
        "lottie" => include_bytes!("../icons/material/lottie.svg"),
        "zip" => include_bytes!("../icons/material/zip.svg"),
        "disc" => include_bytes!("../icons/material/disc.svg"),
        "exe" => include_bytes!("../icons/material/exe.svg"),
        "dll" => include_bytes!("../icons/material/dll.svg"),
        "hex" => include_bytes!("../icons/material/hex.svg"),
        "lib" => include_bytes!("../icons/material/lib.svg"),
        "database" => include_bytes!("../icons/material/database.svg"),
        "table" => include_bytes!("../icons/material/table.svg"),
        "xml" => include_bytes!("../icons/material/xml.svg"),
        "epub" => include_bytes!("../icons/material/epub.svg"),
        "word" => include_bytes!("../icons/material/word.svg"),
        "powerpoint" => include_bytes!("../icons/material/powerpoint.svg"),
        "pdf" => include_bytes!("../icons/material/pdf.svg"),
        "typst" => include_bytes!("../icons/material/typst.svg"),
        "tex" => include_bytes!("../icons/material/tex.svg"),
        "json" => include_bytes!("../icons/material/json.svg"),
        "yaml" => include_bytes!("../icons/material/yaml.svg"),
        "toml" => include_bytes!("../icons/material/toml.svg"),
        "editorconfig" => include_bytes!("../icons/material/editorconfig.svg"),
        "buildkite" => include_bytes!("../icons/material/buildkite.svg"),
        "hosts" => include_bytes!("../icons/material/hosts.svg"),
        "rust" => include_bytes!("../icons/material/rust.svg"),
        "python" => include_bytes!("../icons/material/python.svg"),
        "python-misc" => include_bytes!("../icons/material/python-misc.svg"),
        "javascript" => include_bytes!("../icons/material/javascript.svg"),
        "typescript" => include_bytes!("../icons/material/typescript.svg"),
        "react" => include_bytes!("../icons/material/react.svg"),
        "react_ts" => include_bytes!("../icons/material/react_ts.svg"),
        "c" => include_bytes!("../icons/material/c.svg"),
        "h" => include_bytes!("../icons/material/h.svg"),
        "cpp" => include_bytes!("../icons/material/cpp.svg"),
        "hpp" => include_bytes!("../icons/material/hpp.svg"),
        "csharp" => include_bytes!("../icons/material/csharp.svg"),
        "go" => include_bytes!("../icons/material/go.svg"),
        "java" => include_bytes!("../icons/material/java.svg"),
        "javaclass" => include_bytes!("../icons/material/javaclass.svg"),
        "jar" => include_bytes!("../icons/material/jar.svg"),
        "kotlin" => include_bytes!("../icons/material/kotlin.svg"),
        "ruby" => include_bytes!("../icons/material/ruby.svg"),
        "php" => include_bytes!("../icons/material/php.svg"),
        "swift" => include_bytes!("../icons/material/swift.svg"),
        "lua" => include_bytes!("../icons/material/lua.svg"),
        "powershell" => include_bytes!("../icons/material/powershell.svg"),
        "shellcheck" => include_bytes!("../icons/material/shellcheck.svg"),
        "slint" => include_bytes!("../icons/material/slint.svg"),
        "webassembly" => include_bytes!("../icons/material/webassembly.svg"),
        "makefile" => include_bytes!("../icons/material/makefile.svg"),
        "cmake" => include_bytes!("../icons/material/cmake.svg"),
        "docker" => include_bytes!("../icons/material/docker.svg"),
        "git" => include_bytes!("../icons/material/git.svg"),
        "markdown" => include_bytes!("../icons/material/markdown.svg"),
        "vue" => include_bytes!("../icons/material/vue.svg"),
        "svelte" => include_bytes!("../icons/material/svelte.svg"),
        "dart" => include_bytes!("../icons/material/dart.svg"),
        "elixir" => include_bytes!("../icons/material/elixir.svg"),
        "haskell" => include_bytes!("../icons/material/haskell.svg"),
        "clojure" => include_bytes!("../icons/material/clojure.svg"),
        "scala" => include_bytes!("../icons/material/scala.svg"),
        "perl" => include_bytes!("../icons/material/perl.svg"),
        "r" => include_bytes!("../icons/material/r.svg"),
        "julia" => include_bytes!("../icons/material/julia.svg"),
        "zig" => include_bytes!("../icons/material/zig.svg"),
        "nim" => include_bytes!("../icons/material/nim.svg"),
        "vlang" => include_bytes!("../icons/material/vlang.svg"),
        // 文件夹专用
        "folder-src" => include_bytes!("../icons/material/folder-src.svg"),
        "folder-node" => include_bytes!("../icons/material/folder-node.svg"),
        "folder-git" => include_bytes!("../icons/material/folder-git.svg"),
        "folder-github" => include_bytes!("../icons/material/folder-github.svg"),
        "folder-vscode" => include_bytes!("../icons/material/folder-vscode.svg"),
        "folder-config" => include_bytes!("../icons/material/folder-config.svg"),
        "folder-dist" => include_bytes!("../icons/material/folder-dist.svg"),
        "folder-public" => include_bytes!("../icons/material/folder-public.svg"),
        "folder-resource" => include_bytes!("../icons/material/folder-resource.svg"),
        "folder-images" => include_bytes!("../icons/material/folder-images.svg"),
        "folder-docs" => include_bytes!("../icons/material/folder-docs.svg"),
        "folder-test" => include_bytes!("../icons/material/folder-test.svg"),
        "folder-temp" => include_bytes!("../icons/material/folder-temp.svg"),
        "folder-target" => include_bytes!("../icons/material/folder-target.svg"),
        "folder-lib" => include_bytes!("../icons/material/folder-lib.svg"),
        "folder-scripts" => include_bytes!("../icons/material/folder-scripts.svg"),
        "folder-python" => include_bytes!("../icons/material/folder-python.svg"),
        "folder-rust" => include_bytes!("../icons/material/folder-rust.svg"),
        "folder-javascript" => include_bytes!("../icons/material/folder-javascript.svg"),
        "folder-typescript" => include_bytes!("../icons/material/folder-typescript.svg"),
        "folder-java" => include_bytes!("../icons/material/folder-java.svg"),
        "folder-go" => include_bytes!("../icons/material/folder-go.svg"),
        "folder-css" => include_bytes!("../icons/material/folder-css.svg"),
        "folder-views" => include_bytes!("../icons/material/folder-views.svg"),
        "folder-components" => include_bytes!("../icons/material/folder-components.svg"),
        "folder-home" => include_bytes!("../icons/material/folder-home.svg"),
        "folder-download" => include_bytes!("../icons/material/folder-download.svg"),
        "folder-audio" => include_bytes!("../icons/material/folder-audio.svg"),
        "folder-video" => include_bytes!("../icons/material/folder-video.svg"),
        "folder-desktop" => include_bytes!("../icons/material/folder-desktop.svg"),
        "folder-project" => include_bytes!("../icons/material/folder-project.svg"),
        "folder-repository" => include_bytes!("../icons/material/folder-repository.svg"),
        "folder-trash" => include_bytes!("../icons/material/folder-trash.svg"),
        "folder-archive" => include_bytes!("../icons/material/folder-archive.svg"),
        "folder-backup" => include_bytes!("../icons/material/folder-backup.svg"),
        "folder-include" => include_bytes!("../icons/material/folder-include.svg"),

        // ---- Tabler Icons（界面图标，单色，用 colorize 上色）----
        "ui-arrow-left" => include_bytes!("../icons/tabler/arrow-left.svg"),
        "ui-arrow-right" => include_bytes!("../icons/tabler/arrow-right.svg"),
        "ui-arrow-up" => include_bytes!("../icons/tabler/arrow-up.svg"),
        "ui-refresh" => include_bytes!("../icons/tabler/refresh.svg"),
        "ui-folder-plus" => include_bytes!("../icons/tabler/folder-plus.svg"),
        "ui-trash" => include_bytes!("../icons/tabler/trash.svg"),
        "ui-eye" => include_bytes!("../icons/tabler/eye.svg"),
        "ui-eye-off" => include_bytes!("../icons/tabler/eye-off.svg"),
        "ui-search" => include_bytes!("../icons/tabler/search.svg"),
        "ui-home" => include_bytes!("../icons/tabler/home.svg"),
        "ui-file" => include_bytes!("../icons/tabler/file.svg"),
        "ui-folder" => include_bytes!("../icons/tabler/folder.svg"),
        "ui-x" => include_bytes!("../icons/tabler/x.svg"),
        _ => return None,
    };
    Some(b)
}

/// 没有专属图标时用的通用文件图标。
pub const DEFAULT_FILE_ICON: &str = "document";
/// 没有专属图标时用的通用文件夹图标（Tabler 的 outline 文件夹，可着色）。
pub const DEFAULT_FOLDER_ICON: &str = "ui-folder";

/// 常见「按文件名识别」的文件（没有扩展名，或用完整文件名当类型）。
///
/// 这张表同时服务于图标与「类型」列：像 `Dockerfile`、`Makefile` 这类文件
/// 靠扩展名是认不出来的。
pub fn icon_for_file_name(file_name: &str) -> Option<&'static str> {
    let lower = file_name.to_ascii_lowercase();
    let name = match lower.as_str() {
        "dockerfile" | "containerfile" => "docker",
        "makefile" | "gnumakefile" | "bsdmakefile" => "makefile",
        "cmakelists.txt" => "cmake",
        "cargo.lock" => "lock",
        "cargo.toml" => "toml",
        "package.json" | "package-lock.json" | "composer.json" => "json",
        "tsconfig.json" | "jsconfig.json" => "json",
        "license" | "licence" | "license.md" | "license.txt" | "copying" => "license",
        "readme" | "readme.md" | "readme.txt" | "readme.rst" => "readme",
        "changelog" | "changelog.md" | "changes" | "history" => "log",
        "authors" | "contributors" => "authors",
        "credits" => "credits",
        "citation.cff" => "citation",
        ".gitignore" | ".gitattributes" | ".gitmodules" | ".gitconfig" => "git",
        ".editorconfig" => "editorconfig",
        ".env" | ".env.local" | ".env.example" => "tune",
        "hosts" => "hosts",
        ".bashrc" | ".bash_profile" | ".zshrc" | ".profile" | ".bash_aliases" => "console",
        ".vimrc" | ".gvimrc" => "settings",
        _ => return None,
    };
    Some(name)
}

/// 由图标名推断中文类型文案。
///
/// 类型文案与图标必须来自同一张表，否则会漂移：曾经 `.dockerfile` 拿到了
/// Docker 图标，类型列却因为另一套扩展名表没有它而显示「文件」。
/// 放在这里做单点推导，两边就不会再各写一份。
pub fn kind_label(icon_name: &str) -> &'static str {
    match icon_name {
        "image" | "svg" | "lottie" => "图片",
        "audio" => "音频",
        "video" | "subtitles" => "视频",
        "zip" | "disc" | "exe" | "dll" | "hex" | "lib" | "webassembly" => "二进制",
        "database" | "table" | "json" | "yaml" | "toml" | "xml" | "settings" | "editorconfig"
        | "hosts" | "tune" | "key" | "lock" => "配置",
        "markdown" | "document" | "log" | "diff" | "textlint" => "文本",
        "word" | "powerpoint" | "pdf" | "epub" | "tex" | "typst" | "citation" => "文档",
        "readme" | "license" | "authors" | "credits" => "文档",
        "makefile" | "cmake" | "docker" | "git" | "buildkite" => "构建",
        "font" => "字体",
        "template" => "模板",
        "console" | "powershell" | "shellcheck" | "python-misc" => "脚本",
        // 其余落在语言类图标上的一律算代码
        _ => "代码",
    }
}

/// 选择图标名的**唯一入口**。
///
/// 顺序：
///   1. 目录 -> 目录名精确匹配，未命中用通用文件夹图标；
///   2. 文件 -> 完整文件名匹配（`Dockerfile`、`Makefile`、`README.md`…），
///      再退到扩展名匹配，仍未命中用通用文件图标。
///
/// 先看文件名是必须的：`Dockerfile` 没有扩展名，而 `README.md` 的扩展名是 `md`，
/// 只看扩展名会把它们当成普通文档/Markdown。
pub fn icon_name_for_path(path: &std::path::Path, is_dir: bool) -> &'static str {
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    if is_dir {
        return icon_for_folder(&file_name).unwrap_or(DEFAULT_FOLDER_ICON);
    }
    if let Some(n) = icon_for_file_name(&file_name) {
        return n;
    }
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    icon_for_extension(&ext).unwrap_or(DEFAULT_FILE_ICON)
}

/// 按路径给出中文类型文案（内部先解析图标名，再查 [`kind_label`]）。
pub fn kind_label_for_path(path: &std::path::Path) -> &'static str {
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if let Some(n) = icon_for_file_name(&file_name) {
        return kind_label(n);
    }
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    match icon_for_extension(&ext) {
        Some(n) => kind_label(n),
        None => "文件",
    }
}

/// 图标名是否是可着色的单色图标（Tabler 那批）。
/// Material 的图标本身带色，不能 colorize，否则会被压成单色蒙版。
pub fn is_monochrome(name: &str) -> bool {
    name.starts_with("ui-")
}

/// 按扩展名选文件图标。
///
/// 映射依据是 Material Icon Theme 自己的 `fileIcons.ts`（见 fetch_icons.py 注释），
/// 不是凭印象写的；未收录的扩展名回退到 [`DEFAULT_FILE_ICON`]。
pub fn icon_for_extension(ext: &str) -> Option<&'static str> {
    let name = match ext {
        // 图片
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" | "avif" | "heic" | "tiff"
        | "tif" | "jfif" | "pjpeg" | "pjp" => "image",
        "svg" => "svg",
        "lottie" => "lottie",
        // 音频
        "mp3" | "flac" | "wav" | "m4a" | "opus" | "aac" | "mid" | "midi" | "aiff" | "alac"
        | "ape" | "wma" => "audio",
        // 视频
        "mp4" | "mkv" | "webm" | "mov" | "avi" | "flv" | "m4v" | "mpg" | "mpeg" | "ogv"
        | "vob" | "3gp" => "video",
        "srt" | "ass" | "vtt" | "sub" => "subtitles",
        // 压缩 / 镜像 / 二进制
        "zip" | "tar" | "gz" | "tgz" | "xz" | "zst" | "bz2" | "7z" | "rar" | "deb" | "rpm"
        | "apk" | "war" | "cab" => "zip",
        "iso" | "img" | "dmg" => "disc",
        "exe" | "msi" | "app" | "bat" | "cmd" => "exe",
        "dll" | "so" | "dylib" | "o" | "a" => "dll",
        "bin" | "dat" | "db-journal" => "hex",
        "lib" | "rlib" => "lib",
        "wasm" | "wat" => "webassembly",
        // 数据
        "db" | "sqlite" | "sqlite3" | "mdb" | "sql" => "database",
        "csv" | "tsv" | "xls" | "xlsx" | "ods" | "numbers" => "table",
        "json" | "jsonc" | "json5" | "ndjson" | "geojson" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "xml" | "plist" | "xaml" | "csproj" | "vcxproj" => "xml",
        "ini" | "conf" | "cfg" | "properties" | "editorconfig" => "settings",
        "env" | "dotenv" => "tune",
        "lock" => "lock",
        // 安全
        "key" | "pem" | "pub" | "asc" | "gpg" | "ppk" => "key",
        "crt" | "cer" | "der" | "p12" | "pfx" => "key",
        // 文档
        "pdf" => "pdf",
        "doc" | "docx" | "odt" | "rtf" | "pages" => "word",
        "ppt" | "pptx" | "odp" | "keynote" => "powerpoint",
        "epub" | "mobi" | "azw3" => "epub",
        "tex" | "sty" | "cls" | "dtx" => "tex",
        "typ" => "typst",
        "md" | "markdown" | "mdx" | "mdown" | "mkd" | "rst" | "adoc" | "asciidoc" => "markdown",
        "txt" | "text" | "nfo" | "me" => "document",
        "log" => "log",
        "diff" | "patch" => "diff",
        // 字体
        "ttf" | "otf" | "woff" | "woff2" | "eot" => "font",
        // 语言
        "rs" | "ron" => "rust",
        "py" | "pyi" | "pyw" | "pyx" | "ipynb" => "python",
        "whl" | "egg" | "pyc" | "pyd" => "python-misc",
        "js" | "mjs" | "cjs" => "javascript",
        "ts" | "mts" | "cts" => "typescript",
        "jsx" => "react",
        "tsx" => "react_ts",
        "vue" => "vue",
        "svelte" => "svelte",
        "c" => "c",
        "h" => "h",
        "cpp" | "cc" | "cxx" | "c++" => "cpp",
        "hpp" | "hh" | "hxx" | "h++" => "hpp",
        "cs" | "csx" => "csharp",
        "go" => "go",
        "java" => "java",
        "class" => "javaclass",
        "jar" => "jar",
        "kt" | "kts" => "kotlin",
        "rb" | "erb" | "gemspec" => "ruby",
        "php" | "phtml" => "php",
        "swift" => "swift",
        "lua" => "lua",
        "dart" => "dart",
        "ex" | "exs" => "elixir",
        "hs" | "lhs" => "haskell",
        "clj" | "cljs" | "cljc" | "edn" => "clojure",
        "scala" | "sbt" | "sc" => "scala",
        "pl" | "pm" => "perl",
        "r" | "rmd" | "rds" => "r",
        "jl" => "julia",
        "zig" => "zig",
        "nim" | "nims" => "nim",
        "v" | "vsh" => "vlang",
        "sh" | "bash" | "zsh" | "fish" | "ksh" | "csh" => "console",
        "ps1" | "psm1" | "psd1" => "powershell",
        "slint" => "slint",
        // 构建 / 配置
        "mk" | "mak" => "makefile",
        "cmake" => "cmake",
        "dockerfile" => "docker",
        "gitignore" | "gitattributes" | "gitmodules" => "git",
        "hosts" => "hosts",
        "yml.tmpl" | "tmpl" | "mustache" | "hbs" | "ejs" | "pug" | "jade" => "template",
        "license" | "licence" => "license",
        "readme" => "readme",
        "changelog" => "log",
        "authors" => "authors",
        "cff" => "citation",
        "credits" => "credits",
        "lint" | "stylelintrc" | "eslintrc" => "textlint",
        _ => return None,
    };
    Some(name)
}

/// 按目录名选文件夹图标（只认整名，不做前缀匹配，避免误判）。
pub fn icon_for_folder(dir_name: &str) -> Option<&'static str> {
    let lower = dir_name.to_ascii_lowercase();
    let name = match lower.as_str() {
        "src" | "source" => "folder-src",
        "node_modules" => "folder-node",
        ".git" => "folder-git",
        ".github" => "folder-github",
        ".vscode" | ".idea" | ".fleet" => "folder-vscode",
        "config" | "configs" | "conf" | ".config" | "dotfiles" => "folder-config",
        "dist" | "out" | "output" => "folder-dist",
        "public" | "static" => "folder-public",
        "assets" | "resources" | "res" => "folder-resource",
        "images" | "img" | "icons" | "pictures" => "folder-images",
        "docs" | "doc" | "documentation" => "folder-docs",
        "test" | "tests" | "spec" | "specs" | "__tests__" => "folder-test",
        "tmp" | "temp" | "cache" | ".cache" => "folder-temp",
        "target" | "build" | "cmake-build-debug" | "cmake-build-release" => "folder-target",
        "lib" | "libs" | "vendor" | "third_party" | "external" => "folder-lib",
        "scripts" | "script" | "bin" => "folder-scripts",
        "python" | ".venv" | "venv" | "env" | "__pycache__" => "folder-python",
        "rust" | "cargo" => "folder-rust",
        "js" | "javascript" => "folder-javascript",
        "ts" | "typescript" => "folder-typescript",
        "java" | "jvm" => "folder-java",
        "go" | "golang" => "folder-go",
        "css" | "styles" | "style" | "scss" | "sass" => "folder-css",
        "views" | "view" | "pages" | "layouts" => "folder-views",
        "components" | "component" | "widgets" => "folder-components",
        "home" => "folder-home",
        "download" | "downloads" => "folder-download",
        "audio" | "music" | "sounds" => "folder-audio",
        "video" | "videos" | "movies" => "folder-video",
        "desktop" => "folder-desktop",
        "projects" | "project" | "workspace" => "folder-project",
        "repository" | "repositorys" | "repos" => "folder-repository",
        "trash" | "trashcan" => "folder-trash",
        "archive" | "archives" | "old" => "folder-archive",
        "backup" | "backups" => "folder-backup",
        "include" | "includes" | "inc" | "headers" => "folder-include",
        _ => return None,
    };
    Some(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 映射表里出现的每个图标名都必须真的内嵌了文件。
    ///
    /// `.slint` 侧的 `@image-url` 是编译期解析的，但 Rust 侧是按名字查表的；
    /// 名字写错只会在运行时静默回退到通用图标，所以这里显式兜住。
    #[test]
    fn every_mapped_icon_exists() {
        let mut names: Vec<&str> = vec![DEFAULT_FILE_ICON, DEFAULT_FOLDER_ICON];

        // 覆盖映射函数会返回的所有名字：用一个较大的扩展名/目录名样本驱动
        let exts = [
            "png", "svg", "lottie", "mp3", "mp4", "srt", "zip", "iso", "exe", "so", "bin",
            "lib", "wasm", "db", "csv", "json", "yaml", "toml", "xml", "ini", "env", "lock",
            "key", "crt", "pdf", "doc", "ppt", "epub", "tex", "typ", "md", "txt", "log",
            "diff", "ttf", "rs", "py", "whl", "js", "ts", "jsx", "tsx", "vue", "svelte",
            "c", "h", "cpp", "hpp", "cs", "go", "java", "class", "jar", "kt", "rb", "php",
            "swift", "lua", "dart", "ex", "hs", "clj", "scala", "pl", "r", "jl", "zig",
            "nim", "v", "sh", "ps1", "slint", "mk", "cmake", "dockerfile", "gitignore",
            "hosts", "tmpl", "license", "readme", "changelog", "authors", "cff", "credits",
            "eslintrc",
        ];
        for e in exts {
            if let Some(n) = icon_for_extension(e) {
                names.push(n);
            }
        }
        let dirs = [
            "src", "node_modules", ".git", ".github", ".vscode", "config", "dist", "public",
            "assets", "images", "docs", "tests", "tmp", "target", "lib", "scripts", "venv",
            "rust", "js", "ts", "java", "go", "css", "views", "components", "home",
            "downloads", "music", "videos", "desktop", "projects", "repository", "trash",
            "archive", "backup", "include",
        ];
        for d in dirs {
            if let Some(n) = icon_for_folder(d) {
                names.push(n);
            }
        }

        let mut missing: Vec<&str> = names
            .into_iter()
            .filter(|n| icon_bytes(n).is_none())
            .collect();
        missing.sort_unstable();
        missing.dedup();
        assert!(missing.is_empty(), "映射表引用了不存在的图标: {missing:?}");
    }

    /// 每个内嵌图标都必须能被真正解码成 Image。
    ///
    /// 这一条覆盖的是最危险的静默失败：图标字节存在、名字也对，但解码失败时
    /// `IconCache::get` 会返回 None，界面于是悄悄退回通用图标——功能看着正常，
    /// 实际所有类型图标都是错的。所以这里对每一个名字真的解一遍。
    #[test]
    fn every_icon_decodes() {
        // 从 icon_bytes 的 match 分支里取出全部名字，避免手写清单漏项
        // 注意：include_str! 会把本文件（含测试模块）一起读进来，
        // 必须先截掉 #[cfg(test)] 之后的内容，否则会把测试里的字面量也当成图标名。
        let src = include_str!("icons.rs");
        // 用 rfind 而不是 find：文件里可能还有别的 #[cfg(test)]（例如测试专用的方法），
        // 只有最后一个才是下方那个测试模块的起点。
        let src = &src[..src.rfind("#[cfg(test)]").expect("找不到测试模块边界")];
        // 形如:  "rust" => include_bytes!("../icons/material/rust.svg"),
        // 名字在 => 左边；只取带 include_bytes! 的行，避免抓成别的字符串。
        let mut names: Vec<&str> = src
            .lines()
            .filter(|l| l.contains("include_bytes!") && l.contains("=>"))
            .filter_map(|l| {
                let arrow = l.find("=>")?;
                let left = &l[..arrow];
                let start = left.find('"')? + 1;
                let end = left[start..].find('"')? + start;
                Some(left[start..end].trim())
            })
            .collect();
        names.sort_unstable();
        names.dedup();
        assert!(names.len() > 100, "只解析出 {} 个图标名，解析逻辑可能失效", names.len());

        let cache = IconCache::default();
        let mut failed = Vec::new();
        for name in &names {
            if cache.get(name).is_none() {
                failed.push(*name);
            }
        }
        assert!(failed.is_empty(), "以下图标内嵌了但解码失败: {failed:?}");
        assert_eq!(cache.len(), names.len(), "缓存数量与图标数量不一致");
    }

    /// 单一入口的行为：按名识别的文件不能退化成普通文档。
    #[test]
    fn path_selection_prefers_file_name() {
        use std::path::Path;
        // 没有扩展名，只能靠文件名认出来
        assert_eq!(icon_name_for_path(Path::new("/x/Dockerfile"), false), "docker");
        assert_eq!(icon_name_for_path(Path::new("/x/Makefile"), false), "makefile");
        assert_eq!(icon_name_for_path(Path::new("/x/LICENSE"), false), "license");
        // 有扩展名，但文件名更具体
        assert_eq!(icon_name_for_path(Path::new("/x/README.md"), false), "readme");
        assert_eq!(icon_name_for_path(Path::new("/x/Cargo.toml"), false), "toml");
        assert_eq!(icon_name_for_path(Path::new("/x/.gitignore"), false), "git");
        // 普通文件走扩展名
        assert_eq!(icon_name_for_path(Path::new("/x/main.rs"), false), "rust");
        // 认不出来要有兜底，不能 panic
        assert_eq!(icon_name_for_path(Path::new("/x/weird.zzz"), false), DEFAULT_FILE_ICON);
        assert_eq!(icon_name_for_path(Path::new("/x/random-folder"), true), DEFAULT_FOLDER_ICON);
        assert_eq!(icon_name_for_path(Path::new("/x/src"), true), "folder-src");
    }

    /// 图标与类型文案必须同源：类型由图标名推导，不能再各写一张表。
    #[test]
    fn kind_label_matches_icon() {
        use std::path::Path;
        // .dockerfile 曾经有图标却显示「文件」，这类漂移要能被这条测试拦住
        assert_eq!(kind_label_for_path(Path::new("/x/Dockerfile")), "构建");
        assert_eq!(kind_label_for_path(Path::new("/x/a.rs")), "代码");
        assert_eq!(kind_label_for_path(Path::new("/x/a.md")), "文本");
        assert_eq!(kind_label_for_path(Path::new("/x/a.png")), "图片");
        assert_eq!(kind_label_for_path(Path::new("/x/a.zip")), "二进制");
        assert_eq!(kind_label_for_path(Path::new("/x/a.toml")), "配置");
        assert_eq!(kind_label_for_path(Path::new("/x/a.zzz")), "文件");
    }

    /// 未收录的扩展名要回退，不能 panic。
    #[test]
    fn unknown_extension_falls_back() {
        assert!(icon_for_extension("this-is-not-a-real-ext").is_none());
        assert!(icon_for_folder("这是随便一个目录名").is_none());
    }
}
