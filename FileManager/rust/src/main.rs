//! 文件管理器：Fluent / Material Design 3 风格的 Linux 原生文件管理器。
//!
//! 结构：
//!   * `ui/app.slint`  —— 编译期编译成原生 Rust 的界面（无 WebView / 无 JS 引擎 / 无 GC）
//!   * `filemodel.rs`  —— 目录读取、排序、格式化，纯逻辑无 UI 依赖
//!   * 本文件          —— 把两者接线：导航、打开、重命名、删除、过滤
//!
//! 全部逻辑跑在 UI 线程上。目录读取是同步的，因为单目录 `read_dir` 在本地磁盘上
//! 通常远低于一帧的预算；等真的遇到网络盘再用 `spawn_local` 挪出去。

mod filemodel;
mod icons;

slint::include_modules!();

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use slint::{ComponentHandle, Model, ModelRc, VecModel};

use filemodel as fm;

/// 与应用界面一一对应的可变状态。
struct State {
    ui: AppWindow,
    /// 当前目录的**全量**条目（未过滤）。过滤只影响 `files` 这个展示模型，
    /// 这样清空搜索框时无需重新读盘。
    all: Vec<(PathBuf, fs::Metadata)>,
    cwd: PathBuf,
    home: Option<PathBuf>,
    /// 展示模型。与 UI 里的 `files` 是同一个对象。
    model: Rc<VecModel<FileEntry>>,
    back: Vec<PathBuf>,
    forward: Vec<PathBuf>,
    sort_key: u8,
    sort_desc: bool,
    query: String,
    /// 图标缓存。按需解码 + 内存缓存，不引入任何周期性唤醒。
    icons: icons::IconCache,
    /// 当前看的是普通目录还是回收站。回收站不是普通目录：
    /// 里面是 trashinfo 还原出来的「原始路径」，而且删除语义是永久删除。
    location: Location,
}

/// 当前浏览位置。
#[derive(Debug, Clone, PartialEq, Eq)]
enum Location {
    Dir,
    /// 正在浏览回收站。存它自己的路径，避免每帧重算。
    Trash(PathBuf),
}

fn sort_key_name(key: u8) -> Option<&'static str> {
    match key {
        1 => Some("size"),
        2 => Some("modified"),
        3 => Some("type"),
        _ => None,
    }
}

/// 编译期烘焙进来的风格名（由 build.rs 以环境变量传给 rustc）。
const BUILTIN_STYLE: &str = env!("FILEMANAGER_STYLE");

fn main() -> Result<(), slint::PlatformError> {

    let ui = AppWindow::new()?;

    // 状态栏显示编译期烘焙的风格，避免「以为在测 Material 其实跑的是 Fluent」。
    ui.set_style_name(slint::SharedString::from(BUILTIN_STYLE));

    // 通用文件图标在启动时解析一次，之后所有「未收录类型」的行共用同一个 Image，
    // 避免 ListView 每构造一行就重新解析一次 SVG。
    let icon_cache = icons::IconCache::default();
    if let Some(img) = icon_cache.get(icons::DEFAULT_FILE_ICON) {
        ui.set_default_icon(img);
    }

    let model = Rc::new(VecModel::<FileEntry>::default());
    ui.set_files(ModelRc::from(model.clone()));

    let home = env::var_os("HOME").map(PathBuf::from).filter(|p| p.is_dir());
    let (places_primary, places_folders) = build_places(home.as_deref());
    ui.set_places_primary(ModelRc::from(Rc::new(VecModel::from(places_primary))));
    ui.set_places_folders(ModelRc::from(Rc::new(VecModel::from(places_folders))));
    // 平台层参数：让 niri 能按 app-id 认领窗口（对 Wayland 生效，必须在 show 之前调用）。
    slint::set_xdg_app_id("filemanager").ok();

    // 允许 `filemanager [目录]` 指定启动目录：从终端直接打开某个路径很方便，
    // 也便于截图核对图标。参数无效或未给时回退到家目录。
    let start_dir = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .or_else(|| home.clone())
        .unwrap_or_else(|| PathBuf::from("/"));

    let state = State {
        ui: ui.clone_strong(),
        all: Vec::new(),
        cwd: start_dir,
        home: home.clone(),
        model,
        back: Vec::new(),
        forward: Vec::new(),
        sort_key: 0,
        sort_desc: false,
        query: String::new(),
        icons: icon_cache,
        location: Location::Dir,
    };

    wire(&ui);
    // 状态先入槽，再跑首次读盘——回调里统一从 STATE 取。
    STATE.with(|slot| *slot.borrow_mut() = Some(state));
    STATE.with(|slot| {
        if let Some(state) = slot.borrow_mut().as_mut() {
            state.navigate_to(state.cwd.clone());
        }
    });

    let bounds = ui.window().scale_factor();
    ui.set_status_text(slint::SharedString::from(format!(
        "就绪 · 窗口缩放 {bounds:.2}x"
    )));

    ui.run()
}

