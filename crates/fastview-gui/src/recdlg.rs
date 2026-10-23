//! Record-level dialogs and the parameters bar: «Параметры осциллограммы»
//! (32861, DIALOG 392, `FileInfoDlg`), «Установка настроек программы» (129,
//! DIALOG 510, `CProgSettingdDlg`), the record comment editor (32805, DIALOG
//! 332 in record mode, `CCommentDlg`) and the parameters bar (142,
//! `CParamOscBar`, id 810). Also the per-channel primary/secondary switch
//! (32867 / 32868) and the full recompute those commands trigger.

use crate::app::{App, Doc};
use eframe::egui::{self, Rect};
use fvb_core::comtrade::{self, Encoding};

/// State of the dialogs and bars of this module and `chandlg`.
#[derive(Debug, Clone)]
pub struct DialogState {
    /// The parameters bar is shown (visible at start, `0x474dbe`).
    pub param_bar: bool,
    /// `Settings Parameters/Dst folder` (default "").
    pub dst_folder: String,
    /// `Settings Parameters/UTC for Comtrade2013` (default on).
    pub utc_2013: bool,
    pub file_params: Option<FileParamsDlg>,
    pub settings: Option<SettingsDlg>,
    pub hdr_edit: Option<HdrEdit>,
    pub channels: Option<crate::chandlg::ChannelsDlg>,
    pub coef: Option<crate::chandlg::CoefDlg>,
    /// A message box (the original's AfxMessageBox) waiting for OK.
    pub alert: Option<String>,
}

impl Default for DialogState {
    fn default() -> Self {
        DialogState {
            param_bar: true,
            dst_folder: String::new(),
            utc_2013: true,
            file_params: None,
            settings: None,
            hdr_edit: None,
            channels: None,
            coef: None,
            alert: None,
        }
    }
}

/// DIALOG 392 edits: the sample-rate combo text and the correction choice.
#[derive(Debug, Clone)]
pub struct FileParamsDlg {
    pub rate_text: String,
    pub correction: usize,
}

/// DIALOG 510 edits (applied on OK).
#[derive(Debug, Clone)]
pub struct SettingsDlg {
    pub dst_folder: String,
    pub utc: bool,
    pub encoding: Encoding,
}

/// DIALOG 332 in record mode: the `.hdr` text and its code page.
#[derive(Debug, Clone)]
pub struct HdrEdit {
    pub text: String,
    pub cp: Encoding,
}

/// `Display Encoding` codes of DIALOG 510 (radios 4005–4008, `0x422300`):
/// 0 UTF-8 (65001), 1 Windows-1251, 2 IBM866, 3 KOI8-R (20866). No auto.
pub const ENCODINGS: [(Encoding, &str); 4] =
    [(Encoding::Utf8, "UTF-8"), (Encoding::Cp1251, "Windows-1251"), (Encoding::Cp866, "IBM866"), (Encoding::Koi8r, "KOI8-R")];

pub fn encoding_code(e: Encoding) -> u8 {
    ENCODINGS.iter().position(|(x, _)| *x == e).unwrap_or(1) as u8
}

pub fn encoding_of(code: u8) -> Encoding {
    ENCODINGS.get(code as usize).map_or(Encoding::Cp1251, |(e, _)| *e)
}

/// The fixed sample-rate lists of the file parameters combo (`0x4499e0`).
pub fn rate_list(line_freq: f64) -> &'static [u32] {
    const F50: [u32; 27] = [
        160000, 80000, 40000, 32000, 20000, 19200, 16000, 10000, 9600, 8000, 6400, 5000, 4800, 4000, 3200, 2500, 2400,
        2000, 1600, 1200, 1000, 800, 600, 500, 400, 300, 200,
    ];
    const F60: [u32; 27] = [
        192000, 96000, 48000, 38400, 24000, 23040, 19200, 11520, 9600, 7680, 6000, 5760, 4800, 3840, 3340, 3000, 2880,
        2400, 1920, 1440, 1200, 960, 720, 600, 480, 360, 240,
    ];
    if line_freq == 50.0 {
        &F50
    } else if line_freq == 60.0 {
        &F60
    } else {
        &[]
    }
}

/// «Длительность»: `"%g с."` of the record duration (`0x44c0b0`).
pub fn duration_text(osc: &comtrade::Oscillogram) -> String {
    format!("{} с.", fvb_core::views::fmt_g(osc.duration()))
}

