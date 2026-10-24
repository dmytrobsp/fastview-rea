//! «Проводник» — the oscillogram finder (FileFinder22Dlg, DIALOG 558; ledger
//! L9.9, `notes/UI_BEHAVIOUR_SPEC.md` §7.5): a modal dialog that scans a
//! folder (optionally with subfolders) on a worker thread for `.cfg`, `.cff`,
//! `.osc` and `.sto` files, lists them with station, block and start time,
//! filters them live and opens or deletes the selected ones.

use crate::app::App;
use eframe::egui;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};

/// A start time (`__time64` in the original), ordered by its fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Stamp {
    pub y: i32,
    pub mo: u32,
    pub d: u32,
    pub h: u32,
    pub mi: u32,
    pub s: u32,
}

impl Stamp {
    fn date(self) -> String {
        format!("{:02}/{:02}/{:04}", self.d, self.mo, self.y)
    }

    fn time(self) -> String {
        format!("{:02}:{:02}:{:02}", self.h, self.mi, self.s)
    }

    /// 1 January 1970, a zero CTime's date.
    pub const EPOCH: Stamp = Stamp { y: 1970, mo: 1, d: 1, h: 0, mi: 0, s: 0 };

    /// The system long date (Russian: «23 октября 2026 г.»), as the
    /// date pickers show it (DTS_LONGDATEFORMAT, no custom format).
    pub fn long_date(self) -> String {
        const MONTHS: [&str; 12] = ["января", "февраля", "марта", "апреля", "мая", "июня", "июля", "августа", "сентября", "октября", "ноября", "декабря"];
        format!("{} {} {} г.", self.d, MONTHS[(self.mo.clamp(1, 12) - 1) as usize], self.y)
    }

    /// The start of the following day (the inclusive upper bound of По).
    fn next_day(self) -> Stamp {
        let days = month_days(self.y, self.mo);
        let (mut y, mut mo, mut d) = (self.y, self.mo, self.d + 1);
        if d > days {
            d = 1;
            mo += 1;
            if mo > 12 {
                mo = 1;
                y += 1;
            }
        }
        Stamp { y, mo, d, h: 0, mi: 0, s: 0 }
    }
}

/// The days of month `mo` of year `y`.
pub fn month_days(y: i32, mo: u32) -> u32 {
    match mo {
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// The weekday of a date, Monday 0 … Sunday 6 (Sakamoto's method).
pub fn weekday(y: i32, mo: u32, d: u32) -> u32 {
    const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if mo < 3 { y - 1 } else { y };
    let sun0 = (y + y / 4 - y / 100 + y / 400 + T[(mo.clamp(1, 12) - 1) as usize] + d as i32).rem_euclid(7);
    ((sun0 + 6) % 7) as u32
}

/// A date picker (SysDateTimePick32, long date, drop-down calendar, no
/// «none» check): the button shows the long date; the calendar browses by
/// month and a day click sets the date, keeping the time of day.
fn date_picker(ui: &mut egui::Ui, at: egui::Rect, id: &str, st: &mut Stamp) -> bool {
    let popup = egui::Id::new(("fv_finder_date", id));
    let browse_id = popup.with("month");
    // Closed (VM): white in a (141) frame, the long date 4 px in, the
    // calendar image and its ▾ at the right (our drawing of the image).
    let r = ui.interact(at, popup.with("field"), egui::Sense::click());
    {
        use egui::{Color32, Pos2, Rect, Vec2};
        let p = ui.painter();
        p.rect_filled(at, 0.0, Color32::from_gray(141));
        p.rect_filled(at.shrink(1.0), 0.0, Color32::WHITE);
        p.text(Pos2::new(at.left() + 4.0, at.center().y), egui::Align2::LEFT_CENTER, st.long_date(), crate::dialogs::DLG_FONT, Color32::BLACK);
        let cal = Rect::from_min_size(Pos2::new(at.right() - 27.0, at.center().y - 6.0), Vec2::new(11.0, 12.0));
        p.rect_filled(cal, 0.0, Color32::from_rgb(119, 107, 104));
        p.rect_filled(cal.shrink(1.0), 0.0, Color32::from_rgb(234, 239, 247));
        for k in 0..3 {
            p.hline(cal.left() + 2.0..=cal.right() - 2.0, cal.top() + 3.5 + 2.5 * k as f32, egui::Stroke::new(1.0f32, Color32::from_gray(190)));
        }
        let c = Pos2::new(at.right() - 9.0, at.center().y);
        p.add(egui::Shape::convex_polygon(vec![c + Vec2::new(-3.0, -1.5), c + Vec2::new(3.0, -1.5), c + Vec2::new(0.0, 1.5)], Color32::from_rgb(77, 97, 133), egui::Stroke::NONE));
    }
    if r.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup));
        ui.data_mut(|d| d.insert_temp(browse_id, (st.y.max(1601), st.mo.clamp(1, 12))));
    }
    let mut changed = false;
    egui::popup_below_widget(ui, popup, &r, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
        const NAMES: [&str; 12] = ["Январь", "Февраль", "Март", "Апрель", "Май", "Июнь", "Июль", "Август", "Сентябрь", "Октябрь", "Ноябрь", "Декабрь"];
        let (mut y, mut mo) = ui.data(|d| d.get_temp::<(i32, u32)>(browse_id)).unwrap_or((st.y, st.mo.clamp(1, 12)));
        ui.horizontal(|ui| {
            if ui.small_button("◀").clicked() {
                (y, mo) = if mo == 1 { (y - 1, 12) } else { (y, mo - 1) };
            }
            ui.label(format!("{} {y}", NAMES[(mo - 1) as usize]));
            if ui.small_button("▶").clicked() {
                (y, mo) = if mo == 12 { (y + 1, 1) } else { (y, mo + 1) };
            }
        });
        egui::Grid::new(("fv_finder_cal", id)).spacing([2.0, 2.0]).show(ui, |ui| {
            for w in ["Пн", "Вт", "Ср", "Чт", "Пт", "Сб", "Вс"] {
                ui.label(egui::RichText::new(w).small());
            }
            ui.end_row();
            let lead = weekday(y, mo, 1);
            for _ in 0..lead {
                ui.label("");
            }
            for d in 1..=month_days(y, mo) {
                let on = (st.y, st.mo, st.d) == (y, mo, d);
                if ui.selectable_label(on, format!("{d:>2}")).clicked() {
                    (st.y, st.mo, st.d) = (y, mo, d);
                    changed = true;
                    ui.memory_mut(|m| m.close_popup());
                }
                if (lead + d).is_multiple_of(7) {
                    ui.end_row();
                }
            }
        });
        ui.data_mut(|d| d.insert_temp(browse_id, (y, mo)));
    });
    changed
}

/// One listed file.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub path: PathBuf,
    /// The path relative to the scanned folder (col 0).
    pub name: String,
    pub osc: bool,
    pub station: String,
    pub block: String,
    pub time: Stamp,
}

