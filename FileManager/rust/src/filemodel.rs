//! 目录读取、条目格式化与排序。
//!
//! 这一层刻意保持"纯函数 + 无 UI 依赖"，便于单测，也便于以后换成异步读取。

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// 一次目录读取的结果。
pub struct DirListing {
    pub entries: Vec<(PathBuf, fs::Metadata)>,
    /// 真正被读取的路径（经过符号链接解析后可能变化）。
    pub resolved: PathBuf,
    /// 读取失败的原因（权限不足等）。
    pub error: Option<String>,
}

/// 读取目录。`show_hidden` 为假时跳过 `.` 开头的条目。
///
/// 单个条目的元数据读取失败（断链符号链接、权限不足）不会让整次读取失败，
/// 而是跳过该条目——文件管理器里"部分可见"远好过"整页空白"。
pub fn list_dir(dir: &Path, show_hidden: bool) -> DirListing {
    let resolved = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());

    let read = match fs::read_dir(&resolved) {
        Ok(rd) => rd,
        Err(err) => {
            return DirListing {
                entries: Vec::new(),
                resolved,
                error: Some(describe_io_error(&err)),
            };
        }
    };

    let mut entries = Vec::new();
    for item in read.flatten() {
        let name = item.file_name().to_string_lossy().into_owned();
        if !show_hidden && name.starts_with('.') {
            continue;
        }
        // 用 metadata()（跟随符号链接）而不是 symlink_metadata()：
        // 文件管理器里指向目录的链接应当表现为目录。
        let Ok(meta) = fs::metadata(item.path()) else {
            continue;
        };
        entries.push((item.path(), meta));
    }

    DirListing {
        entries,
        resolved,
        error: None,
    }
}

/// 把 io 错误翻译成给用户看的中文短句。
pub fn describe_io_error(err: &std::io::Error) -> String {
    use std::io::ErrorKind::*;
    let text = match err.kind() {
        PermissionDenied => "没有访问权限",
        NotFound => "路径不存在",
        AlreadyExists => "同名文件已存在",
        _ => "操作失败",
    };
    format!("{text}（{}）", err)
}

/// 排序键：目录一律排在文件前面，同组内按 `descending` 决定方向。
///
/// `key` 为 `None` 时按文件名排序。
pub fn sort_entries(
    rows: &mut Vec<(PathBuf, fs::Metadata)>,
    key: Option<&str>,
    descending: bool,
) {
    let dir_first = |a: &(PathBuf, fs::Metadata), b: &(PathBuf, fs::Metadata)| {
        b.1.is_dir().cmp(&a.1.is_dir())
    };
    match key {
        Some("size") => rows.sort_by(|a, b| {
            dir_first(a, b).then_with(|| {
                let (x, y) = (a.1.len(), b.1.len());
                if descending { y.cmp(&x) } else { x.cmp(&y) }
            })
        }),
        Some("modified") => rows.sort_by(|a, b| {
            dir_first(a, b).then_with(|| {
                let (x, y) = (modified_secs(&a.1), modified_secs(&b.1));
                if descending { y.cmp(&x) } else { x.cmp(&y) }
            })
        }),
        Some("type") => rows.sort_by(|a, b| {
            dir_first(a, b).then_with(|| {
                let (x, y) = (extension_of(&a.0), extension_of(&b.0));
                let ord = x.cmp(&y).then_with(|| base_name(&a.0).cmp(&base_name(&b.0)));
                if descending { ord.reverse() } else { ord }
            })
        }),
        _ => rows.sort_by(|a, b| {
            dir_first(a, b).then_with(|| {
                let (x, y) = (base_name(&a.0), base_name(&b.0));
                let ord = x.to_lowercase().cmp(&y.to_lowercase());
                if descending { ord.reverse() } else { ord }
            })
        }),
    }
}