/// The bar's / dialog's start and trigger texts (`0x456e90` with 0 / 1).
pub fn stamp_texts(osc: &comtrade::Oscillogram, utc: bool) -> (String, String) {
    let s = &osc.stamps;
    (
        comtrade::format_stamp(&s.start_date, &s.start_time, osc.rev_year, &s.local_code, utc),
        comtrade::format_stamp(&s.trigger_date, &s.trigger_time, osc.rev_year, &s.local_code, utc),
    )
}

/// Code-page combo of DIALOG 392 / 332 (DLGINIT: 0 «866 MS-DOS», 1 «Windows-1251»).
const CODEPAGES: [(Encoding, &str); 2] = [(Encoding::Cp866, "866 MS-DOS"), (Encoding::Cp1251, "Windows-1251")];

fn codepage_label(e: Encoding) -> &'static str {
    CODEPAGES.iter().find(|(x, _)| *x == e).map_or("", |(_, l)| l)
}

impl Doc {
    /// 32867/32868 CMDUI (`0x42ffb0`): the radio follows channel 0.
    pub fn primary_values(&self) -> bool {
        self.chans.first().is_some_and(|c| c.primary)
    }

    /// 32867 / 32868's check (`0x42ffb0`): from the first analog channel;
    /// `None` (neither checked) with no analog channel.
    pub fn primary_shown(&self) -> Option<bool> {
        self.chans.first().map(|c| c.primary)
    }

    /// The raw bytes the code-page guess reads (`0x44f74d`: station and
    /// recorder via `0x4573d0`, then every channel name appended).
    fn name_bytes(&self) -> Vec<u8> {
        let names = [self.osc.station.as_str(), self.osc.device.as_str()]
            .into_iter()
            .chain(self.osc.analog.iter().map(|c| c.id.as_str()))
            .chain(self.osc.digital.iter().map(|c| c.id.as_str()));
        names.flat_map(|n| comtrade::encode_text(n, self.codepage)).collect()
    }

    /// After a file is read: the names are decoded with the program encoding;
    /// for the two single-byte code pages the record code page is then
    /// guessed from the channel names and applied (`0x44f5b0` → `0x459e60`
    /// → `0x4543e0`); a saved `Oscillogram/Codepage` would override it (the
    /// `.set` file is not modelled). The `.hdr` text comes from the `.cff`
    /// section or the sibling `.hdr` file.
    pub fn init_record_text(&mut self, enc: Encoding) {
        self.codepage = enc;
        let single = matches!(enc, Encoding::Cp1251 | Encoding::Cp866);
        if single {
            let cp = comtrade::detect_codepage(&self.name_bytes());
            self.set_codepage(cp);
        }
        let raw: Option<Vec<u8>> = match &self.cff.hdr {
            Some(t) => Some(comtrade::encode_text(t, enc)),
            None => self.path.as_ref().and_then(|p| {
                ["hdr", "HDR", "Hdr"].iter().find_map(|e| std::fs::read(p.with_extension(e)).ok())
            }),
        };
        if let Some(b) = raw {
            self.hdr_cp = if single { comtrade::detect_codepage(&b) } else { enc };
            self.hdr = Some(comtrade::decode_bytes(&b, self.hdr_cp));
        }
    }

    /// «Кодировка» of DIALOG 392 (`0x4543e0`): re-decode the station, the
    /// recorder and every channel name in the new code page.
    pub fn set_codepage(&mut self, cp: Encoding) {
        let from = self.codepage;
        if from == cp {
            return;
        }
        let rc = |s: &mut String| *s = comtrade::recode(s, from, cp);
        rc(&mut self.osc.station);
        rc(&mut self.osc.device);
        self.osc.analog.iter_mut().for_each(|c| rc(&mut c.id));
        self.osc.digital.iter_mut().for_each(|c| rc(&mut c.id));
        self.codepage = cp;
    }

    /// The `.cff` text sections to write, with the edited record comment.
    pub fn cff_sections(&self) -> comtrade::CffSections {
        let mut s = self.cff.clone();
        if self.hdr.is_some() {
            s.hdr = self.hdr.clone();
        }
        s
    }

    /// The `.hdr` file to write beside a saved `.cfg` (CRLF lines, in the
    /// comment's code page), if the record has a comment.
    pub fn hdr_file_bytes(&self) -> Option<Vec<u8>> {
        let t = self.hdr.as_ref().filter(|t| !t.is_empty())?;
        Some(comtrade::encode_text(&t.replace("\r\n", "\n").replace('\n', "\r\n"), self.hdr_cp))
    }