/// Header text: UTF-8 when valid, else Windows-1251 (Cyrillic letters).
fn decode(bytes: &[u8]) -> String {
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    bytes
        .iter()
        .map(|&b| match b {
            0..=0x7f => b as char,
            0xc0..=0xff => char::from_u32(0x410 + u32::from(b - 0xc0)).unwrap_or('?'),
            0xa8 => 'Ё',
            0xb8 => 'ё',
            _ => '?',
        })
        .collect()
}

/// The COMTRADE header reader (`0x46c080`): station and block from line 1
/// (block «-» without a second token), the start time from the line after
/// the channel, frequency and rate lines; `.cfg` dates are month first,
/// `.cff` day first, swapped when the month is above 12; years below 100 get
/// +2000.
pub fn read_comtrade_header(text: &str, cff: bool) -> Option<(String, String, Stamp)> {
    let lines: Vec<&str> = text.lines().filter(|l| !(cff && l.trim_start().starts_with("---"))).collect();
    let first: Vec<&str> = lines.first()?.split(',').collect();
    let station = first.first()?.trim().to_string();
    let block = first.get(1).map(|b| b.trim().to_string()).filter(|b| !b.is_empty()).unwrap_or_else(|| "-".into());
    let total: usize = lines.get(1)?.split(',').next()?.trim().parse().ok()?;
    let mut at = 2 + total + 1;
    let nrates: usize = lines.get(at)?.trim().parse().ok()?;
    at += 1 + nrates.max(1);
    let start = lines.get(at)?;
    let nums: Vec<i64> = start.split([',', '/', ':', '.']).filter_map(|t| t.trim().parse().ok()).collect();
    if nums.len() < 7 {
        return None;
    }
    let (mut mo, mut d) = if cff { (nums[1], nums[0]) } else { (nums[0], nums[1]) };
    if mo > 12 {
        std::mem::swap(&mut mo, &mut d);
    }
    let y = if nums[2] < 100 { nums[2] + 2000 } else { nums[2] };
    Some((station, block, Stamp { y: y as i32, mo: mo as u32, d: d as u32, h: nums[3] as u32, mi: nums[4] as u32, s: nums[5] as u32 }))
}

/// The OSC2 header reader (`0x46c730`): «OSC Осциллограф. Версия a.b»,
/// block «-», the time from header bytes 6..11 (s, min, h, day, month,
/// year − 2000).
/// `0x46c730`: the version from block 0's bytes 0 / 1, the time from the
/// info block (128 · H, H = byte 2) BCD-decoded — the trigger moment
/// without its milliseconds.
pub fn read_osc_header(b: &[u8]) -> Option<(String, String, Stamp)> {
    let h = usize::from(*b.get(2)?);
    let info = b.get(128 * h..128 * h + 12)?;
    let [y, mo, d, hh, mi, s, _] = fvb_core::comtrade::osc_trigger(info)?;
    let t = Stamp { y: y as i32, mo, d, h: hh, mi, s };
    Some((format!("OSC Осциллограф. Версия {}.{}", b[0], b[1]), "-".into(), t))
}

/// Read one file by its last three characters, lower-cased (`0x46bec0`).
fn read_entry(path: &Path, root: &Path) -> Option<Found> {
    let s = path.to_string_lossy().to_lowercase();
    let ext = &s[s.len().saturating_sub(3)..];
    let bytes = || std::fs::read(path).ok();
    let (station, block, time, osc) = match ext {
        "cfg" => {
            let (a, b, t) = read_comtrade_header(&decode(&bytes()?), false)?;
            (a, b, t, false)
        }
        "cff" => {
            let (a, b, t) = read_comtrade_header(&decode(&bytes()?), true)?;
            (a, b, t, false)
        }
        "osc" => {
            let (a, b, t) = read_osc_header(&bytes()?)?;
            (a, b, t, true)
        }
        _ => return None,
    };
    let name = path.strip_prefix(root).unwrap_or(path).to_string_lossy().into_owned();
    Some(Found { path: path.to_path_buf(), name, osc, station, block, time })
}

/// Walk `root` (recursively when asked), checking `cancel` at every step.
pub fn scan(root: &Path, recursive: bool, cancel: &AtomicBool) -> Vec<Found> {
    // FindFirstFile / FindNextFile with the recursion inline (`0x46bd20`):
    // a subfolder's files come where the subfolder is met.
    fn walk(dir: &Path, root: &Path, recursive: bool, cancel: &AtomicBool, out: &mut Vec<Found>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        let mut entries: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        entries.sort();
        for p in entries {
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            if p.is_dir() {
                if recursive {
                    walk(&p, root, recursive, cancel, out);
                }
            } else if let Some(f) = read_entry(&p, root) {
                out.push(f);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, recursive, cancel, &mut out);
    out
}

/// Files deleted together with `path` (`0x46d4b0`): the existing companions
/// of its kind.
pub fn companions(path: &Path) -> Vec<PathBuf> {
    let s = path.to_string_lossy().to_lowercase();
    let exts: &[&str] = match &s[s.len().saturating_sub(4)..] {
        ".cfg" => &["dat", "hdr", "ini", "inf", "set"],
        ".cff" | ".osc" => &["set"],
        ".sto" => &["dst"],
        _ => &[],
    };
    // The files that are actually there (by directory entry, so a
    // case-insensitive file system does not list one file twice), with the
    // same stem and a companion extension in either case.
    let mut out = vec![path.to_path_buf()];
    let (Some(dir), Some(stem)) = (path.parent(), path.file_stem()) else { return out };
    let Ok(entries) = std::fs::read_dir(if dir.as_os_str().is_empty() { Path::new(".") } else { dir }) else { return out };
    let mut found: Vec<(usize, PathBuf)> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.file_stem() == Some(stem) && p.is_file())
        .filter_map(|p| {
            let ext = p.extension()?.to_string_lossy().to_lowercase();
            exts.iter().position(|e| *e == ext).map(|i| (i, p))
        })
        .collect();
    found.sort();
    out.extend(found.into_iter().map(|(_, p)| p));
    out
}

/// The shell delete of `0x46d4b0`: one `SHFileOperationW` FO_DELETE over
/// all `paths` (double-NUL list, no owner window), `FOF_ALLOWUNDO` (the
/// recycle bin) unless `permanent` (Shift held) — the shell shows its own
/// confirmation, progress and errors. True when it returns 0 (then the
/// finder rescans; `fAnyOperationsAborted` is not looked at).
#[cfg(windows)]
pub fn shell_delete(paths: &[PathBuf], permanent: bool) -> bool {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::{SHFileOperationW, FOF_ALLOWUNDO, FO_DELETE, SHFILEOPSTRUCTW};
    let mut from: Vec<u16> = Vec::new();
    for p in paths {
        from.extend(p.as_os_str().encode_wide());
        from.push(0);
    }
    from.push(0);
    let mut op = SHFILEOPSTRUCTW {
        hwnd: std::ptr::null_mut(),
        wFunc: FO_DELETE,
        pFrom: from.as_ptr(),
        pTo: std::ptr::null(),
        fFlags: if permanent { 0 } else { FOF_ALLOWUNDO as u16 },
        fAnyOperationsAborted: 0,
        hNameMappings: std::ptr::null_mut(),
        lpszProgressTitle: std::ptr::null(),
    };
    // SAFETY: `from` is a double-NUL-terminated UTF-16 list that outlives
    // the call; the other pointers are null as the API allows.
    unsafe { SHFileOperationW(&mut op) == 0 }
}

/// Elsewhere: to the desktop's trash, or removed when `permanent`.
#[cfg(not(windows))]
pub fn shell_delete(paths: &[PathBuf], permanent: bool) -> bool {
    if permanent {
        paths.iter().filter(|p| std::fs::remove_file(p).is_err()).count() == 0
    } else {
        trash::delete_all(paths).is_ok()
    }
}

/// Sortable columns (the original's header click: ascending, again descending).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Column {
    Name,
    Station,
    Block,
    Date,
    Time,
}

