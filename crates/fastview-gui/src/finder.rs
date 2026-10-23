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

    /// The current date (UTC; the pickers' initial value — the local
    /// offset is not taken).
    pub fn today() -> Stamp {
        let days = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() / 86_400) as i64;
        // Days since 1970-01-01 to a civil date (Hinnant's algorithm).
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let mo = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let y = (yoe + era * 400 + i64::from(mo <= 2)) as i32;
        Stamp { y, mo, d, h: 0, mi: 0, s: 0 }
    }

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
fn date_picker(ui: &mut egui::Ui, id: &str, st: &mut Stamp) -> bool {
    let popup = egui::Id::new(("fv_finder_date", id));
    let browse_id = popup.with("month");
    let r = ui.button(format!("{} ▾", st.long_date()));
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
    let mut out = Vec::new();
    let mut dirs = vec![root.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        let mut entries: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        entries.sort();
        for p in entries {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            if p.is_dir() {
                if recursive {
                    dirs.push(p);
                }
            } else if let Some(f) = read_entry(&p, root) {
                out.push(f);
            }
        }
    }
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
    expanded: std::collections::HashSet<Node>,
    children: std::collections::HashMap<Node, Vec<Node>>,
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
        let mut t = FolderTree { selected: Node::Desktop, expanded: Default::default(), children: Default::default(), list };
        t.open_to(start);
        t
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
        self.expanded.extend(opened);
        self.selected = at;
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
            // An unread folder may have children (the shell's has-children
            // mark); a read one shows its button only when it has some.
            let more = if open || self.children.contains_key(&n) { !self.kids(&n).is_empty() } else { true };
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
            // A date-time picker starts at the current date.
            from: Stamp::today(),
            to: Stamp::today(),
            files: Vec::new(),
            selected: Vec::new(),
            sort: (Column::Date, false),
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
        for (cur, list) in [(&mut self.station, stations), (&mut self.block, blocks)] {
            *cur = list.iter().find(|v| v.starts_with(cur.as_str()) && !cur.is_empty()).cloned().unwrap_or_else(|| ALL.into());
        }
        self.sort = (Column::Date, false);
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

    /// Indices of the listed rows, filtered and sorted.
    pub fn rows(&self) -> Vec<usize> {
        let mut v: Vec<usize> = (0..self.files.len()).filter(|&i| passes(&self.files[i], &self.station, &self.block, self.from, self.to)).collect();
        let key = |i: &usize| {
            let f = &self.files[*i];
            match self.sort.0 {
                Column::Name => (f.name.clone(), Stamp::default()),
                Column::Station => (f.station.clone(), Stamp::default()),
                Column::Block => (f.block.clone(), Stamp::default()),
                Column::Date => (String::new(), f.time),
                // «Время» sorts by the time of day only.
                Column::Time => (String::new(), Stamp { h: f.time.h, mi: f.time.mi, s: f.time.s, ..Default::default() }),
            }
        };
        v.sort_by_key(key);
        if self.sort.1 {
            v.reverse();
        }
        v
    }
}

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
            self.explorer_dir = f.folder.clone();
            if self.persist {
                self.store.save();
            }
        }
    }

    pub fn finder_window(&mut self, ctx: &egui::Context) {
        let Some(f) = self.finder.as_mut() else { return };
        f.poll();
        if f.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        let mut open_files: Vec<PathBuf> = Vec::new();
        let mut close = false;
        let mut rescan = false;
        let mut open = true;
        egui::Window::new("Проводник")
            .collapsible(false)
            .default_size([900.0, 520.0])
            .open(&mut open)
            .show(ctx, |ui| {
                egui::SidePanel::left("finder_tree").default_width(220.0).show_inside(ui, |ui| {
                    // The shell tree (lines, buttons, no lines at the root).
                    egui::ScrollArea::both().max_height(360.0).auto_shrink([false, false]).show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        let mut toggle = None;
                        for (node, depth, open, more) in f.tree.rows() {
                            ui.horizontal(|ui| {
                                ui.add_space(depth as f32 * 16.0);
                                let (r, resp) = ui.allocate_exact_size(egui::vec2(14.0, 18.0), egui::Sense::click());
                                if more {
                                    ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, if open { "▾" } else { "▸" }, egui::FontId::proportional(12.0), ui.visuals().text_color());
                                    if resp.clicked() {
                                        toggle = Some(node.clone());
                                    }
                                }
                                // A small drawn icon (the shell's images are its own).
                                let (ir, _) = ui.allocate_exact_size(egui::vec2(16.0, 18.0), egui::Sense::hover());
                                let c = ir.center();
                                let pt = ui.painter();
                                match node {
                                    Node::Dir(_) => {
                                        let fill = egui::Color32::from_rgb(240, 200, 90);
                                        pt.rect_filled(egui::Rect::from_min_size(c + egui::vec2(-7.0, -4.0), egui::vec2(6.0, 3.0)), 1.0f32, fill);
                                        pt.rect(egui::Rect::from_min_size(c + egui::vec2(-7.0, -2.0), egui::vec2(14.0, 9.0)), 1.0f32, fill, egui::Stroke::new(1.0f32, egui::Color32::from_rgb(170, 130, 40)));
                                    }
                                    _ => {
                                        let blue = egui::Color32::from_rgb(70, 110, 180);
                                        pt.rect(egui::Rect::from_min_size(c + egui::vec2(-7.0, -6.0), egui::vec2(14.0, 9.0)), 1.0f32, egui::Color32::from_rgb(170, 200, 240), egui::Stroke::new(1.0f32, blue));
                                        pt.hline((c.x - 4.0)..=(c.x + 4.0), c.y + 6.0, egui::Stroke::new(1.5f32, blue));
                                    }
                                }
                                let label = ui.add(egui::Label::new(node.label()).selectable(false).sense(egui::Sense::click()));
                                if f.tree.selected == node {
                                    ui.painter().rect_stroke(label.rect.expand(1.0), 1.0f32, ui.visuals().selection.stroke);
                                }
                                if label.clicked() {
                                    f.tree.selected = node.clone();
                                }
                                if label.double_clicked() && more {
                                    toggle = Some(node.clone());
                                }
                            });
                        }
                        if let Some(n) = toggle {
                            f.tree.toggle(&n);
                        }
                    });
                    // «Выбрать» (`0x46b8f0`): the tree's folder, rescanned
                    // only when it differs (case-sensitively); a virtual node
                    // does nothing.
                    if ui.button("Выбрать").clicked() {
                        if let Some(p) = f.tree.chosen().filter(|p| p.as_os_str() != f.folder.as_os_str()).map(Path::to_path_buf) {
                            f.folder = p;
                            rescan = true;
                        }
                    }
                    if ui.checkbox(&mut f.recursive, "Включая вложенные папки").changed() {
                        rescan = true;
                    }
                });
                // «Папка:» and a framed read-only field (static 1084).
                ui.horizontal(|ui| {
                    ui.label("Папка:");
                    egui::Frame::none().stroke(ui.visuals().widgets.noninteractive.bg_stroke).inner_margin(egui::Margin::symmetric(4.0, 1.0)).show(ui, |ui| {
                        ui.add(egui::Label::new(f.folder.display().to_string()).truncate());
                    });
                });
                ui.horizontal(|ui| {
                    for (label, cur, list) in [("Станция", &mut f.station, f.files.iter().map(|x| x.station.clone()).collect::<std::collections::BTreeSet<_>>()), ("Блок", &mut f.block, f.files.iter().map(|x| x.block.clone()).collect())] {
                        ui.label(label);
                        egui::ComboBox::from_id_salt(label).selected_text(cur.clone()).show_ui(ui, |ui| {
                            ui.selectable_value(cur, ALL.to_string(), ALL);
                            for v in list {
                                ui.selectable_value(cur, v.clone(), v);
                            }
                        });
                    }
                    // A date change refilters (`0x46cfd0`).
                    for (label, st) in [("С:", &mut f.from), ("По:", &mut f.to)] {
                        ui.label(label);
                        date_picker(ui, label, st);
                    }
                });
                if f.busy() {
                    ui.label("Поиск файлов...");
                }
                let rows = f.rows();
                egui::ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
                    egui::Grid::new("finder_list").striped(true).num_columns(5).show(ui, |ui| {
                        for (col, label) in [(Column::Name, "Название файла"), (Column::Station, "Станция"), (Column::Block, "Блок"), (Column::Date, "Дата"), (Column::Time, "Время")] {
                            let arrow = if f.sort.0 == col { if f.sort.1 { " ▼" } else { " ▲" } } else { "" };
                            if ui.button(format!("{label}{arrow}")).clicked() {
                                f.sort = if f.sort.0 == col { (col, !f.sort.1) } else { (col, false) };
                            }
                        }
                        ui.end_row();
                        let (ctrl, shift) = ui.input(|i| (i.modifiers.command || i.modifiers.ctrl, i.modifiers.shift));
                        for &i in &rows {
                            let x = &f.files[i];
                            let sel = f.selected.contains(&i);
                            let r = ui.selectable_label(sel, format!("{} {}", if x.osc { "〰" } else { "📄" }, x.name));
                            // NM_DBLCLK (`0x46e330`): with a selection, OnOK —
                            // every selected file opens (the first click of
                            // the double-click has set the selection).
                            if r.double_clicked() {
                                if !f.selected.contains(&i) {
                                    f.selected = vec![i];
                                }
                                open_files = f.selected.iter().map(|&j| f.files[j].path.clone()).collect();
                            } else if r.clicked() {
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
                            ui.label(&x.station);
                            ui.label(&x.block);
                            ui.label(x.time.date());
                            ui.label(x.time.time());
                            ui.end_row();
                        }
                    });
                });
                ui.horizontal(|ui| {
                    if ui.add_enabled(!f.selected.is_empty(), egui::Button::new("Удалить файлы")).clicked() {
                        // `0x46d4b0`: every selected file and its companions
                        // in one shell delete — to the recycle bin unless
                        // Shift is held; the shell asks, not the program.
                        let paths: Vec<PathBuf> = f.selected.iter().flat_map(|&i| companions(&f.files[i].path)).collect();
                        let permanent = ui.input(|i| i.modifiers.shift);
                        if cfg!(windows) {
                            rescan = shell_delete(&paths, permanent);
                        } else {
                            f.confirm_delete = Some((paths, permanent));
                        }
                    }
                    if ui.add_enabled(!f.selected.is_empty(), egui::Button::new("Открыть файлы")).clicked() {
                        open_files = f.selected.iter().map(|&i| f.files[i].path.clone()).collect();
                    }
                    if ui.button("Закрыть").clicked() {
                        close = true;
                    }
                });
                if let Some((paths, permanent)) = f.confirm_delete.clone() {
                    // Off Windows there is no shell prompt: ours stands in.
                    ui.separator();
                    ui.label(if permanent {
                        format!("Удалить безвозвратно файлы ({})?", paths.len())
                    } else {
                        format!("Переместить файлы в корзину ({})?", paths.len())
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Да").clicked() {
                            rescan = shell_delete(&paths, permanent);
                            f.confirm_delete = None;
                        }
                        if ui.button("Нет").clicked() {
                            f.confirm_delete = None;
                        }
                    });
                }
            });
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