    /// Recompute every derived series after a coefficient, primary/secondary
    /// or sample-rate change (`0x42e540`): phasors (keeping the channel
    /// roles), one-period RMS and DC, and the formula channels.
    /// A channel was added or removed: the original rebuilds its frequency
    /// candidates (`0x42ebd0`), which turns «Не корректировать» back into the
    /// first candidate whenever there is one (L9.65).
    pub fn channels_changed(&mut self) {
        // Formula channels (after the analog ones) and pasted analog ones
        // (at their insertion point) are candidates too, by the same test
        // (`0x42aee0`: ph / ccbm «system» or a frequency unit — no bit-14
        // test); they keep their vector places.
        let n_analog = self.osc.analog.len();
        let mut extra = Vec::new();
        for (j, c) in self.computed.iter().enumerate() {
            let pos = match c.before {
                Some(crate::app::ChanKey::Analog(a)) => a,
                _ => n_analog,
            };
            if c.formula == crate::merge_ui::PENDING {
                if let Some(Ok(a)) = self.pending_of(j).map(|p| &p.pasted.channel) {
                    if a.phase == "system" || a.ccbm == "system" || fvb_core::freqcorr::is_freq_unit(&a.unit) {
                        extra.push(fvb_core::comtrade::ExtraFreq { name: c.name.clone(), samples: a.samples.clone(), pos, id: j });
                    }
                }
            } else if fvb_core::freqcorr::is_freq_unit(&c.unit) {
                extra.push(fvb_core::comtrade::ExtraFreq { name: c.name.clone(), samples: c.samples.clone(), pos: n_analog, id: j });
            }
        }
        self.osc.extra_freq = extra;
        // The rebuild (`0x42ebd0`) refreshes no cache (L9.103): a changed
        // selection reaches the statistics panel and the frequency diagram
        // at once (they read it live) but the cached series — phasors, RMS
        // lanes, formula channels — only at the next full recompute.
        fvb_core::freqcorr::refresh_selection(&mut self.osc);
    }

    /// The correction for computed channel `j`: none for the frequency
    /// channel itself (the gate compares the channel, `0x4262b6`).
    pub fn computed_corr(&self, j: usize) -> Option<fvb_core::freqcorr::FreqCorr<'_>> {
        fvb_core::freqcorr::of(&self.osc).filter(|_| fvb_core::freqcorr::selected_extra(&self.osc) != Some(j))
    }

    pub fn recompute_signals(&mut self) {
        let roles = (self.analysis.u_abc, self.analysis.i_abc, self.analysis.calc_ib);
        let primary = self.primary_values();
        self.recompute(primary);
        (self.analysis.u_abc, self.analysis.i_abc, self.analysis.calc_ib) = roles;
    }
}

impl App {
    /// A full recompute of document `di`; ОМП is re-run silently when its
    /// result is on this document (`doc+0x690`).
    pub fn recompute_doc(&mut self, di: usize) {
        let Some(d) = self.docs.get_mut(di) else { return };
        d.recompute_signals();
        if self.docs[di].omp.shown {
            let (active, status) = (self.active, self.status.clone());
            self.active = Some(di);
            self.run_omp();
            self.active = active;
            self.status = status;
        }
    }

    /// 32867 / 32868 (`0x42fdb0`): `ch+0x89` := 1 / 0 on every file channel,
    /// then the full recompute.
    pub fn set_primary_values(&mut self, primary: bool) {
        let Some(ai) = self.active.filter(|&a| a < self.docs.len()) else { return };
        for c in self.docs[ai].chans.iter_mut() {
            c.primary = primary;
        }
        self.recompute_doc(ai);
    }

    /// `Settings Parameters`, read at start-up (`0x448c00`).
    pub fn load_program_settings(&mut self) {
        let s = &self.store.settings;
        if let Some(v) = s.get("Dst folder") {
            self.dlg.dst_folder = v.clone();
        }
        if let Some(v) = s.get("UTC for Comtrade2013") {
            self.dlg.utc_2013 = v != "0";
        }
        if let Some(c) = s.get("Display Encoding").and_then(|v| v.parse::<u8>().ok()) {
            self.encoding = encoding_of(c);
        }
    }