/// Whether `f` passes the station / block / date filters (`0x46cb20`):
/// exact case-sensitive or «Все», `from ≤ t ≤ (to + 1 day)` at 00:00.
pub fn passes(f: &Found, station: &str, block: &str, from: Stamp, to: Stamp) -> bool {
    (station == ALL || f.station == station) && (block == ALL || f.block == block) && from <= f.time && f.time <= to.next_day()
}

const ALL: &str = "Все";

/// A node of the folder pane (`CMFCShellTreeCtrl`, control 2167): the
/// Desktop root, the computer, or a folder.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Node {
    Desktop,
    Computer,
    Dir(PathBuf),
}

impl Node {
    /// The shown name: a folder's own name, a root's path whole.
    pub fn label(&self) -> String {
        match self {
            Node::Desktop => "Рабочий стол".into(),
            Node::Computer => "Компьютер".into(),
            Node::Dir(p) => p.file_name().map_or_else(|| p.to_string_lossy().into_owned(), |n| n.to_string_lossy().into_owned()),
        }
    }
}

/// The folder pane (L9.116): MFC's shell tree as FastView leaves it — one
/// Desktop root (`0x10132fa6`), folders only, hidden ones left out
/// (SHCONTF_FOLDERS alone, `0x10132ec6`), children read when a node is
/// expanded, opened down to the scanned folder at start (`SelectPath`
/// `0x46b7de`). Its selection is not the scanned folder: only «Выбрать»
/// takes it (`0x46b8f0`).
pub struct FolderTree {
    pub selected: Node,
    /// The nodes `open_to` expanded, in order, until the tree is first
    /// drawn and scrolled as the TreeView scrolls for them.
    reveal: Vec<Node>,
    expanded: std::collections::HashSet<Node>,
    children: std::collections::HashMap<Node, Vec<Node>>,
    has_sub: std::collections::HashMap<Node, bool>,
    /// Where the children come from (the file system; a stand-in in tests).
    list: fn(&Node) -> Vec<Node>,
}

/// A node's children on this system: under the Desktop the computer, the
/// user's folder and the desktop's folders; under the computer its drives
/// (the file system root off Windows); under a folder its non-hidden
/// subfolders, by name ignoring case.
pub fn fs_children(n: &Node) -> Vec<Node> {
    match n {
        Node::Desktop => {
            let mut v = vec![Node::Computer];
            if let Some(home) = home_dir() {
                v.push(Node::Dir(home.clone()));
                v.extend(subfolders(&home.join("Desktop")));
            }
            v
        }
        Node::Computer => roots().into_iter().map(Node::Dir).collect(),
        Node::Dir(p) => subfolders(p),
    }
}

fn subfolders(p: &Path) -> Vec<Node> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(p).map(|rd| rd.flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_dir()) && !hidden(e)).map(|e| e.path()).collect()).unwrap_or_default();
    v.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
    v.into_iter().map(Node::Dir).collect()
}

#[cfg(windows)]
fn hidden(e: &std::fs::DirEntry) -> bool {
    use std::os::windows::fs::MetadataExt;
    e.metadata().is_ok_and(|m| m.file_attributes() & 0x2 != 0)
}

#[cfg(not(windows))]
fn hidden(e: &std::fs::DirEntry) -> bool {
    e.file_name().to_string_lossy().starts_with('.')
}

#[cfg(windows)]
fn roots() -> Vec<PathBuf> {
    (b'A'..=b'Z').map(|c| PathBuf::from(format!("{}:\\", c as char))).filter(|p| p.exists()).collect()
}

#[cfg(not(windows))]
fn roots() -> Vec<PathBuf> {
    vec![PathBuf::from("/")]
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}

/// The finder's default folder: My Documents (`0x46b0c0`, CSIDL_PERSONAL);
/// off Windows `~/Documents` when it exists, else the home folder.
pub fn documents_dir() -> Option<PathBuf> {
    let home = home_dir()?;
    let docs = home.join("Documents");
    Some(if docs.is_dir() { docs } else { home })
}

impl FolderTree {
    pub fn new(start: &Path, list: fn(&Node) -> Vec<Node>) -> FolderTree {
        let mut t = FolderTree { selected: Node::Desktop, reveal: Vec::new(), expanded: Default::default(), children: Default::default(), has_sub: Default::default(), list };
        t.open_to(start);
        t
    }

    /// Whether a node has a subfolder to show, asked once (the shell's
    /// SFGAO_HASSUBFOLDER); the virtual nodes always.
    fn has_sub(&mut self, n: &Node) -> bool {
        let Node::Dir(p) = n else { return true };
        *self.has_sub.entry(n.clone()).or_insert_with(|| {
            std::fs::read_dir(p).map(|rd| rd.flatten().any(|e| e.file_type().is_ok_and(|t| t.is_dir()) && !hidden(&e))).unwrap_or(false)
        })
    }

    /// The node's children, read once.
    fn kids(&mut self, n: &Node) -> &[Node] {
        let list = self.list;
        self.children.entry(n.clone()).or_insert_with(|| list(n))
    }

    /// Expand the tree down to `path` and select it (`SelectPath`); a path
    /// not found leaves the Desktop expanded and selected.
    pub fn open_to(&mut self, path: &Path) {
        self.expanded.insert(Node::Desktop);
        self.selected = Node::Desktop;
        let ancestors: Vec<PathBuf> = path.ancestors().map(Path::to_path_buf).collect();
        let Some(root) = ancestors.last().cloned() else { return };
        if !self.kids(&Node::Computer).contains(&Node::Dir(root.clone())) {
            return;
        }
        // All or nothing: a path the shell cannot parse selects nothing.
        let mut opened = vec![Node::Computer];
        let mut at = Node::Dir(root);
        for p in ancestors.iter().rev().skip(1) {
            let next = Node::Dir(p.clone());
            if !self.kids(&at).contains(&next) {
                return;
            }
            opened.push(at);
            at = next;
        }
        // `SelectPath` expands the folder it selects too (VM: Documents
        // opened on its subfolder).
        opened.push(at.clone());
        self.expanded.extend(opened.iter().cloned());
        self.reveal = opened;
        self.selected = at;
    }

    /// The first row to show once `open_to` has expanded its path, as the
    /// Win32 TreeView scrolls (VM): each expansion in turn brings its last
    /// child into view without taking the expanded folder above the top,
    /// then the selection is made visible. `None` after the first call.
    pub fn take_reveal(&mut self, visible: usize) -> Option<usize> {
        if self.reveal.is_empty() {
            return None;
        }
        let order = std::mem::take(&mut self.reveal);
        let rows = self.rows();
        Some(reveal_top(&rows, &order, &self.selected, visible.max(1)))
    }

