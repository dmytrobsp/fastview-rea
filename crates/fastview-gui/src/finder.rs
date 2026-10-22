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

    /// The start of the following day (the inclusive upper bound of По).
    fn next_day(self) -> Stamp {
        let days = match self.mo {
            2 if (self.y % 4 == 0 && self.y % 100 != 0) || self.y % 400 == 0 => 29,
            2 => 28,
            4 | 6 | 9 | 11 => 30,
            _ => 31,
        };
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
pub fn read_osc_header(b: &[u8]) -> Option<(String, String, Stamp)> {
    if b.len() < 12 {
        return None;
    }
    let t = Stamp { y: 2000 + i32::from(b[11]), mo: u32::from(b[10]), d: u32::from(b[9]), h: u32::from(b[8]), mi: u32::from(b[7]), s: u32::from(b[6]) };
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

/// A running scan: its cancel flag, result channel and worker thread.
type Scan = (Arc<AtomicBool>, mpsc::Receiver<Vec<Found>>, std::thread::JoinHandle<()>);

/// The dialog's state.
pub struct Finder {
    pub folder: PathBuf,
    pub recursive: bool,
    pub station: String,
    pub block: String,
    pub from: Stamp,
    pub to: Stamp,
    pub files: Vec<Found>,
    pub selected: Vec<usize>,
    pub sort: (Column, bool),
    pub confirm_delete: bool,
    scanning: Option<Scan>,
}

impl Finder {
    pub fn new(folder: PathBuf, recursive: bool, station: String, block: String) -> Finder {
        let mut f = Finder {
            folder,
            recursive,
            station,
            block,
            from: Stamp::default(),
            to: Stamp::default(),
            files: Vec::new(),
            selected: Vec::new(),
            sort: (Column::Date, false),
            confirm_delete: false,
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
        let folder = v.get("FileFinder/Path").map(PathBuf::from).unwrap_or_else(|| self.explorer_dir.clone());
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
                    // The folder tree: the folders under the current one.
                    if let Some(parent) = f.folder.parent() {
                        if ui.selectable_label(false, "📁 ..").clicked() {
                            f.folder = parent.to_path_buf();
                        }
                    }
                    egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                        let mut dirs: Vec<PathBuf> = std::fs::read_dir(&f.folder).map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect()).unwrap_or_default();
                        dirs.sort();
                        for d in dirs {
                            let name = d.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                            if ui.selectable_label(false, format!("📁 {name}")).clicked() {
                                f.folder = d;
                            }
                        }
                    });
                    // «Выбрать» rescans with the tree's folder; a tree click alone does not.
                    if ui.button("Выбрать").clicked() {
                        rescan = true;
                    }
                    if ui.checkbox(&mut f.recursive, "Включая вложенные папки").changed() {
                        rescan = true;
                    }
                });
                ui.label(format!("Папка: {}", f.folder.display()));
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
                    for (label, st) in [("С:", &mut f.from), ("По:", &mut f.to)] {
                        ui.label(label);
                        ui.add(egui::DragValue::new(&mut st.d).range(1..=31));
                        ui.add(egui::DragValue::new(&mut st.mo).range(1..=12));
                        ui.add(egui::DragValue::new(&mut st.y).range(1990..=2100));
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
                            if r.double_clicked() {
                                open_files = vec![x.path.clone()];
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
                        f.confirm_delete = true;
                    }
                    if ui.add_enabled(!f.selected.is_empty(), egui::Button::new("Открыть файлы")).clicked() {
                        open_files = f.selected.iter().map(|&i| f.files[i].path.clone()).collect();
                    }
                    if ui.button("Закрыть").clicked() {
                        close = true;
                    }
                });
                if f.confirm_delete {
                    let paths: Vec<PathBuf> = f.selected.iter().flat_map(|&i| companions(&f.files[i].path)).collect();
                    ui.separator();
                    ui.label(format!("Удалить без возможности восстановления файлов: {}?", paths.len()));
                    ui.horizontal(|ui| {
                        if ui.button("Да").clicked() {
                            for p in &paths {
                                let _ = std::fs::remove_file(p);
                            }
                            f.confirm_delete = false;
                            rescan = true;
                        }
                        if ui.button("Нет").clicked() {
                            f.confirm_delete = false;
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
        let mut b = vec![2, 6, 0, 0, 0, 0, 30, 15, 9, 1, 2, 21];
        b.resize(64, 0);
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
}