    /// Write `Settings Parameters` (the original at exit, `0x448da0`; we on OK).
    fn save_program_settings(&mut self) {
        if !self.persist {
            return;
        }
        let s = &mut self.store.settings;
        s.insert("Dst folder".into(), self.dlg.dst_folder.clone());
        s.insert("UTC for Comtrade2013".into(), u8::from(self.dlg.utc_2013).to_string());
        s.insert("Display Encoding".into(), encoding_code(self.encoding).to_string());
        self.store.save();
    }

    /// The record comment editor is enabled for `.cfg` / `.cff` records only
    /// (CMDUI `0x44c5e0`).
    pub fn hdr_comment_enabled(&self) -> bool {
        self.active_doc().and_then(|d| d.path.as_ref()).and_then(|p| p.extension()).and_then(|e| e.to_str()).is_some_and(|e| {
            e.eq_ignore_ascii_case("cfg") || e.eq_ignore_ascii_case("cff")
        })
    }

    /// 57603 «Сохранить» (`0x475750`): greyed for a `.sto` record (its path
    /// lowercased ends in ".sto") and with no document.
    pub fn can_save(&self) -> bool {
        self.active_doc().is_some_and(|d| !d.path.as_ref().is_some_and(|p| p.to_string_lossy().to_lowercase().ends_with(".sto")))
    }

    /// 32800 / 32801 / 32802 / 32803's states (`0x45c8c0`): YAnalogSize
    /// below 500 / above 70, `f` below 1.0 / above the fit scale; all
    /// greyed with no document.
    pub fn zoom_enabled(&self) -> [bool; 4] {
        let Some(d) = self.active_doc() else { return [false; 4] };
        let y = self.y_analog.round() as i32;
        let [grow_x, shrink_x] = d.zoom_x_enabled(self.view_width);
        [y < 500, y > 70, grow_x, shrink_x]
    }

    pub fn open_hdr_comment(&mut self) {
        if !self.hdr_comment_enabled() {
            return;
        }
        if let Some(d) = self.active_doc() {
            let cp = if matches!(d.hdr_cp, Encoding::Cp866 | Encoding::Cp1251) { d.hdr_cp } else { Encoding::Cp1251 };
            let text = comtrade::recode(d.hdr.as_deref().unwrap_or(""), d.hdr_cp, cp);
            self.dlg.hdr_edit = Some(HdrEdit { text, cp });
        }
    }

    /// Everything of this module and `chandlg`, called from `dialogs_ui`.
    pub fn record_dialogs_ui(&mut self, ctx: &egui::Context) {
        self.file_params_window(ctx);
        self.settings_window(ctx);
        self.hdr_comment_window(ctx);
        self.channels_window(ctx);
        self.coef_window(ctx);
        self.alert_window(ctx);
    }