    pub fn toggle(&mut self, n: &Node) {
        if !self.expanded.remove(n) {
            self.expanded.insert(n.clone());
        }
    }

    /// The rows shown: (node, depth, expanded, may have children).
    pub fn rows(&mut self) -> Vec<(Node, usize, bool, bool)> {
        let mut out = Vec::new();
        let mut stack = vec![(Node::Desktop, 0usize)];
        while let Some((n, d)) = stack.pop() {
            let open = self.expanded.contains(&n);
            // An unread folder shows its button by the shell's
            // has-subfolder mark (SFGAO_HASSUBFOLDER: VM, no button on a
            // folder without subfolders); a read one when it has some.
            let more = if open || self.children.contains_key(&n) { !self.kids(&n).is_empty() } else { self.has_sub(&n) };
            if open {
                for k in self.kids(&n).to_vec().into_iter().rev() {
                    stack.push((k, d + 1));
                }
            }
            out.push((n, d, open, more));
        }
        out
    }

    /// «Выбрать»'s folder: the selected node's path (none for the virtual
    /// Desktop / computer nodes — GetItemPath fails there).
    pub fn chosen(&self) -> Option<&Path> {
        match &self.selected {
            Node::Dir(p) => Some(p),
            _ => None,
        }
    }
}

/// [`FolderTree::take_reveal`]'s scrolling over `rows`.
fn reveal_top(rows: &[(Node, usize, bool, bool)], order: &[Node], selected: &Node, visible: usize) -> usize {
    let mut top = 0;
    for n in order {
        let Some(r) = rows.iter().position(|x| x.0 == *n) else { continue };
        let d = rows[r].1;
        let last = rows[r + 1..].iter().take_while(|x| x.1 > d).enumerate().filter(|(_, x)| x.1 == d + 1).map(|(k, _)| r + 1 + k).last().unwrap_or(r);
        if last >= top + visible {
            top = r.min(last + 1 - visible);
        }
    }
    if let Some(s) = rows.iter().position(|x| x.0 == *selected) {
        if s < top {
            top = s;
        } else if s >= top + visible {
            top = s + 1 - visible;
        }
    }
    top
}

/// A running scan: its cancel flag, result channel and worker thread.
type Scan = (Arc<AtomicBool>, mpsc::Receiver<Vec<Found>>, std::thread::JoinHandle<()>);

/// The dialog's state.
pub struct Finder {
    /// The scanned folder (Path, `dlg+0x98`): set only by «Выбрать».
    pub folder: PathBuf,
    pub tree: FolderTree,
    pub recursive: bool,
    pub station: String,
    pub block: String,
    pub from: Stamp,
    pub to: Stamp,
    pub files: Vec<Found>,
    pub selected: Vec<usize>,
    pub sort: (Column, bool),
    /// The list's order after a header click, for the filter it was made
    /// under: the refill (`0x46cb20`, after a scan or a filter change)
    /// lists the matches in scan order and nothing re-sorts them — only a
    /// header click sorts (`0x46d2e0`, LVM_SORTITEMS).
    sorted: Option<(Filter, Vec<usize>)>,
    /// Off Windows: the files awaiting our stand-in for the shell's delete
    /// prompt, and whether Shift was held (permanent).
    pub confirm_delete: Option<(Vec<PathBuf>, bool)>,
    scanning: Option<Scan>,
}

impl Finder {
    pub fn new(folder: PathBuf, recursive: bool, station: String, block: String) -> Finder {
        let mut f = Finder {
            tree: FolderTree::new(&folder, fs_children),
            folder,
            recursive,
            station,
            block,
            // Both pickers start at the members' zero time — 1 January 1970
            // (VM: with no file found they show «1 января 1970 г.»; a scan
            // that finds files sets the files' earliest / latest, `poll`).
            from: Stamp::EPOCH,
            to: Stamp::EPOCH,
            files: Vec::new(),
            selected: Vec::new(),
            // OnInitDialog's Sort(6) — Дата ascending — runs before the scan
            // fills the list, so it sorts nothing but sets the arrow.
            sort: (Column::Date, false),
            sorted: None,
            confirm_delete: None,
            scanning: None,
        };
        f.rescan();
        f
    }

    /// Start a scan, cancelling and joining a running one first.
    pub fn rescan(&mut self) {
        self.stop();
        let (cancel, (tx, rx)) = (Arc::new(AtomicBool::new(false)), mpsc::channel());
        let (root, rec, c) = (self.folder.clone(), self.recursive, cancel.clone());
        let handle = std::thread::spawn(move || {
            let _ = tx.send(scan(&root, rec, &c));
        });
        self.files.clear();
        self.selected.clear();
        self.sorted = None;
        self.scanning = Some((cancel, rx, handle));
    }

    pub fn stop(&mut self) {
        if let Some((cancel, _, handle)) = self.scanning.take() {
            cancel.store(true, Ordering::Relaxed);
            let _ = handle.join();
        }
    }

    pub fn busy(&self) -> bool {
        self.scanning.is_some()
    }

    /// Collect a finished scan: the date range becomes the files' earliest /
    /// latest times; a saved station / block survives when still listed
    /// (prefix match), else «Все».
    pub fn poll(&mut self) {
        let Some((_, rx, _)) = &self.scanning else { return };
        let Ok(files) = rx.try_recv() else { return };
        self.stop();
        self.files = files;
        if let (Some(lo), Some(hi)) = (self.files.iter().map(|f| f.time).min(), self.files.iter().map(|f| f.time).max()) {
            self.from = lo;
            self.to = hi;
        }
        let (stations, blocks) = (self.stations(), self.blocks());
        // The saved text re-picked by CB_FINDSTRING (`0x46bae0`): a
        // case-insensitive prefix, searched from after «Все» and wrapping to
        // it last — an empty text takes the first real entry; no match «Все».
        for (cur, list) in [(&mut self.station, stations), (&mut self.block, blocks)] {
            let want = cur.to_lowercase();
            *cur = list.iter().find(|v| v.to_lowercase().starts_with(&want)).cloned().unwrap_or_else(|| ALL.into());
        }
        self.sorted = None;
    }

    fn uniq(&self, f: impl Fn(&Found) -> &str) -> Vec<String> {
        let set: std::collections::BTreeSet<String> = self.files.iter().map(|x| f(x).to_string()).collect();
        set.into_iter().collect()
    }

    pub fn stations(&self) -> Vec<String> {
        self.uniq(|f| &f.station)
    }

    pub fn blocks(&self) -> Vec<String> {
        self.uniq(|f| &f.block)
    }

    fn filter(&self) -> Filter {
        (self.station.clone(), self.block.clone(), self.from, self.to)
    }

    /// Indices of the listed rows: the matches in scan order, or the order
    /// of the last header click while the filter is unchanged.
    pub fn rows(&self) -> Vec<usize> {
        if let Some((f, v)) = &self.sorted {
            if *f == self.filter() {
                return v.clone();
            }
        }
        (0..self.files.len()).filter(|&i| passes(&self.files[i], &self.station, &self.block, self.from, self.to)).collect()
    }

