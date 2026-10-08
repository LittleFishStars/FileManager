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

    let model = Rc::new(VecModel::<FileEntry>::default());
    ui.set_files(ModelRc::from(model.clone()));

    let home = env::var_os("HOME").map(PathBuf::from).filter(|p| p.is_dir());
    let places = build_places(home.as_deref());
    ui.set_places(ModelRc::from(Rc::new(VecModel::from(places.clone()))));
    // 平台层参数：让 niri 能按 app-id 认领窗口（对 Wayland 生效，必须在 show 之前调用）。
    slint::set_xdg_app_id("filemanager").ok();

    let state = State {
        ui: ui.clone_strong(),
        all: Vec::new(),
        cwd: home.clone().unwrap_or_else(|| PathBuf::from("/")),
        home: home.clone(),
        model,
        back: Vec::new(),
        forward: Vec::new(),
        sort_key: 0,
        sort_desc: false,
        query: String::new(),
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

    /// 重新读盘。
    fn reload(&mut self) {
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
                entry
            })
            .collect();

        let total = visible.len();
        self.model.set_vec(visible);

        self.ui.set_current_path(slint::SharedString::from(
            fm::display_path(&self.cwd, self.home.as_deref()),
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
        let name = row.name.to_string();
        if name.is_empty() || name == ".." {
            return None;
        }
        Some(self.cwd.join(name))
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
    let (icon_kind, type_text) = fm::kind_of(path, is_dir);
    FileEntry {
        name: slint::SharedString::from(fm::base_name(path)),
        path: slint::SharedString::from(path.display().to_string()),
        is_dir,
        size_text: slint::SharedString::from(fm::format_size(meta)),
        modified_text: slint::SharedString::from(fm::format_modified(meta)),
        icon_kind,
        type_text: slint::SharedString::from(type_text),
        alt: false,   // 由 refresh_model 按行号覆盖
    }
}

/// 侧边栏入口：只收录真实存在的目录。
fn build_places(home: Option<&Path>) -> Vec<Place> {
    let mut places = Vec::new();
    let mut push = |label: &str, path: PathBuf| {
        if path.is_dir() {
            places.push(Place {
                label: slint::SharedString::from(label.to_string()),
                path: slint::SharedString::from(path.display().to_string()),
            });
        }
    };

    push("根目录 /", PathBuf::from("/"));
    if let Some(home) = home {
        push("主目录", home.to_path_buf());
        for (label, dir) in [
            ("桌面", "Desktop"),
            ("文稿", "Documents"),
            ("下载", "Downloads"),
            ("图片", "Pictures"),
            ("音乐", "Music"),
            ("视频", "Videos"),
            ("项目", "Projects"),
            ("配置", ".config"),
        ] {
            push(label, home.join(dir));
        }
    }
    // XDG 之外的常见挂载点
    for (label, dir) in [("临时文件", "/tmp"), ("外部设备", "/run/media")] {
        push(label, PathBuf::from(dir));
    }
    places
}