// ---------------------------------------------------------------------------
// 回调接线
// ---------------------------------------------------------------------------

fn wire(ui: &AppWindow) {
    let ui = ui.clone_strong();

    {
        let weak = ui.as_weak();
        ui.on_navigate(move |path| {
            with_state(&weak, |st| st.navigate_to(PathBuf::from(path.as_str())));
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_go_back(move || with_state(&weak, |st| st.go_back()));
    }
    {
        let weak = ui.as_weak();
        ui.on_go_forward(move || with_state(&weak, |st| st.go_forward()));
    }
    {
        let weak = ui.as_weak();
        ui.on_go_up(move || with_state(&weak, |st| st.go_up()));
    }
    {
        let weak = ui.as_weak();
        ui.on_refresh(move || with_state(&weak, |st| st.reload()));
    }
    {
        let weak = ui.as_weak();
        ui.on_toggle_hidden(move || {
            with_state(&weak, |st| {
                let next = !st.ui.get_show_hidden();
                st.ui.set_show_hidden(next);
                st.reload();
            });
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_set_sort(move |mode, descending| {
            with_state(&weak, |st| {
                st.sort_key = mode.clamp(0, 3) as u8;
                st.sort_desc = descending;
                st.ui.set_sort_mode(mode);
                st.ui.set_sort_descending(descending);
                st.refresh_model();
            });
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_open_selected(move || with_state(&weak, |st| st.open_selected(false)));
    }
    {
        let weak = ui.as_weak();
        ui.on_open_external(move |path| {
            with_state(&weak, |st| st.open_external(Path::new(path.as_str())));
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_search_edited(move |text| {
            with_state(&weak, |st| {
                st.query = text.to_lowercase();
                st.refresh_model();
            });
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_rename_selected(move |new_name| {
            with_state(&weak, |st| st.rename_selected(new_name.as_str()));
        });
    }
    {
        let weak = ui.as_weak();
        ui.on_delete_selected(move || with_state(&weak, |st| st.delete_selected()));
    }
    {
        let weak = ui.as_weak();
        ui.on_create_folder(move || with_state(&weak, |st| st.create_folder()));
    }
}

/// 把弱引用升级成 `&mut State`，在里面跑一段操作。
///
/// 回调闭包只捕获 `slint::Weak`，从不捕获强句柄：组件句柄本身是强引用，
/// 回调里再捕获一个就会形成 `组件 → 回调 → 组件` 的环，窗口永不释放。
fn with_state(weak: &slint::Weak<AppWindow>, f: impl FnOnce(&mut State)) {
    let Some(ui) = weak.upgrade() else { return };
    STATE.with(|slot| {
        let mut borrowed = slot.borrow_mut();
        if let Some(state) = borrowed.as_mut() {
            state.ui = ui.clone_strong();
            f(state);
        }
    });
}

thread_local! {
    /// 全局唯一的应用状态。放在 thread_local 里，回调只需捕获弱引用即可访问，
    /// 从根上避免强引用环。
    static STATE: std::cell::RefCell<Option<State>> = const { std::cell::RefCell::new(None) };
}

// ---------------------------------------------------------------------------
// State 实现
// ---------------------------------------------------------------------------

impl State {
    /// 导航到某个目录（会清空前进历史）。
    fn navigate_to(&mut self, path: PathBuf) {
        let target = fm::normalize(&path);
        if !target.is_dir() {
            self.ui.set_status_text(slint::SharedString::from(format!(
                "无法进入：{}",
                target.display()
            )));
            return;
        }
        if target != self.cwd && !self.all.is_empty() {
            self.back.push(self.cwd.clone());
            self.forward.clear();
        }
        self.cwd = target;
        self.query.clear();
        self.ui.set_search_query(slint::SharedString::default());
        self.reload();
    }

    fn go_back(&mut self) {
        if let Some(prev) = self.back.pop() {
            self.forward.push(self.cwd.clone());
            self.cwd = prev;
            self.query.clear();
            self.ui.set_search_query(slint::SharedString::default());
            self.reload();
        }
    }

    fn go_forward(&mut self) {
        if let Some(next) = self.forward.pop() {
            self.back.push(self.cwd.clone());
            self.cwd = next;
            self.query.clear();
            self.ui.set_search_query(slint::SharedString::default());
            self.reload();
        }
    }

    fn go_up(&mut self) {
        if let Some(parent) = self.cwd.parent() {
            let parent = parent.to_path_buf();
            self.navigate_to(parent);
        }
    }

    /// 重新读盘。回收站走另一条路：它不是普通目录。
    fn reload(&mut self) {
        // 先判断是不是走了回收站：回收站目录本身是普通目录，
        // 但列出来的应当是 trashinfo 还原后的「原始路径」。
        if let Some(home) = self.home.clone() {
            let trash = fm::home_trash_dir(&home);
            if trash.is_dir()
                && self.cwd.canonicalize().unwrap_or_else(|_| self.cwd.clone())
                    == trash.canonicalize().unwrap_or_else(|_| trash.clone())
            {
                self.location = Location::Trash(trash.clone());
            } else if matches!(self.location, Location::Trash(_)) {
                self.location = Location::Dir;
            }
        }

        if let Location::Trash(_) = self.location {
            let home = self.home.clone().unwrap_or_default();
            self.all = fm::list_trash(&home);
            self.ui.set_selected_index(-1);
            self.refresh_model();
            return;
        }

        let show_hidden = self.ui.get_show_hidden();
        let listing = fm::list_dir(&self.cwd, show_hidden);
        self.all = listing.entries;
        if listing.resolved != self.cwd {
            self.cwd = listing.resolved;
        }
        self.ui.set_selected_index(-1);
        self.refresh_model();

        if let Some(err) = listing.error {
            self.ui
                .set_status_text(slint::SharedString::from(format!("读取失败：{err}")));
        }
    }

    /// 只重排/重过滤，不读盘。
    fn refresh_model(&mut self) {
        let mut rows = self.all.clone();
        fm::sort_entries(&mut rows, sort_key_name(self.sort_key), self.sort_desc);

        // 斑马纹按「过滤后的行号」算，这样筛选时条纹依然稳。
        let visible: Vec<FileEntry> = rows
            .iter()
            .filter(|(path, _)| {
                self.query.is_empty() || fm::base_name(path).to_lowercase().contains(&self.query)
            })
            .enumerate()
            .map(|(index, (path, meta))| {
                let mut entry = entry_of(path, meta);
                entry.alt = index % 2 == 1;
                resolve_icon(&self.icons, &self.ui, path, meta.is_dir(), &mut entry);
                entry
            })
            .collect();

        let total = visible.len();
        self.model.set_vec(visible);

        // 展示用的路径（家目录缩写成 ~）与状态判断用的原始路径分开设置：
        // 侧边栏高亮必须比原始路径，否则家目录下的项永远匹配不上。
        self.ui.set_current_path(slint::SharedString::from(
            fm::display_path(&self.cwd, self.home.as_deref()),
        ));
        self.ui.set_current_path_raw(slint::SharedString::from(
            self.cwd.canonicalize().unwrap_or_else(|_| self.cwd.clone()).display().to_string(),
        ));
        self.ui.set_can_go_back(!self.back.is_empty());
        self.ui.set_can_go_forward(!self.forward.is_empty());
        self.ui.set_can_go_up(self.cwd.parent().is_some());

        let hidden_note = if self.ui.get_show_hidden() {
            "隐藏项已显示"
        } else {
            "隐藏项已过滤"
        };
        let scope = if self.query.is_empty() {
            String::new()
        } else {
            format!("（共 {} 项，已按「{}」筛选）", self.all.len(), self.query)
        };
        self.ui.set_status_text(slint::SharedString::from(format!(
            "{total} 项{scope} · {hidden_note}"
        )));

        // 列表区的空提示与状态栏分开：过滤无匹配、目录真为空、隐藏项被滤掉
        // 是三种不同情况，同一句话在两个位置重复出现会让人以为是渲染残留。
        self.ui.set_empty_message(slint::SharedString::from(if total > 0 {
            String::new()
        } else if !self.query.is_empty() {
            format!("没有匹配「{}」的条目", self.query)
        } else if self.ui.get_show_hidden() {
            "这个目录是空的".to_string()
        } else {
            "这个目录是空的（可能还有隐藏项，用工具栏的「隐藏项」开关查看）".to_string()
        }));
    }

    /// 选中项绝对路径（用**当前路径 + 名称**拼，不依赖展示模型里的 path 字段，
    /// 避免模型被外部替换后拿到过期路径）。
    fn selected_path(&self) -> Option<PathBuf> {
        let index = self.ui.get_selected_index();
        if index < 0 {
            return None;
        }
        let row = self.model.row_data(index as usize)?;
        // 展示模型里的 path 就是条目的真实位置：
        // 普通目录里是「当前目录 + 名称」，回收站里是 trashinfo 还原出的原始路径。
        // 用它对回收站尤其重要——那里不能靠 cwd.join(name)。
        let path = row.path.to_string();
        if path.is_empty() || path == ".." {
            return None;
        }
        Some(PathBuf::from(path))
    }

    /// 激活选中项：目录就进入，文件就交给系统默认应用。
    fn open_selected(&mut self, force_external: bool) {
        let Some(path) = self.selected_path() else {
            self.ui
                .set_status_text(slint::SharedString::from("没有选中任何条目"));
            return;
        };
        if path.is_dir() && !force_external {
            self.navigate_to(path);
        } else {
            self.open_external(&path);
        }
    }

    fn open_external(&mut self, path: &Path) {
        if !path.exists() {
            self.set_error(format!("不存在：{}", path.display()));
            return;
        }
        let result = Command::new("xdg-open").arg(path).spawn();
        match result {
            Ok(_) => self.ui.set_status_text(slint::SharedString::from(format!(
                "已交给系统默认应用：{}",
                fm::base_name(path)
            ))),
            Err(err) => self.set_error(format!("调用 xdg-open 失败：{err}")),
        }
    }

    fn rename_selected(&mut self, new_name: &str) {
        let new_name = new_name.trim();
        if new_name.is_empty() {
            self.set_error("新名字不能为空");
            return;
        }
        if new_name.contains('/') {
            self.set_error("新名字不能包含斜杠");
            return;
        }
        let Some(from) = self.selected_path() else {
            self.set_error("请先选中一个条目");
            return;
        };
        let to = self.cwd.join(new_name);
        if to == from {
            self.ui.set_status_text(slint::SharedString::from("名字未变化"));
            return;
        }
        if to.exists() {
            self.set_error(format!("已存在同名条目：{new_name}"));
            return;
        }
        match fs::rename(&from, &to) {
            Ok(()) => {
                self.reload();
                self.ui.set_status_text(slint::SharedString::from(format!(
                    "已重命名为 {new_name}"
                )));
            }
            Err(err) => self.set_error(format!("重命名失败：{}", fm::describe_io_error(&err))),
        }
    }

    /// 删除选中项：移入系统回收站（freedesktop trash 规范），不是永久删除。
    ///
    /// 走 `trash-put` / `gio trash` 这两个外部命令而不是引入 Rust crate：本机 trash-cli
    /// 已装，而且它们本身就是回收站规范的实现，行为与桌面环境一致。
    /// 有意不做二次确认对话框——Slint 各后端没有统一的模态对话框能力，
    /// 与其塞一个可能被小窗口裁掉的伪对话框，不如让操作本身可逆。
    fn delete_selected(&mut self) {
        let Some(target) = self.selected_path() else {
            self.set_error("请先选中一个条目");
            return;
        };
        let name = fm::base_name(&target);

        // 在回收站里按删除 = 永久删除（回收站本来就该有这个出口），
        // 而不是再往回收站里塞一次。
        if matches!(self.location, Location::Trash(_)) {
            let result = if target.is_dir() {
                fs::remove_dir_all(&target)
            } else {
                fs::remove_file(&target)
            };
            return match result {
                Ok(()) => {
                    // 回收站内的条目要连 info 一起清掉，否则重复删除会重建同名条目
                    self.purge_trash_entry(&target);
                    self.reload();
                    self.ui.set_status_text(slint::SharedString::from(format!(
                        "已永久删除：{name}"
                    )));
                }
                Err(err) => self.set_error(format!("永久删除失败：{}", fm::describe_io_error(&err))),
            };
        }
        let mut last_error = String::new();
        for tool in ["trash-put", "gio"] {
            let mut command = Command::new(tool);
            if tool == "gio" {
                command.args(["trash", "--"]);
            }
            match command.arg(&target).status() {
                Ok(status) if status.success() => {
                    self.reload();
                    self.ui.set_status_text(slint::SharedString::from(format!(
                        "已移入回收站：{name}"
                    )));
                    return;
                }
                Ok(status) => last_error = format!("{tool} 退出码 {status}"),
                Err(err) => last_error = format!("{tool} 无法执行：{err}"),
            }
        }
        self.set_error(format!("移入回收站失败（{last_error}）"));
    }

    fn create_folder(&mut self) {
        let mut candidate = String::from("新建文件夹");
        let mut n = 1;
        while self.cwd.join(&candidate).exists() {
            n += 1;
            candidate = format!("新建文件夹 {n}");
        }
        let path = self.cwd.join(&candidate);
        match fs::create_dir(&path) {
            Ok(()) => {
                self.reload();
                self.ui.set_status_text(slint::SharedString::from(format!(
                    "已新建 {candidate}"
                )));
            }
            Err(err) => self.set_error(format!("新建失败：{}", fm::describe_io_error(&err))),
        }
    }

    /// 彻底清除回收站里的一条：删掉 `files/` 里的存储副本，并清掉对应的 trashinfo。
    ///
    /// 注意不能拿 `selected_path()` 去删——在回收站里它给的是**原始路径**，
    /// 那个路径在移入回收站时就已经不存在了，对着它删除只会报「文件不存在」。
    /// 真正要删的是回收站内的存储副本，所以要按名字（或 trashinfo 里的记录）反查。
    fn purge_trash_entry(&mut self, original: &Path) {
        let Some(home) = self.home.clone() else { return };
        let trash = fm::home_trash_dir(&home);
        let files_dir = trash.join("files");
        let info_dir = trash.join("info");

        // 1) 多数情况下存储名与原名相同，先直接试
        let mut stored = files_dir.join(original);
        let mut found = stored.exists();

        // 2) 重名条目会被加后缀（foo.2 之类），这时按 trashinfo 还原出的原始路径找
        if !found {
            if let Ok(read) = fs::read_dir(&files_dir) {
                for item in read.flatten() {
                    let candidate = item.path();
                    let info = info_dir.join(format!(
                        "{}.trashinfo",
                        item.file_name().to_string_lossy()
                    ));
                    let matches_original = fs::read_to_string(&info)
                        .ok()
                        .and_then(|t| fm::trashinfo_original_path(&t))
                        .map(|p| p == original)
                        .unwrap_or(false);
                    if matches_original {
                        stored = candidate;
                        found = true;
                        break;
                    }
                }
            }
        }
        if !found {
            self.set_error(format!("在回收站里找不到 {} 对应的副本", original.display()));
            return;
        }

        let stored_name = stored
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let result = if stored.is_dir() {
            fs::remove_dir_all(&stored)
        } else {
            fs::remove_file(&stored)
        };
        match result {
            Ok(()) => {
                let _ = fs::remove_file(info_dir.join(format!("{stored_name}.trashinfo")));
            }
            Err(err) => self.set_error(format!("清除失败：{}", fm::describe_io_error(&err))),
        }
    }

    fn set_error(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.ui.set_status_text(slint::SharedString::from(message));
    }
}

// ---------------------------------------------------------------------------
// 辅助函数
// ---------------------------------------------------------------------------

/// 由「路径 + 元数据」构造一个界面条目。
fn entry_of(path: &Path, meta: &fs::Metadata) -> FileEntry {
    let is_dir = meta.is_dir();
    let type_text = fm::describe_kind(path, is_dir);
    FileEntry {
        name: slint::SharedString::from(fm::base_name(path)),
        path: slint::SharedString::from(path.display().to_string()),
        is_dir,
        size_text: slint::SharedString::from(fm::format_size(meta)),
        modified_text: slint::SharedString::from(fm::format_modified(meta)),
        icon: slint::Image::default(),   // 由 resolve_icon 填
        icon_mono: false,                // 由 resolve_icon 填
        type_text: slint::SharedString::from(type_text),
        alt: false,   // 由 refresh_model 按行号覆盖
    }
}

/// 给一个条目选图标并填进去。
///
/// 选择顺序：
///   1. 目录 -> 按目录名匹配（`src`、`node_modules`、`.git`…），未命中用通用文件夹图标；
///   2. 文件 -> 按扩展名匹配，未命中用 `default-icon`（启动时已解析好的通用文件图标）。
///
/// 特别注意 `.gitignore` 这类「文件名即类型」的文件：它们的扩展名是 `gitignore`，
/// 所以在按扩展名查之前，先用完整文件名当扩展名查一次。
fn resolve_icon(
    cache: &icons::IconCache,
    ui: &AppWindow,
    path: &Path,
    is_dir: bool,
    entry: &mut FileEntry,
) {
    let icon_name = icons::icon_name_for_path(path, is_dir);

    match cache.get(icon_name) {
        Some(img) => {
            entry.icon = img;
            entry.icon_mono = icons::is_monochrome(icon_name);
        }
        None => {
            // 理论上不会发生（icons 模块的测试会兜住映射表的完整性），
            // 真出现了就退回通用图标，而不是留一个空图标。
            entry.icon = ui.get_default_icon();
            entry.icon_mono = false;
        }
    }
}

/// 侧边栏入口，返回 (第一区, 第二区)：
///   第一区 —— 固定入口（主文件夹 / 回收站 / 根目录 / 临时文件 / 外部设备）
///   第二区 —— 家目录下扫到的文件夹
///
/// 两区的差别不只是位置：第一区是「写死的常见位置」，第二区是「按机器实际情况扫出来的」，
/// 所以换一台机器时第二区会自然跟着变，不用改代码。
///
/// 每条都存软链解析后的真实路径：读取目录时 `list_dir` 会 canonicalize，当前目录总是
/// 真实路径；这里若保留软链形式（某些发行版 /home 是软链），侧边栏高亮会落空。
fn build_places(home: Option<&Path>) -> (Vec<Place>, Vec<Place>) {
    let mut primary = Vec::new();
    let mut folders = Vec::new();
    // 去重按路径算：.config / .local 既在固定名单里，也会被第二区扫到
    let mut seen: Vec<String> = Vec::new();

    let mut push = |out: &mut Vec<Place>, seen: &mut Vec<String>, label: &str, path: PathBuf| {
        if !path.is_dir() {
            return;
        }
        let resolved = path.canonicalize().unwrap_or(path);
        let key = resolved.display().to_string();
        if seen.contains(&key) {
            return;
        }
        seen.push(key.clone());
        out.push(Place {
            label: slint::SharedString::from(label.to_string()),
            path: slint::SharedString::from(key),
        });
    };

    // ---- 第一区：固定入口 ----
    if let Some(home) = home {
        push(&mut primary, &mut seen, "主文件夹", home.to_path_buf());
        // 回收站：家目录内的 freedesktop 回收站。没有就不显示（从没删过东西时确实不存在）。
        let trash = fm::home_trash_dir(home);
        if trash.is_dir() {
            push(&mut primary, &mut seen, "回收站", trash);
        }
    }
    push(&mut primary, &mut seen, "根目录 /", PathBuf::from("/"));
    push(&mut primary, &mut seen, "临时文件", PathBuf::from("/tmp"));
    push(&mut primary, &mut seen, "外部设备", PathBuf::from("/run/media"));

    // ---- 第二区：家目录下扫到的文件夹 ----
    if let Some(home) = home {
        // 常见目录排前面，其余按名字排后面，这样顺序稳定、可预期
        const PREFERRED: &[&str] = &[
            "Desktop", "Documents", "Downloads", "Pictures", "Music", "Videos",
            "Projects", "Files", "Games", "Repository", ".config", ".local",
        ];
        let mut dirs: Vec<String> = Vec::new();
        if let Ok(read) = fs::read_dir(home) {
            for item in read.flatten() {
                let name = item.file_name().to_string_lossy().into_owned();
                // 常规隐藏目录不列，但 .config / .local 是明确要的
                if name.starts_with('.') && !matches!(name.as_str(), ".config" | ".local") {
                    continue;
                }
                if item.path().is_dir() {
                    dirs.push(name);
                }
            }
        }
        dirs.sort_by_key(|name| {
            let idx = PREFERRED.iter().position(|p| p == name);
            (idx.is_none(), idx.unwrap_or(usize::MAX), name.clone())
        });
        // 侧边栏不该长到需要滚动；固定入口已占几条，这里留 14 条上限
        for name in dirs.into_iter().take(14) {
            push(&mut folders, &mut seen, &name, home.join(&name));
        }
    }

    (primary, folders)
}