    /// A header click (`0x46d420` → `0x46d2e0`): the column ascending, again
    /// descending; the listed rows sorted by it — descending with the
    /// comparison reversed, so equal keys keep their order.
    pub fn click_sort(&mut self, col: Column) {
        self.sort = if self.sort.0 == col { (col, !self.sort.1) } else { (col, false) };
        let key = |i: usize| {
            let f = &self.files[i];
            match col {
                Column::Name => (f.name.clone(), Stamp::default()),
                Column::Station => (f.station.clone(), Stamp::default()),
                Column::Block => (f.block.clone(), Stamp::default()),
                Column::Date => (String::new(), f.time),
                // «Время» sorts by the time of day only.
                Column::Time => (String::new(), Stamp { h: f.time.h, mi: f.time.mi, s: f.time.s, ..Default::default() }),
            }
        };
        let mut v = self.rows();
        let desc = self.sort.1;
        v.sort_by(|&a, &b| if desc { key(b).cmp(&key(a)) } else { key(a).cmp(&key(b)) });
        self.sorted = Some((self.filter(), v));
    }

    /// `FileFinder/SortMode` (`0x46b3f5`): 2 × column + 1 when descending.
    pub fn sort_mode(&self) -> u32 {
        2 * self.sort.0 as u32 + u32::from(self.sort.1)
    }
}

/// The filter a sorted order was made under: station, block, С:, По:.
type Filter = (String, String, Stamp, Stamp);

impl Drop for Finder {
    fn drop(&mut self) {
        self.stop();
    }
}

impl App {
    /// Open the finder (32880), from the settings saved at its last close.
    pub fn open_finder(&mut self) {
        let v = &self.store.view;
        let folder = v.get("FileFinder/Path").map(PathBuf::from).or_else(documents_dir).unwrap_or_else(|| self.explorer_dir.clone());
        let recursive = v.get("FileFinder/IncludeSubdirs").is_some_and(|x| x == "1");
        let station = v.get("FileFinder/StationName").cloned().unwrap_or_default();
        let block = v.get("FileFinder/StationID").cloned().unwrap_or_default();
        self.finder = Some(Finder::new(folder, recursive, station, block));
    }

    fn close_finder(&mut self) {
        if let Some(mut f) = self.finder.take() {
            f.stop();
            let v = &mut self.store.view;
            v.insert("FileFinder/Path".into(), f.folder.to_string_lossy().into_owned());
            v.insert("FileFinder/IncludeSubdirs".into(), u8::from(f.recursive).to_string());
            v.insert("FileFinder/StationName".into(), f.station.clone());
            v.insert("FileFinder/StationID".into(), f.block.clone());
            v.insert("FileFinder/SortMode".into(), f.sort_mode().to_string());
            self.explorer_dir = f.folder.clone();
            if self.persist {
                self.store.save();
            }
        }
    }