/// 文件名字符串（不含父目录）。
pub fn base_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// 小写扩展名；没有扩展名时返回空串。
pub fn extension_of(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

fn modified_secs(meta: &fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 人类可读的大小。目录没有"大小"的概念，返回破折号。
pub fn format_size(meta: &fs::Metadata) -> String {
    if meta.is_dir() {
        return "—".to_string();
    }
    let bytes = meta.len();
    const KIB: f64 = 1024.0;
    let value = bytes as f64;
    if value < KIB {
        format!("{bytes} B")
    } else if value < KIB * KIB {
        format!("{:.1} KiB", value / KIB)
    } else if value < KIB * KIB * KIB {
        format!("{:.1} MiB", value / (KIB * KIB))
    } else {
        format!("{:.1} GiB", value / (KIB * KIB * KIB))
    }
}

/// 修改时间，本地时区，`YYYY-MM-DD HH:MM`。无法读取时返回破折号。
pub fn format_modified(meta: &fs::Metadata) -> String {
    let Some(secs) = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
    else {
        return "—".to_string();
    };
    let local = secs + local_utc_offset_secs();
    let (y, m, d) = civil_from_days(local.div_euclid(86_400));
    let secs_of_day = local.rem_euclid(86_400);
    let (hh, mm) = (secs_of_day / 3600, (secs_of_day % 3600) / 60);
    format!("{y:04}-{m:02}-{d:02} {hh:02}:{mm:02}")
}

/// 当前本地时区相对 UTC 的秒数偏移。
///
/// 走 libc 的 `localtime_r`，避免为了这一件事引入 chrono；
/// `TZ` 变化会在进程内被 libc 缓存，单次启动读一次足够。
#[cfg(unix)]
fn local_utc_offset_secs() -> i64 {
    unsafe extern "C" {
        fn localtime_r(timep: *const i64, result: *mut Tm) -> *mut Tm;
    }
    #[repr(C)]
    struct Tm {
        tm_sec: i32,
        tm_min: i32,
        tm_hour: i32,
        tm_mday: i32,
        tm_mon: i32,
        tm_year: i32,
        tm_wday: i32,
        tm_yday: i32,
        tm_isdst: i32,
        tm_gmtoff: i64,
        tm_zone: *const i8,
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let mut out = Tm {
        tm_sec: 0,
        tm_min: 0,
        tm_hour: 0,
        tm_mday: 0,
        tm_mon: 0,
        tm_year: 0,
        tm_wday: 0,
        tm_yday: 0,
        tm_isdst: 0,
        tm_gmtoff: 0,
        tm_zone: std::ptr::null(),
    };
    // SAFETY: `now` 与 `out` 都是本函数栈上的有效对象，libc 只做写入。
    let ok = unsafe { !localtime_r(&now as *const i64, &mut out as *mut Tm).is_null() };
    if ok { out.tm_gmtoff } else { 0 }
}

#[cfg(not(unix))]
fn local_utc_offset_secs() -> i64 {
    0
}

/// 由"距 1970-01-01 的天数"反推公历年月日（Howard Hinnant 的 civil_from_days）。
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// 给条目一个人类可读的类型描述（用于列表的「类型」列）。
///
/// 刻意不在这里维护第二张扩展名表：类型由 `icons` 模块选出的图标名推导，
/// 保证「图标」与「类型」永远一致。以前两处各写一份，结果 `.dockerfile`
/// 有 Docker 图标、类型列却显示「文件」。
pub fn describe_kind(path: &Path, is_dir: bool) -> String {
    if is_dir {
        return "文件夹".to_string();
    }
    let ext = extension_of(path);
    let suffix = if ext.is_empty() {
        String::new()
    } else {
        format!(" · .{ext}")
    };
    format!("{}{suffix}", crate::icons::kind_label_for_path(path))
}

/// 路径按显示需要缩写到家目录。
pub fn display_path(path: &Path, home: Option<&Path>) -> String {
    if let Some(home) = home {
        if let Ok(rest) = path.strip_prefix(home) {
            if rest.as_os_str().is_empty() {
                return "~".to_string();
            }
            return format!("~/{}", rest.display());
        }
    }
    path.display().to_string()
}

/// 词法归一化：折叠 `.` / `..` / 重复分隔符，不访问文件系统。
///
/// 之所以不用 `canonicalize`：向上导航到父目录时目标可能不存在，
/// 而用户仍然期望地址栏能正常显示。
pub fn normalize(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    if out.as_os_str().is_empty() {
        PathBuf::from("/")
    } else {
        out
    }
}

/// 解析一个 `.trashinfo` 文件，取出原始路径（freedesktop trash 规范）。
///
/// 格式只有两行有信息量：
/// ```text
/// [Trash Info]
/// Path=/home/me/%E6%96%B0%E5%BB%BA%E6%96%87%E4%BB%B6%E5%A4%B9
/// DeletionDate=2026-10-09T03:37:15
/// ```
/// `Path` 是 **URL 百分号编码**的，必须解码，否则中文文件名会显示成一串 `%E6%...`。
pub fn trashinfo_original_path(text: &str) -> Option<PathBuf> {
    for line in text.lines() {
        if let Some(raw) = line.strip_prefix("Path=") {
            return Some(PathBuf::from(percent_decode(raw.trim())));
        }
    }
    None
}

/// 只解码百分号转义（`%XX`）。不需要处理 `+`，因为 trashinfo 用的是 URL 编码而非表单编码。
///
/// 自己实现而不是引入 `percent-encoding`：只有十几行，且要处理的是「非法转义原样保留」
/// 这种容错行为，自己写更好控制。
pub fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// 回收站目录（家目录内的那一个）。
/// 挂载点上的回收站（`<挂载点>/.Trash-<uid>`）暂不支持，见 README 的「尚未实现」。
pub fn home_trash_dir(home: &Path) -> PathBuf {
    home.join(".local/share/Trash")
}

/// 读取家目录回收站里的条目。回收站不存在时返回空列表（不是错误：从没删过东西）。
pub fn list_trash(home: &Path) -> Vec<(PathBuf, fs::Metadata)> {
    let trash = home_trash_dir(home);
    let files_dir = trash.join("files");
    let info_dir = trash.join("info");
    let Ok(read) = fs::read_dir(&files_dir) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for item in read.flatten() {
        let stored = item.file_name().to_string_lossy().into_owned();
        let Ok(meta) = fs::metadata(item.path()) else {
            continue;
        };
        // 优先用 trashinfo 里的原始路径，这样中文名与原始位置都能还原；
        // 读不到就退回回收站内的存储名，至少不丢条目。
        let original = fs::read_to_string(info_dir.join(format!("{stored}.trashinfo")))
            .ok()
            .and_then(|text| trashinfo_original_path(&text))
            .unwrap_or_else(|| PathBuf::from(&stored));
        out.push((original, meta));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 家目录必须被缩写成 `~`，而侧边栏判断用的是原始路径。
    ///
    /// 这条测试钉住的是「展示路径 ≠ 判断路径」这个区分：曾经侧边栏拿缩写过后的
    /// 字符串去比原始路径，结果只有 /、/tmp、/run/media 会高亮，家目录下的项全失灵。
    /// trashinfo 里的 Path 是百分号编码的，必须解码；中文名尤其明显。
    #[test]
    fn trashinfo_decodes_percent_escapes() {
        let text = "[Trash Info]\nPath=/home/me/%E6%96%B0%E5%BB%BA%E6%96%87%E4%BB%B6%E5%A4%B9\nDeletionDate=2026-10-09T03:37:15\n";
        assert_eq!(
            trashinfo_original_path(text).unwrap(),
            PathBuf::from("/home/me/新建文件夹")
        );
        // 没有 Path 行 -> None，不要瞎猜
        assert!(trashinfo_original_path("[Trash Info]\nDeletionDate=x\n").is_none());
    }

    /// 非法转义要原样保留，不能吃掉字符或 panic。
    #[test]
    fn percent_decode_keeps_invalid_escapes() {
        assert_eq!(percent_decode("abc"), "abc");
        assert_eq!(percent_decode("a%20b"), "a b");
        assert_eq!(percent_decode("%E4%B8%AD%E6%96%87"), "中文");
        assert_eq!(percent_decode("100%"), "100%");      // 尾部孤立 %
        assert_eq!(percent_decode("%ZZ"), "%ZZ");        // 非十六进制
        assert_eq!(percent_decode("%2"), "%2");          // 截断
    }

    #[test]
    fn display_path_abbreviates_home() {
        let home = Path::new("/home/me");
        assert_eq!(display_path(home, Some(home)), "~");
        assert_eq!(display_path(Path::new("/home/me/Projects"), Some(home)), "~/Projects");
        // 家目录之外保持原样
        assert_eq!(display_path(Path::new("/tmp"), Some(home)), "/tmp");
        assert_eq!(display_path(Path::new("/"), Some(home)), "/");
        // 前缀相似但不是子目录，不能被误缩写
        assert_eq!(display_path(Path::new("/home/melon"), Some(home)), "/home/melon");
    }
}