    fn alert_window(&mut self, ctx: &egui::Context) {
        let Some(msg) = self.dlg.alert.clone() else { return };
        let mut ok = false;
        crate::dialogs::modal_backdrop(ctx, "fv_alert_block", egui::Order::Middle);
        egui::Window::new(crate::app::APP_TITLE)
            .id(egui::Id::new("fv_alert"))
            .collapsible(false)
            .resizable(false)
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                crate::dialogs::message_body(ui, crate::dialogs::MsgIcon::Exclamation, &msg);
                ui.vertical_centered(|ui| ok = ui.button("    OK    ").clicked());
            });
        if ok || ctx.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Escape)) {
            self.dlg.alert = None;
        }
    }

    /// DIALOG 392 «Параметры осциллограммы»: «Закрыть» is IDOK and applies
    /// the sample rate and the correction choice (`0x449f40`); Esc or the
    /// close box cancel them. The code page applies at once (`0x44a330`).
    fn file_params_window(&mut self, ctx: &egui::Context) {
        if !self.show_file_params {
            return;
        }
        let Some(ai) = self.active.filter(|&a| a < self.docs.len()) else {
            self.show_file_params = false;
            return;
        };
        let utc = self.dlg.utc_2013;
        let mut st = self.dlg.file_params.take().unwrap_or_else(|| FileParamsDlg {
            rate_text: format!("{}", self.docs[ai].osc.sample_rate),
            // CB_SETCURSEL `rec+0x874` (0 with no candidate, `0x449efd`).
            correction: if fvb_core::freqcorr::candidate_names(&self.docs[ai].osc).is_empty() { 0 } else { self.docs[ai].osc.freq_sel.max(0) as usize },
        });
        let mut open = true;
        let mut apply = false;
        let mut cp_pick: Option<Encoding> = None;
        egui::Window::new("Параметры осциллограммы")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                let d = &self.docs[ai];
                let o = &d.osc;
                let (start, trig) = stamp_texts(o, utc);
                let ro = |ui: &mut egui::Ui, s: String| {
                    let mut s = s;
                    ui.add(egui::TextEdit::singleline(&mut s).interactive(false).desired_width(260.0));
                };
                egui::Grid::new("fv_fileparams").num_columns(2).spacing([12.0, 5.0]).show(ui, |ui| {
                    ui.label("Число каналов:");
                    ro(ui, (o.analog.len() + o.digital.len()).to_string());
                    ui.end_row();
                    ui.label("Число аналоговых каналов:");
                    ro(ui, o.analog.len().to_string());
                    ui.end_row();
                    ui.label("Число дискретных каналов:");
                    ro(ui, o.digital.len().to_string());
                    ui.end_row();
                    ui.label("Число измерений:");
                    ro(ui, o.sample_count().to_string());
                    ui.end_row();
                    ui.label("Частота сети:");
                    ro(ui, format!("{}", o.line_freq));
                    ui.end_row();
                    ui.label("Частота дискретизации:");
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut st.rate_text).desired_width(228.0));
                        ui.menu_button("▾", |ui| {
                            for r in rate_list(o.line_freq) {
                                if ui.button(r.to_string()).clicked() {
                                    st.rate_text = r.to_string();
                                    ui.close_menu();
                                }
                            }
                        });
                    });
                    ui.end_row();
                    ui.label("Частота для коррекции\nрасчётных значений:");
                    // «Не корректировать», then each frequency candidate by
                    // name (`0x4499e0`); disabled when there is none.
                    let items: Vec<String> = std::iter::once("Не корректировать".to_string())
                        .chain(fvb_core::freqcorr::candidate_names(o))
                        .collect();
                    ui.add_enabled_ui(items.len() > 1, |ui| {
                        let shown = items.get(st.correction).cloned().unwrap_or_default();
                        egui::ComboBox::from_id_salt("fv_corr").selected_text(shown).width(260.0).show_ui(ui, |ui| {
                            for (k, t) in items.iter().enumerate() {
                                ui.selectable_value(&mut st.correction, k, t);
                            }
                        });
                    });
                    ui.end_row();
                    ui.label("Станция:");
                    ro(ui, o.station.clone());
                    ui.end_row();
                    ui.label("Номер регистратора:");
                    ro(ui, o.device.clone());
                    ui.end_row();
                    ui.label("Начало записи:");
                    ro(ui, start);
                    ui.end_row();
                    ui.label("Момент запуска:");
                    ro(ui, trig);
                    ui.end_row();
                    ui.label("Длительность:");
                    ro(ui, duration_text(o));
                    ui.end_row();
                    ui.label("Кодировка:");
                    egui::ComboBox::from_id_salt("fv_codepage").selected_text(codepage_label(d.codepage)).width(260.0).show_ui(ui, |ui| {
                        for (e, l) in CODEPAGES {
                            if ui.selectable_label(d.codepage == e, l).clicked() {
                                cp_pick = Some(e);
                            }
                        }
                    });
                    ui.end_row();
                });
                ui.separator();
                ui.vertical_centered(|ui| apply = ui.button("  Закрыть  ").clicked());
            });
        if let Some(cp) = cp_pick {
            self.docs[ai].set_codepage(cp);
        }
        let esc = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        if apply {
            let rate = st.rate_text.trim().parse::<f64>().ok().filter(|r| r.is_finite() && *r > 0.0);
            match rate {
                None => {
                    // DDX of a double refuses non-numbers and keeps the dialog.
                    self.dlg.alert = Some("Введите число.".into());
                    self.dlg.file_params = Some(st);
                    return;
                }
                Some(r) if r != self.docs[ai].osc.sample_rate => {
                    // `0x44a24a`: undo snapshot, the new rate, the full
                    // recompute, the silent ОМП run (always) and the
                    // modified mark.
                    let d = &mut self.docs[ai];
                    let snap = App::snapshot_of_doc(d);
                    App::push_undo(d, snap);
                    d.osc.retime(r);
                    d.recompute_signals();
                    d.set.modified = true;
                    self.run_omp_silent(ai);
                }
                Some(_) => {}
            }
            // `0x44a2c9`: a new correction choice is stored, the document
            // marked modified and every channel recomputed.
            let d = &mut self.docs[ai];
            if st.correction as i32 != d.osc.freq_sel {
                d.osc.freq_sel = st.correction as i32;
                d.set.modified = true;
                d.recompute_signals();
            }
            self.show_file_params = false;
        } else if !open || esc {
            self.show_file_params = false;
        } else {
            self.dlg.file_params = Some(st);
        }
    }

    /// DIALOG 510 «Установка настроек программы»: OK stores and persists; the
    /// encoding applies to the next file opened (nothing is re-read).
    fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return;
        }
        let mut st = self.dlg.settings.take().unwrap_or_else(|| SettingsDlg {
            dst_folder: self.dlg.dst_folder.clone(),
            utc: self.dlg.utc_2013,
            encoding: self.encoding,
        });
        let mut open = true;
        let mut result: Option<bool> = None;
        egui::Window::new("Установка настроек программы")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label("Директория хранения файлов описания осциллограмм (*.dst):");
                ui.horizontal(|ui| {
                    let shown = if st.dst_folder.is_empty() { "C:\\".to_string() } else { st.dst_folder.clone() };
                    ui.add_enabled(!st.dst_folder.is_empty(), egui::Link::new(shown));
                    if ui.button("...").clicked() {
                        if let Some(p) = rfd::FileDialog::new().set_title("Директория хранения файлов описания осциллограмм").pick_folder() {
                            st.dst_folder = p.display().to_string();
                        }
                    }
                });
                ui.add_space(4.0);
                ui.group(|ui| {
                    ui.label("Настройки отображения времени для Comtrade 2013");
                    ui.radio_value(&mut st.utc, true, "Отображать в UTC");
                    ui.radio_value(&mut st.utc, false, "Отображать местное время");
                });
                ui.group(|ui| {
                    ui.label("Отображать согласно кодировке:");
                    for (e, l) in ENCODINGS {
                        ui.radio_value(&mut st.encoding, e, l);
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        result = Some(true);
                    }
                    if ui.button("Cancel").clicked() {
                        result = Some(false);
                    }
                });
            });
        match result {
            Some(true) => {
                self.dlg.dst_folder = st.dst_folder;
                self.dlg.utc_2013 = st.utc;
                self.encoding = st.encoding;
                self.save_program_settings();
                self.show_settings = false;
            }
            Some(false) => self.show_settings = false,
            None if open => self.dlg.settings = Some(st),
            None => self.show_settings = false,
        }
    }

    /// DIALOG 332 in record mode (32805, `0x44c2e0`): the code-page combo
    /// re-encodes the text being edited (`0x43e410`); OK stores text and code
    /// page (modified only when the code page changed).
    fn hdr_comment_window(&mut self, ctx: &egui::Context) {
        let Some(mut st) = self.dlg.hdr_edit.take() else { return };
        let mut open = true;
        let mut result: Option<bool> = None;
        egui::Window::new("Комментарии к диаграмме")
            .id(egui::Id::new("fv_hdr_comment"))
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                    ui.add(egui::TextEdit::multiline(&mut st.text).desired_width(540.0).desired_rows(22));
                });
                ui.horizontal(|ui| {
                    let mut pick = None;
                    egui::ComboBox::from_id_salt("fv_hdr_cp").selected_text(codepage_label(st.cp)).show_ui(ui, |ui| {
                        for (e, l) in CODEPAGES {
                            if ui.selectable_label(st.cp == e, l).clicked() {
                                pick = Some(e);
                            }
                        }
                    });
                    if let Some(e) = pick.filter(|e| *e != st.cp) {
                        st.text = comtrade::recode(&st.text, st.cp, e);
                        st.cp = e;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Отмена").clicked() {
                            result = Some(false);
                        }
                        if ui.button("OK").clicked() {
                            result = Some(true);
                        }
                    });
                });
            });
        match result {
            Some(true) => {
                if let Some(d) = self.active_doc_mut() {
                    d.hdr = Some(st.text.replace('\r', ""));
                    d.hdr_cp = st.cp;
                    d.store_user_data();
                }
            }
            Some(false) => {}
            None if open => self.dlg.hdr_edit = Some(st),
            None => {}
        }
    }

    /// The parameters bar (`CParamOscBar::OnDrawText` `0x47ed30`): labels
    /// right-aligned, values left-aligned, in three rows of the 75 px strip;
    /// only the labels without a document.
    pub fn param_bar_ui(&mut self, ui: &mut egui::Ui) {
        let pal = self.skin.palette();
        let rect = ui.max_rect();
        ui.allocate_rect(rect, egui::Sense::hover());
        let font = egui::FontId::proportional(12.0);
        let width = |s: &str| ui.fonts(|f| f.layout_no_wrap(s.to_string(), font.clone(), pal.text).size());
        let lab = ["Станция: ", "Номер регистратора: ", "Начало записи: ", "Момент запуска:", "Длительность: "];
        let sz = width(lab[1]);
        let (w, h) = (sz.x, sz.y);
        let w2 = width(lab[3]).x;
        let row = |k: usize| (rect.top() + 5.0 + k as f32 * (h + 5.0), rect.top() + 10.0 + (k + 1) as f32 * h + k as f32 * 5.0);
        let x0 = rect.left();
        let painter = ui.painter();
        let label = |text: &str, x1: f32, k: usize| {
            let (y0, y1) = row(k);
            painter.text(egui::pos2(x0 + x1, (y0 + y1) / 2.0), egui::Align2::RIGHT_CENTER, text.trim_end(), font.clone(), pal.text);
        };
        label(lab[0], w + 15.0, 0);
        label(lab[1], w + 15.0, 1);
        label(lab[2], w + w2 + 465.0, 0);
        label(lab[3], w + w2 + 465.0, 1);
        label(lab[4], w + w2 + 465.0, 2);
        let Some(d) = self.active_doc() else { return };
        let (start, trig) = stamp_texts(&d.osc, self.dlg.utc_2013);
        let value = |text: String, xa: f32, xb: f32, k: usize, ellipsis: bool| {
            let (y0, y1) = row(k);
            let r = Rect::from_min_max(egui::pos2(x0 + xa, y0), egui::pos2(x0 + xb, y1));
            let mut job = egui::text::LayoutJob::simple_singleline(text, font.clone(), pal.text);
            if ellipsis {
                job.wrap = egui::text::TextWrapping::truncate_at_width(r.width());
            }
            let g = ui.fonts(|f| f.layout_job(job));
            let pos = egui::pos2(r.left(), r.center().y - g.size().y / 2.0);
            painter.with_clip_rect(r.intersect(rect)).galley(pos, g, pal.text);
        };
        value(d.osc.station.clone(), w + 30.0, w + 420.0, 0, true);
        value(d.osc.device.clone(), w + 30.0, w + 470.0, 1, false);
        value(start, w + w2 + 480.0, w + w2 + 870.0, 0, false);
        value(trig, w + w2 + 480.0, w + w2 + 870.0, 1, false);
        value(duration_text(&d.osc), w + w2 + 480.0, w + w2 + 770.0, 2, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Command, DEMO_CFG, DEMO_DAT};
    use eframe::egui::Vec2;

    fn demo_app() -> App {
        let mut app = App::new();
        let osc = comtrade::parse(DEMO_CFG, DEMO_DAT).unwrap();
        app.add_doc(Doc::new(osc, "demo".into()));
        app
    }

    #[test]
    fn rate_lists_by_line_frequency() {
        assert_eq!(rate_list(50.0).len(), 27);
        assert_eq!(rate_list(50.0)[0], 160000);
        assert_eq!(rate_list(60.0)[14], 3340);
        assert!(rate_list(55.0).is_empty());
    }

    #[test]
    fn display_encoding_codes_have_no_auto() {
        assert_eq!(encoding_code(Encoding::Utf8), 0);
        assert_eq!(encoding_of(1), Encoding::Cp1251);
        assert_eq!(encoding_of(3), Encoding::Koi8r);
        assert!(ENCODINGS.iter().all(|(e, _)| *e != Encoding::Auto));
        // The program default is Windows-1251, UTC on, the bar shown.
        let app = App::new();
        assert_eq!(app.encoding, Encoding::Cp1251);
        assert!(app.dlg.utc_2013 && app.dlg.param_bar);
    }

    #[test]
    fn param_bar_toggles() {
        let mut app = demo_app();
        app.dispatch(Command::ToggleParamBar);
        assert!(!app.dlg.param_bar);
        app.dispatch(Command::ToggleParamBar);
        assert!(app.dlg.param_bar);
    }

    #[test]
    fn primary_secondary_is_a_per_channel_radio() {
        let mut app = demo_app();
        assert!(!app.active_doc().unwrap().primary_values());
        app.dispatch(Command::SetPrimaryValues(true));
        let d = app.active_doc().unwrap();
        assert!(d.chans.iter().all(|c| c.primary));
        assert!(d.primary_values());
        app.dispatch(Command::SetPrimaryValues(false));
        assert!(!app.active_doc().unwrap().primary_values());
    }

    #[test]
    fn recompute_reevaluates_formula_channels_and_keeps_roles() {
        let mut app = demo_app();
        app.append_computed(&[("X".into(), "Ua*2".into())]);
        let before = app.active_doc().unwrap().computed[0].samples[10];
        let d = app.active_doc_mut().unwrap();
        d.analysis.u_abc = [Some(2), Some(1), Some(0)];
        d.osc.analog[0].primary = 100.0;
        app.dispatch(Command::SetPrimaryValues(true));
        let d = app.active_doc().unwrap();
        assert_eq!(d.analysis.u_abc, [Some(2), Some(1), Some(0)]);
        // Primary values scale Ua by primary/secondary = 100.
        assert!((d.computed[0].samples[10] - before * 100.0).abs() < 1e-9 * before.abs().max(1.0));
    }

    #[test]
    fn codepage_switch_redecodes_names() {
        let mut app = demo_app();
        let d = app.active_doc_mut().unwrap();
        d.codepage = Encoding::Cp1251;
        let raw = comtrade::encode_text("Ток", Encoding::Cp866);
        d.osc.analog[0].id = comtrade::decode_bytes(&raw, Encoding::Cp1251);
        d.set_codepage(Encoding::Cp866);
        assert_eq!(d.osc.analog[0].id, "Ток");
    }

    #[test]
    fn init_guesses_the_code_page_from_the_names() {
        let osc = comtrade::parse(DEMO_CFG, DEMO_DAT).unwrap();
        let mut d = Doc::new(osc, "x".into());
        // Names as an 866 file read with the 1251 default.
        let as_1251 = |s: &str| comtrade::decode_bytes(&comtrade::encode_text(s, Encoding::Cp866), Encoding::Cp1251);
        let mut d866 = Doc::new(d.osc.clone(), "y".into());
        d866.osc.station = as_1251("Подстанция");
        d866.osc.analog[0].id = as_1251("Напряжение");
        d866.init_record_text(Encoding::Cp1251);
        assert_eq!(d866.codepage, Encoding::Cp866);
        assert_eq!((d866.osc.station.as_str(), d866.osc.analog[0].id.as_str()), ("Подстанция", "Напряжение"));
        // The demo: a 1251 station and Latin channel names stay 1251.
        d.init_record_text(Encoding::Cp1251);
        assert_eq!(d.codepage, Encoding::Cp1251);
        assert!(d.osc.station.starts_with("НТЦ"));
    }

    #[test]
    fn hdr_comment_only_for_cfg_and_cff() {
        let mut app = demo_app();
        assert!(!app.hdr_comment_enabled());
        app.active_doc_mut().unwrap().path = Some("/x/rec.osc".into());
        app.dispatch(Command::HdrComment);
        assert!(app.dlg.hdr_edit.is_none());
        app.active_doc_mut().unwrap().path = Some("/x/rec.CFG".into());
        app.dispatch(Command::HdrComment);
        assert!(app.dlg.hdr_edit.is_some());
        let d = app.active_doc_mut().unwrap();
        d.hdr = Some("строка 1\nстрока 2".into());
        d.hdr_cp = Encoding::Cp866;
        let b = d.hdr_file_bytes().unwrap();
        assert_eq!(comtrade::decode_bytes(&b, Encoding::Cp866), "строка 1\r\nстрока 2");
    }

    #[test]
    fn duration_is_percent_g_seconds() {
        let osc = comtrade::parse(DEMO_CFG, DEMO_DAT).unwrap();
        let t = duration_text(&osc);
        assert!(t.ends_with(" с."), "{t}");
        assert_eq!(t, format!("{} с.", fvb_core::views::fmt_g(osc.duration())));
    }

    /// The bar renders headless with and without a document.
    #[test]
    fn param_bar_renders() {
        for with_doc in [false, true] {
            let mut app = if with_doc { demo_app() } else { App::new() };
            let ctx = egui::Context::default();
            let _ = ctx.run(egui::RawInput { screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(1320.0, 860.0))), ..Default::default() }, |ctx| {
                egui::TopBottomPanel::top("parambar").exact_height(75.0).show(ctx, |ui| app.param_bar_ui(ui));
            });
        }
    }
}