    pub fn finder_window(&mut self, ctx: &egui::Context) {
        // «Открыть файлы» is the default button: Enter is IDOK (`0x46e120`),
        // Esc IDCANCEL (`0x46e2b0`).
        let keys = self.dialog_keys(ctx);
        let Some(f) = self.finder.as_mut() else { return };
        let keys = keys.filter(|_| f.confirm_delete.is_none());
        f.poll();
        if f.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        let mut open_files: Vec<PathBuf> = Vec::new();
        let mut close = keys.is_some();
        if keys == Some(true) {
            open_files = f.selected.iter().map(|&i| f.files[i].path.clone()).collect();
        }
        let mut rescan = false;
        let mut open = true;
        // The template's caption (DIALOG 558), not the menu item's; its
        // 739 × 354 DLU (1109 × 575 px), every control at its rectangle.
        crate::dialogs::dlg(ctx, "Поиск файлов")
            .margin(0.0)
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                use crate::dialogs::{dlu, win_button, win_check, win_combo_list, win_label, win_tree, TreeRow, DLG_FONT};
                use egui::{Color32, Pos2, Rect, Stroke, Vec2};
                let (client, _) = ui.allocate_exact_size(Vec2::new(1109.0, 575.0), egui::Sense::hover());
                let r = |x, y, w, h| dlu(client, x, y, w, h);
                win_label(ui, r(12, 36, 174, 8), "Выберите папку для поиска", false, true);
                // The shell tree (2167: lines, buttons, no lines at the
                // root), in its frame (VM): (130, 135, 144), white, (240),
                // then the sunken edge — (160) / (105) top and left, (255) /
                // (227) bottom and right.
                let tr = r(8, 57, 177, 243);
                let rows_t = f.tree.rows();
                let rows: Vec<TreeRow> = rows_t.iter().map(|(node, depth, open, more)| TreeRow { depth: *depth, label: node.label(), has_children: *more, expanded: *open, selected: f.tree.selected == *node }).collect();
                // 16 px rows inside the tree's 1 px frame.
                let top_row = f.tree.take_reveal(((tr.shrink(4.0).height() - 2.0) / 16.0).floor() as usize);
                let ev = win_tree(ui, tr.shrink(4.0), egui::Id::new("finder_tree"), Color32::TRANSPARENT, &rows, false, top_row, false, true, &mut |p, cell, k| {
                    let c = cell.center();
                    match rows_t[k].0 {
                        Node::Dir(_) => {
                            let fill = Color32::from_rgb(240, 200, 90);
                            p.rect_filled(Rect::from_min_size(c + Vec2::new(-7.0, -4.0), Vec2::new(6.0, 3.0)), 1.0f32, fill);
                            p.rect(Rect::from_min_size(c + Vec2::new(-7.0, -2.0), Vec2::new(14.0, 9.0)), 1.0f32, fill, Stroke::new(1.0f32, Color32::from_rgb(170, 130, 40)));
                        }
                        _ => {
                            let blue = Color32::from_rgb(70, 110, 180);
                            p.rect(Rect::from_min_size(c + Vec2::new(-7.0, -6.0), Vec2::new(14.0, 9.0)), 1.0f32, Color32::from_rgb(170, 200, 240), Stroke::new(1.0f32, blue));
                            p.hline((c.x - 4.0)..=(c.x + 4.0), c.y + 6.0, Stroke::new(1.5f32, blue));
                        }
                    }
                });
                {
                    let p = ui.painter();
                    let ring = |q: Rect, tl: Color32, br: Color32| {
                        p.rect_filled(Rect::from_min_max(q.min, Pos2::new(q.right(), q.top() + 1.0)), 0.0, tl);
                        p.rect_filled(Rect::from_min_max(q.min, Pos2::new(q.left() + 1.0, q.bottom())), 0.0, tl);
                        p.rect_filled(Rect::from_min_max(Pos2::new(q.left(), q.bottom() - 1.0), q.max), 0.0, br);
                        p.rect_filled(Rect::from_min_max(Pos2::new(q.right() - 1.0, q.top()), q.max), 0.0, br);
                    };
                    ring(tr, Color32::from_rgb(130, 135, 144), Color32::from_rgb(130, 135, 144));
                    ring(tr.shrink(1.0), Color32::WHITE, Color32::WHITE);
                    ring(tr.shrink(2.0), Color32::from_gray(240), Color32::from_gray(240));
                    ring(tr.shrink(3.0), Color32::from_gray(160), Color32::WHITE);
                    ring(tr.shrink(4.0), Color32::from_gray(105), Color32::from_gray(227));
                }
                if let Some(k) = ev.toggled.or(ev.double_clicked.filter(|&k| rows_t[k].3)) {
                    f.tree.toggle(&rows_t[k].0);
                }
                if let Some(k) = ev.clicked {
                    f.tree.selected = rows_t[k].0.clone();
                }
                // «Выбрать» (`0x46b8f0`): the tree's folder, rescanned
                // only when it differs (case-sensitively); a virtual node
                // does nothing.
                if win_button(ui, r(12, 306, 40, 13), "Выбрать", false).clicked() {
                    if let Some(p) = f.tree.chosen().filter(|p| p.as_os_str() != f.folder.as_os_str()).map(Path::to_path_buf) {
                        f.folder = p;
                        rescan = true;
                    }
                }
                if win_check(ui, r(72, 307, 111, 11), "Включая вложенные папки", f.recursive) {
                    f.recursive = !f.recursive;
                    rescan = true;
                }
                // «Папка:» and static 1084 with its static edge (VM: (160)
                // top and left, white bottom and right).
                win_label(ui, r(195, 12, 27, 11), "Папка:", false, true);
                let pr = r(233, 10, 330, 12);
                {
                    let p = ui.painter();
                    p.rect_filled(pr, 0.0, Color32::WHITE);
                    p.rect_filled(Rect::from_min_max(pr.min, pr.max - Vec2::splat(1.0)), 0.0, Color32::from_gray(160));
                    p.rect_filled(Rect::from_min_max(pr.min + Vec2::splat(1.0), pr.max - Vec2::splat(1.0)), 0.0, Color32::from_gray(240));
                    p.with_clip_rect(pr.shrink(1.0)).text(Pos2::new(pr.left() + 3.0, pr.center().y), egui::Align2::LEFT_CENTER, f.folder.display().to_string(), DLG_FONT, Color32::BLACK);
                }
                for (k, (lr, cr)) in [((194, 36, 36, 11), (233, 34, 118, 265)), ((354, 36, 23, 11), (380, 34, 116, 265))].into_iter().enumerate() {
                    let (label, value) = if k == 0 { ("Станция:", &mut f.station) } else { ("Блок:", &mut f.block) };
                    win_label(ui, r(lr.0, lr.1, lr.2, lr.3), label, false, true);
                    let mut items = vec![ALL.to_string()];
                    let list: std::collections::BTreeSet<String> = f.files.iter().map(|x| if k == 0 { x.station.clone() } else { x.block.clone() }).collect();
                    items.extend(list);
                    let at = items.iter().position(|it| it == value).unwrap_or(0);
                    if let Some(pick) = win_combo_list(ui, r(cr.0, cr.1, cr.2, cr.3), egui::Id::new(("finder_combo", k)), &items, at, true) {
                        *value = items[pick].clone();
                    }
                }
                // A date change refilters (`0x46cfd0`).
                win_label(ui, r(503, 36, 11, 9), "С:", false, true);
                date_picker(ui, r(517, 34, 94, 12), "С:", &mut f.from);
                win_label(ui, r(621, 36, 13, 9), "По:", false, true);
                date_picker(ui, r(636, 34, 94, 12), "По:", &mut f.to);
                // The list (1082, report, grid lines; VM): a (160) border, a
                // 24 px header with (229) separators 2 px inside each column
                // and the sort image after its column's text, columns 257,
                // 256, 85, 85, 85 px, 17 px rows ruled in (240) down the
                // whole list; the file's image at +4, its name at +18.
                let lr = r(198, 57, 532, 243);
                let inner = lr.shrink(1.0);
                let rows = f.rows();
                {
                    let p = ui.painter();
                    p.rect_filled(lr, 0.0, Color32::from_gray(160));
                    p.rect_filled(inner, 0.0, Color32::WHITE);
                }
                let cols = [(Column::Name, "Название файла", 257.0), (Column::Station, "Станция", 256.0), (Column::Block, "Блок", 85.0), (Column::Date, "Дата", 85.0), (Column::Time, "Время", 85.0)];
                let grid = Stroke::new(1.0f32, Color32::from_gray(240));
                let mut x = inner.left();
                let mut xs = Vec::new();
                for (col, label, w) in cols {
                    let h = Rect::from_min_size(Pos2::new(x, inner.top()), Vec2::new(w, 24.0));
                    xs.push((x, w));
                    let p = ui.painter();
                    p.vline(h.right() - 1.5, h.y_range(), Stroke::new(1.0f32, Color32::from_gray(229)));
                    p.vline(h.right() - 0.5, h.bottom()..=inner.bottom(), grid);
                    let t = p.with_clip_rect(h).text(Pos2::new(h.left() + 6.0, h.center().y), egui::Align2::LEFT_CENTER, label, DLG_FONT, Color32::BLACK);
                    if f.sort.0 == col {
                        // The sort image (the original's own: a pale box
                        // with a pink triangle) after the text.
                        let b = Rect::from_min_size(Pos2::new(t.right() + 13.0, h.center().y - 6.0), Vec2::splat(12.0));
                        p.rect_filled(b, 0.0, Color32::from_gray(214));
                        p.rect_filled(b.shrink(1.0), 0.0, Color32::WHITE);
                        let c = b.center();
                        // Ascending points down (VM: SortMode 6, Дата ascending).
                        let s = if f.sort.1 { -1.0 } else { 1.0 };
                        p.add(egui::Shape::convex_polygon(vec![c + Vec2::new(-3.0, -1.5 * s), c + Vec2::new(3.0, -1.5 * s), c + Vec2::new(0.0, 1.5 * s)], Color32::from_rgb(239, 36, 107), Stroke::NONE));
                    }
                    if ui.interact(h, ui.id().with(("finder_hdr", label)), egui::Sense::click()).clicked() {
                        f.click_sort(col);
                    }
                    x += w;
                }
                let mut y = inner.top() + 24.0 + 16.5;
                while y < inner.bottom() {
                    ui.painter().hline(inner.x_range(), y, grid);
                    y += 17.0;
                }
                if f.busy() {
                    win_label(ui, r(431, 180, 60, 10), "Поиск файлов...", false, true);
                }
                let (ctrl, shift) = ui.input(|i| (i.modifiers.command || i.modifiers.ctrl, i.modifiers.shift));
                let body = Rect::from_min_max(Pos2::new(inner.left(), inner.top() + 24.0), inner.max);
                ui.allocate_new_ui(egui::UiBuilder::new().max_rect(body), |ui| {
                    egui::ScrollArea::vertical().id_salt("finder_list").show(ui, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::ZERO;
                        for &i in &rows {
                            let (row, resp) = ui.allocate_exact_size(Vec2::new(body.width(), 17.0), egui::Sense::click());
                            let x = &f.files[i];
                            let sel = f.selected.contains(&i);
                            let p = ui.painter();
                            if sel {
                                p.rect_filled(Rect::from_min_size(row.min, Vec2::new(xs[4].0 + xs[4].1 - inner.left() - 1.0, 16.0)), 0.0, Color32::from_rgb(0, 120, 215));
                            }
                            let ink = if sel { Color32::WHITE } else { Color32::BLACK };
                            // The file's image (the original's own: ours a
                            // page, green for an oscillogram description).
                            let ic = Rect::from_min_size(row.min + Vec2::new(4.0, 0.5), Vec2::new(12.0, 15.0));
                            p.rect(ic, 0.0, Color32::from_rgb(244, 252, 245), Stroke::new(1.0f32, if x.osc { Color32::from_rgb(53, 104, 48) } else { Color32::from_gray(120) }));
                            let texts = [x.name.clone(), x.station.clone(), x.block.clone(), x.time.date(), x.time.time()];
                            for (k, t) in texts.into_iter().enumerate() {
                                let (cx, cw) = xs[k];
                                let cell = Rect::from_min_size(Pos2::new(cx, row.top()), Vec2::new(cw - 1.0, 16.0));
                                let at = Pos2::new(cx + if k == 0 { 18.0 } else { 6.0 }, row.top() + 8.0);
                                p.with_clip_rect(cell).text(at, egui::Align2::LEFT_CENTER, t, DLG_FONT, ink);
                            }
                            // NM_DBLCLK (`0x46e330`): with a selection, OnOK —
                            // every selected file opens (the first click of
                            // the double-click has set the selection).
                            if resp.double_clicked() {
                                if !f.selected.contains(&i) {
                                    f.selected = vec![i];
                                }
                                open_files = f.selected.iter().map(|&j| f.files[j].path.clone()).collect();
                            } else if resp.clicked() {
                                if ctrl {
                                    if sel {
                                        f.selected.retain(|&j| j != i);
                                    } else {
                                        f.selected.push(i);
                                    }
                                } else if shift && !f.selected.is_empty() {
                                    let last = *f.selected.last().unwrap();
                                    let (a, b) = (rows.iter().position(|&j| j == last).unwrap_or(0), rows.iter().position(|&j| j == i).unwrap_or(0));
                                    f.selected = rows[a.min(b)..=a.max(b)].to_vec();
                                } else {
                                    f.selected = vec![i];
                                }
                            }
                        }
                    });
                });
                // Neither button is ever greyed (the finder only enables
                // and disables its list).
                if win_button(ui, r(198, 306, 83, 14), "Удалить файлы", false).clicked() && !f.selected.is_empty() {
                    // `0x46d4b0`: every selected file and its companions in
                    // one shell delete — to the recycle bin unless Shift is
                    // held; the shell asks, not the program.
                    let paths: Vec<PathBuf> = f.selected.iter().flat_map(|&i| companions(&f.files[i].path)).collect();
                    let permanent = ui.input(|i| i.modifiers.shift);
                    if cfg!(windows) {
                        rescan = shell_delete(&paths, permanent);
                    } else {
                        f.confirm_delete = Some((paths, permanent));
                    }
                }
                // OnOK: stop the scan, open every selected file, close —
                // with nothing selected it only closes.
                if win_button(ui, r(552, 306, 83, 14), "Открыть файлы", true).clicked() {
                    open_files = f.selected.iter().map(|&i| f.files[i].path.clone()).collect();
                    close = true;
                }
                if win_button(ui, r(648, 306, 83, 14), "Закрыть", false).clicked() {
                    close = true;
                }
            });
        // Off Windows there is no shell prompt: ours stands in, a message
        // box over the finder.
        if let Some(f) = self.finder.as_mut() {
            if let Some((paths, permanent)) = f.confirm_delete.clone() {
                let text = if permanent { format!("Удалить безвозвратно файлы ({})?", paths.len()) } else { format!("Переместить файлы в корзину ({})?", paths.len()) };
                crate::dialogs::dlg(ctx, crate::app::APP_TITLE).id(egui::Id::new("finder_confirm")).margin(0.0).collapsible(false).resizable(false).order(egui::Order::Foreground).show(ctx, |ui| {
                    match crate::dialogs::message_box(ui, crate::dialogs::MsgIcon::Question, &text, &["Да", "Нет"]) {
                        Some(0) => {
                            rescan = shell_delete(&paths, permanent);
                            f.confirm_delete = None;
                        }
                        Some(_) => f.confirm_delete = None,
                        None => {}
                    }
                });
            }
        }
        if rescan {
            if let Some(f) = self.finder.as_mut() {
                f.rescan();
            }
        }
        if !open || close || !open_files.is_empty() {
            self.close_finder();
        }
        for p in open_files {
            self.load_comtrade_path(&p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CFG: &str = "ПС Заря,Рег-1,1999\n2,1A,1D\n1,Ia,a,,A,1,0,0,-1,1,1,1,S\n1,D1,,,0\n50\n1\n1000,10\n17/08/2008,13:59:10.322588\n17/08/2008,13:59:10.422588\nASCII\n1\n";

    #[test]
    fn long_dates_and_weekdays() {
        let st = Stamp { y: 2026, mo: 10, d: 23, h: 14, mi: 5, s: 0 };
        assert_eq!(st.long_date(), "23 октября 2026 г.");
        assert_eq!(weekday(2026, 10, 23), 4, "a Friday");
        assert_eq!(weekday(2024, 2, 29), 3, "a Thursday");
        assert_eq!((month_days(2024, 2), month_days(1900, 2), month_days(2000, 2)), (29, 28, 29));
    }

    /// The folder pane: opened down to the start folder, children read on
    /// expanding, the virtual nodes not choosable.
    #[test]
    fn folder_tree_opens_to_the_start_folder() {
        fn list(n: &Node) -> Vec<Node> {
            let d = |s: &str| Node::Dir(PathBuf::from(s));
            match n {
                Node::Desktop => vec![Node::Computer, d("/home/u")],
                Node::Computer => vec![d("/")],
                Node::Dir(p) if p == Path::new("/") => vec![d("/data"), d("/home")],
                Node::Dir(p) if p == Path::new("/data") => vec![d("/data/rec")],
                _ => vec![],
            }
        }
        let mut t = FolderTree::new(Path::new("/data/rec"), list);
        assert_eq!(t.selected, Node::Dir(PathBuf::from("/data/rec")));
        let rows: Vec<(String, usize)> = t.rows().into_iter().map(|(n, d, ..)| (n.label(), d)).collect();
        assert_eq!(rows.iter().map(|r| r.0.as_str()).collect::<Vec<_>>(), ["Рабочий стол", "Компьютер", "/", "data", "rec", "home", "u"]);
        assert_eq!(rows[4].1, 4, "rec under data under / under the computer");
        t.selected = Node::Computer;
        assert_eq!(t.chosen(), None, "GetItemPath fails on a virtual node");
        // A folder not reachable: the Desktop.
        let t = FolderTree::new(Path::new("/nowhere/x"), list);
        assert_eq!(t.selected, Node::Desktop);
    }

    /// The VM's tree: opened to C:\Users\devuser\Documents (the 18th of
    /// devuser's 30 folders) with 24 rows shown, devuser's expansion scrolls
    /// it to the top row; Documents is expanded on its subfolder and in view.
    #[test]
    fn the_tree_scrolls_as_the_treeview_opening_its_path() {
        fn list(n: &Node) -> Vec<Node> {
            let d = |s: &str| Node::Dir(PathBuf::from(s));
            match n {
                Node::Desktop => vec![Node::Computer],
                Node::Computer => vec![d("/")],
                Node::Dir(p) if p == Path::new("/") => ["inetpub", "PerfLogs", "Program Files", "Program Files (x86)", "Users"].iter().map(|x| d(&format!("/{x}"))).collect(),
                Node::Dir(p) if p == Path::new("/Users") => ["a", "b", "c", "devuser"].iter().map(|x| d(&format!("/Users/{x}"))).collect(),
                Node::Dir(p) if p == Path::new("/Users/devuser") => (0..30).map(|k| d(&format!("/Users/devuser/{}", if k == 17 { "Documents".to_string() } else { format!("f{k:02}") }))).collect(),
                Node::Dir(p) if p == Path::new("/Users/devuser/Documents") => vec![d("/Users/devuser/Documents/WindowsPowerShell")],
                _ => vec![],
            }
        }
        let mut t = FolderTree::new(Path::new("/Users/devuser/Documents"), list);
        let top = t.take_reveal(24).expect("scrolled once");
        let rows = t.rows();
        assert_eq!(rows[top].0.label(), "devuser");
        let docs = rows.iter().position(|r| r.0.label() == "Documents").unwrap();
        assert!(rows[docs].2 && rows[docs + 1].0.label() == "WindowsPowerShell", "the selected folder expanded");
        assert!(docs < top + 24);
        assert_eq!(t.take_reveal(24), None, "only on opening");
    }

    #[test]
    fn comtrade_header_dates_month_first_for_cfg_day_first_for_cff() {
        // `.cfg`: 17/08 → month 17 > 12 → swapped to 17 August.
        let (st, bl, t) = read_comtrade_header(CFG, false).unwrap();
        assert_eq!((st.as_str(), bl.as_str()), ("ПС Заря", "Рег-1"));
        assert_eq!((t.y, t.mo, t.d, t.h, t.mi, t.s), (2008, 8, 17, 13, 59, 10));
        let cfg = CFG.replace("17/08/2008,13", "03/04/08,13");
        assert_eq!(read_comtrade_header(&cfg, false).unwrap().2.mo, 3, ".cfg reads month first");
        assert_eq!(read_comtrade_header(&cfg, true).unwrap().2.mo, 4, ".cff reads day first");
        assert_eq!(read_comtrade_header(&cfg, true).unwrap().2.y, 2008);
        let no_block = CFG.replace("ПС Заря,Рег-1,1999", "ПС Заря");
        assert_eq!(read_comtrade_header(&no_block, false).unwrap().1, "-");
    }

    #[test]
    fn osc_header_and_filters() {
        // H = 1: the info block at 128, its time in BCD (0x30 s, 0x15 min,
        // 0x09 h, 0x01 day, 0x02 month, 0x21 year).
        let mut b = vec![2, 6, 1];
        b.resize(128, 0);
        b.extend([0, 0, 0, 0, 0, 0x45, 0x30, 0x15, 0x09, 0x01, 0x02, 0x21]);
        b.resize(256, 0);
        let (st, bl, t) = read_osc_header(&b).unwrap();
        assert_eq!(st, "OSC Осциллограф. Версия 2.6");
        assert_eq!(bl, "-");
        assert_eq!(t, Stamp { y: 2021, mo: 2, d: 1, h: 9, mi: 15, s: 30 });
        let f = Found { path: "x".into(), name: "x".into(), osc: true, station: st.clone(), block: bl, time: t };
        let day = |d| Stamp { y: 2021, mo: 2, d, ..Default::default() };
        assert!(passes(&f, ALL, ALL, day(1), day(1)), "the whole По day is included");
        assert!(!passes(&f, ALL, ALL, day(2), day(3)));
        assert!(!passes(&f, "другая", ALL, day(1), day(1)));
        assert_eq!(day(28).next_day(), Stamp { y: 2021, mo: 3, d: 1, ..Default::default() });
    }

    /// The list keeps the scan order until a header click sorts it; a
    /// filter change (a refill) brings the scan order back (`0x46cb20`,
    /// `0x46d2e0`); `SortMode` is 2 × column + descending.
    #[test]
    fn sorted_only_by_a_header_click() {
        let dir = std::env::temp_dir().join(format!("fv_finder_sort_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut f = Finder::new(dir.clone(), false, String::new(), String::new());
        while f.busy() {
            f.poll();
        }
        let stamp = |d: u32| Stamp { y: 2024, mo: 1, d, ..Default::default() };
        let found = |name: &str, st: &str, d: u32| Found { path: name.into(), name: name.into(), osc: false, station: st.into(), block: String::new(), time: stamp(d) };
        f.files = vec![found("b", "S1", 3), found("a", "S2", 1), found("c", "S1", 2)];
        (f.station, f.block, f.from, f.to) = (ALL.into(), ALL.into(), stamp(1), stamp(3));
        assert_eq!(f.rows(), [0, 1, 2], "scan order, though the arrow says Дата ▲");
        f.click_sort(Column::Name);
        assert_eq!(f.rows(), [1, 0, 2]);
        f.click_sort(Column::Name);
        assert_eq!((f.rows(), f.sort_mode()), (vec![2, 0, 1], 1));
        f.station = "S1".into();
        assert_eq!(f.rows(), [0, 2], "a refill lists in scan order");
        f.click_sort(Column::Date);
        assert_eq!((f.rows(), f.sort_mode()), (vec![2, 0], 6));
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn scan_finds_known_kinds_and_companions() {
        let dir = std::env::temp_dir().join(format!("fv_finder_{}", std::process::id()));
        let sub = dir.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(dir.join("a.cfg"), CFG).unwrap();
        std::fs::write(dir.join("a.dat"), "1,0,0,0\n").unwrap();
        std::fs::write(dir.join("a.hdr"), "").unwrap();
        std::fs::write(sub.join("b.CFG"), CFG).unwrap();
        std::fs::write(dir.join("c.txt"), "x").unwrap();
        let none = AtomicBool::new(false);
        assert_eq!(scan(&dir, false, &none).len(), 1);
        let all = scan(&dir, true, &none);
        assert_eq!(all.len(), 2, "subfolders only when asked; .txt ignored");
        let c = companions(&dir.join("a.cfg"));
        assert_eq!(c.len(), 3, "{c:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Shift+delete removes the file and its companions for good (the
    /// recycle-bin path goes through the desktop's trash, not tested here).
    #[cfg(not(windows))]
    #[test]
    fn shift_delete_removes_the_companions() {
        let dir = std::env::temp_dir().join(format!("fv_finder_del_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.cfg"), CFG).unwrap();
        std::fs::write(dir.join("a.dat"), "1,0,0,0\n").unwrap();
        std::fs::write(dir.join("b.cfg"), CFG).unwrap();
        assert!(shell_delete(&companions(&dir.join("a.cfg")), true));
        let left: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(left, ["b.cfg"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
