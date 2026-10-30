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
    /// Message boxes (the original's AfxMessageBox, MB_OK) waiting for OK,
    /// shown one after another as the original's modal boxes and posted
    /// exceptions come.
    pub alerts: std::collections::VecDeque<(String, crate::dialogs::MsgIcon)>,
    /// Frames the front box has been up (a box takes keys from its second
    /// frame: the key that raised it is not also its answer).
    pub alert_frames: u32,
}

impl DialogState {
    /// Queue an MB_OK box with the exclamation icon (MFC's default for a
    /// box with no icon).
    pub fn push_alert(&mut self, msg: impl Into<String>) {
        self.push_alert_icon(msg, crate::dialogs::MsgIcon::Exclamation);
    }

    pub fn push_alert_icon(&mut self, msg: impl Into<String>, icon: crate::dialogs::MsgIcon) {
        self.alerts.push_back((msg.into(), icon));
    }

    /// The box shown now.
    #[cfg(test)]
    pub fn alert(&self) -> Option<&str> {
        self.alerts.front().map(|(m, _)| m.as_str())
    }
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
            alerts: Default::default(),
            alert_frames: 0,
        }
    }
}

/// DIALOG 392 edits: the sample-rate combo text and the correction choice.
#[derive(Debug, Clone)]
pub struct FileParamsDlg {
    pub rate_text: String,
    pub correction: usize,
    /// A refused rate: the field takes the focus again (CDataExchange::Fail).
    pub fail: bool,
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
    pub fn init_record_text(&mut self, program: Encoding) {
        // A `.cfg` that fixes its own encoding (UTF-8, `comtrade::cfg_utf8`)
        // was decoded in it: that is the record's code page, not guessed.
        let enc = self.osc.text_enc.unwrap_or(program);
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
            let guess = if matches!(program, Encoding::Cp1251 | Encoding::Cp866) { comtrade::detect_codepage(&b) } else { program };
            // A UTF-8 record's `.hdr` (a 2013 export's header text) is read
            // as UTF-8 when it is valid UTF-8 — kept byte for byte on save.
            let utf8 = self.osc.text_enc == Some(Encoding::Utf8) && std::str::from_utf8(&b).is_ok();
            self.hdr_cp = if utf8 { Encoding::Utf8 } else { guess };
            self.hdr = Some(comtrade::decode_bytes(&b, self.hdr_cp));
            // The reader keeps the `.hdr` bytes (`doc+0x1e0`, written back
            // on every save) and stores their guessed code page as
            // `Comments/Codepage` (`0x450d1f`…`0x450d63`) — the guess
            // under the program encoding, as the original's.
            self.set.tree.section_mut("Comments").set_int("Codepage", crate::setfile::cp_code(guess));
        }
    }

    /// «Кодировка» of DIALOG 392 (`0x4543e0`): re-decode the station, the
    /// recorder and every channel name in the new code page — from the
    /// bytes as read, so 1251 → 866 → 1251 gives the names back (VM) —
    /// into the original's ANSI strings (`comtrade::best_fit_1251`: «БЛОКА»
    /// read as 866 shows «+T+¦L»). A name edited since is taken as its
    /// text in the old code page.
    pub fn set_codepage(&mut self, cp: Encoding) {
        let from = self.codepage;
        if from == cp {
            return;
        }
        let mut raw = std::mem::take(&mut self.name_raw).into_iter();
        let mut rc = |s: &mut String| {
            let bytes = match raw.next() {
                Some((b, seen)) if seen == *s => b,
                _ => comtrade::encode_text(s, from),
            };
            *s = comtrade::decode_bytes(&bytes, cp).chars().map(comtrade::best_fit_1251).collect();
            (bytes, s.clone())
        };
        let mut kept = vec![rc(&mut self.osc.station), rc(&mut self.osc.device)];
        kept.extend(self.osc.analog.iter_mut().map(|c| rc(&mut c.id)));
        kept.extend(self.osc.digital.iter_mut().map(|c| rc(&mut c.id)));
        self.name_raw = kept;
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
        self.recolor();
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

    /// A dialog's default button and Cancel (IsDialogMessage): Enter = OK,
    /// Esc = Cancel — not while a message box is up (it takes them).
    pub fn dialog_keys(&self, ctx: &egui::Context) -> Option<bool> {
        if !self.dlg.alerts.is_empty() {
            return None;
        }
        ctx.input(|i| if i.key_pressed(egui::Key::Enter) { Some(true) } else if i.key_pressed(egui::Key::Escape) { Some(false) } else { None })
    }

    pub fn open_hdr_comment(&mut self) {
        if !self.hdr_comment_enabled() {
            return;
        }
        // 32805 (`0x44c2e0`): the `.set`'s `Comments/UserData` — not the
        // `.hdr` text —, the node made if missing (kept on Cancel too).
        if let Some(d) = self.active_doc_mut() {
            let text = d.set.tree.section_mut("Comments").get_str("UserData", "");
            self.dlg.hdr_edit = Some(HdrEdit { text });
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
        let Some((msg, icon)) = self.dlg.alerts.front().cloned() else { return };
        let mut ok = false;
        crate::dialogs::modal_backdrop(ctx, "fv_alert_block", egui::Order::Middle);
        crate::dialogs::dlg(ctx, crate::app::APP_TITLE)
            .id(egui::Id::new("fv_alert"))
            .margin(0.0)
                .collapsible(false)
            .resizable(false)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ok = crate::dialogs::message_box(ui, icon, &msg, &["OK"]).is_some();
            });
        // The box takes Enter / Esc: consumed, so the dialog under it does
        // not see them too.
        let key = self.dlg.alert_frames > 0 && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter) | i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        if ok || key {
            self.dlg.alerts.pop_front();
            self.dlg.alert_frames = 0;
        } else {
            self.dlg.alert_frames += 1;
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
            rate_text: format!("{}", self.docs[ai].osc.rate_r),
            // CB_SETCURSEL `rec+0x874` (0 with no candidate, `0x449efd`).
            correction: if fvb_core::freqcorr::candidate_names(&self.docs[ai].osc).is_empty() { 0 } else { self.docs[ai].osc.freq_sel.max(0) as usize },
            fail: false,
        });
        let mut open = true;
        let mut apply = false;
        let mut cp_pick: Option<Encoding> = None;
        crate::dialogs::dlg(ctx, "Параметры осциллограммы")
            .margin(0.0)
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                use crate::dialogs::{dlu, win_label};
                let d = &self.docs[ai];
                let o = &d.osc;
                let (start, trig) = stamp_texts(o, utc);
                // The 347 × 277 DLU template (521 × 450 px): right-aligned
                // labels at x 12–13, the values as static text at 120–121.
                let (client, _) = ui.allocate_exact_size(egui::vec2(521.0, 450.0), egui::Sense::hover());
                let rows: [(i32, i32, &str, Option<String>); 10] = [
                    (12, 12, "Число каналов:", Some((o.analog.len() + o.digital.len()).to_string())),
                    (12, 30, "Число аналоговых каналов:", Some(o.analog.len().to_string())),
                    (12, 48, "Число дискретных каналов:", Some(o.digital.len().to_string())),
                    (12, 66, "Число измерений:", Some(o.sample_count().to_string())),
                    // DDX_Text of a double shows `%.*g` at 15 digits.
                    (12, 84, "Частота сети:", Some(fvb_core::views::fmt_gp(o.line_freq, 15))),
                    (13, 145, "Станция:", Some(o.station.clone())),
                    (13, 162, "Номер регистратора:", Some(o.device.clone())),
                    (13, 181, "Начало записи:", Some(start)),
                    (13, 199, "Момент запуска:", Some(trig)),
                    (13, 217, "Длительность:", Some(duration_text(o))),
                ];
                // The controls in the template's order, the tab order:
                // «Закрыть» first (the initial focus), the rate combo no tab
                // stop, then «Кодировка» and the correction combo (VM).
                apply = crate::dialogs::win_button(ui, dlu(client, 258, 256, 80, 14), "Закрыть", true).clicked();
                for (x, y, label, value) in rows {
                    win_label(ui, dlu(client, x, y, 96, 9), label, true, true);
                    if let Some(v) = value {
                        win_label(ui, dlu(client, if x == 12 { 120 } else { 121 }, y, 216, 9), &v, false, true);
                    }
                }
                win_label(ui, dlu(client, 13, 102, 96, 9), "Частота дискретизации:", true, true);
                let focus = st.fail && self.dlg.alerts.is_empty();
                if focus {
                    st.fail = false;
                }
                let rates: Vec<String> = rate_list(o.line_freq).iter().map(|r| r.to_string()).collect();
                crate::dialogs::no_tab_stop(ui, |ui| crate::dialogs::win_combo_edit(ui, dlu(client, 120, 102, 80, 12), egui::Id::new("fv_rate"), &mut st.rate_text, &rates, focus));
                win_label(ui, dlu(client, 12, 234, 96, 9), "Кодировка:", true, true);
                let cps: Vec<String> = CODEPAGES.iter().map(|(_, l)| l.to_string()).collect();
                let cur = CODEPAGES.iter().position(|(e, _)| *e == d.codepage).unwrap_or(0);
                if let Some(k) = crate::dialogs::win_combo_list(ui, dlu(client, 121, 233, 80, 70), egui::Id::new("fv_codepage"), &cps, cur, true) {
                    cp_pick = Some(CODEPAGES[k].0);
                }
                win_label(ui, dlu(client, 12, 123, 96, 17), "Частота для коррекции расчётных значений:", true, true);
                // «Не корректировать», then each frequency candidate by
                // name (`0x4499e0`); disabled when there is none.
                let items: Vec<String> = std::iter::once("Не корректировать".to_string())
                    .chain(fvb_core::freqcorr::candidate_names(o))
                    .collect();
                if let Some(k) = crate::dialogs::win_combo_list(ui, dlu(client, 119, 125, 80, 70), egui::Id::new("fv_corr"), &items, st.correction, items.len() > 1) {
                    st.correction = k;
                }
            });
        if let Some(cp) = cp_pick {
            self.docs[ai].set_codepage(cp);
        }
        // «Закрыть» is IDOK: Enter applies, Esc cancels.
        let keys = self.dialog_keys(ctx);
        let esc = keys == Some(false);
        let apply = apply || keys == Some(true);
        if apply {
            if let Err(fail) = self.apply_file_rate(ai, &st.rate_text) {
                st.fail = fail;
                self.dlg.file_params = Some(st);
                return;
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

    /// 392's sample rate on «Закрыть». 0x415 goes through DDX_Text of an
    /// int (mfc #3761, `0x44a230`: scanf "%d", L9.148) with no DDV, so 0
    /// and negatives are taken. A new R (`0x44a24a`): the undo snapshot, R
    /// stored and the nominal track (`0x42e540`); then each channel's
    /// recompute, which throws the format error when R div f is 0
    /// (`0x4285b9`) — after the snapshot and the store, before anything
    /// else: no recompute, ОМП or modified mark. Otherwise the full
    /// recompute, the silent ОМП run (always) and the modified mark.
    /// `Err(refocus)`: the dialog stays (UpdateData failed); `true` puts
    /// the caret back in the field (the int parse).
    pub fn apply_file_rate(&mut self, ai: usize, text: &str) -> Result<(), bool> {
        let Some(rate) = crate::dialogs::ddx_int(text) else {
            // AFX_IDP_PARSE_INT.
            self.dlg.push_alert("Введите целое число.");
            return Err(true);
        };
        let rate = f64::from(rate);
        if rate == self.docs[ai].osc.rate_r {
            return Ok(());
        }
        let d = &mut self.docs[ai];
        let snap = App::snapshot_of_doc(d);
        App::push_undo(d, snap);
        d.osc.set_rate_r(rate);
        if fvb_core::signal::samples_per_period(rate, d.osc.line_freq) == Some(0) {
            // The exception's text with an empty detail: no file name.
            self.dlg.push_alert(fvb_core::comtrade::ComtradeError::Format.to_string());
            return Err(false);
        }
        d.recompute_signals();
        d.set.modified = true;
        self.run_omp_silent(ai);
        Ok(())
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
        crate::dialogs::dlg(ctx, "Установка настроек программы")
            .margin(0.0)
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                use crate::dialogs::{dlu, win_button, win_group, win_label, win_radio};
                // The 419 × 259 DLU template (629 × 421 px).
                let (client, _) = ui.allocate_exact_size(egui::vec2(629.0, 421.0), egui::Sense::hover());
                // The template's order is the tab order: OK, Cancel, the
                // link, «...», then each group's checked radio (an auto radio
                // button takes WS_TABSTOP while checked; VM: Tab after «...»
                // lands on «Отображать в UTC»).
                if win_button(ui, dlu(client, 301, 238, 50, 14), "OK", true).clicked() {
                    result = Some(true);
                }
                if win_button(ui, dlu(client, 362, 238, 50, 14), "Cancel", false).clicked() {
                    result = Some(false);
                }
                win_label(ui, dlu(client, 7, 16, 405, 8), "Директория хранения файлов описания осциллограмм (*.dst):", false, true);
                win_group(ui, dlu(client, 7, 28, 405, 30), "");
                // The folder as an MFC link (`0x47f840`): its text and URL,
                // disabled — and empty — while no folder is set; a click
                // opens the folder.
                let link = dlu(client, 16, 38, 386, 15);
                if !st.dst_folder.is_empty() {
                    let resp = ui.interact(link, ui.id().with("fv_dst_link"), egui::Sense::click());
                    let focused = crate::dialogs::tab_stop(ui, resp.id, crate::dialogs::StopKind::Button);
                    let t = ui.painter().with_clip_rect(link).text(link.left_center(), egui::Align2::LEFT_CENTER, &st.dst_folder, crate::dialogs::DLG_FONT, egui::Color32::from_rgb(0, 0, 255));
                    if focused && crate::dialogs::cues(ui) {
                        crate::dialogs::focus_rect(ui.painter(), t.expand(1.0));
                    }
                    if resp.hovered() {
                        ui.painter().hline(t.x_range(), t.bottom() - 0.5, egui::Stroke::new(1.0f32, egui::Color32::from_rgb(0, 0, 255)));
                    }
                    if resp.clicked() {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(format!("file:///{}", st.dst_folder.replace('\\', "/"))));
                    }
                }
                // «...» (`0x47f950`): the folder browser starts at the current one.
                if win_button(ui, dlu(client, 6, 60, 50, 14), "...", false).clicked() {
                    let mut fd = crate::dialogs::file_dialog().set_title("Директория хранения файлов описания осциллограмм");
                    if !st.dst_folder.is_empty() {
                        fd = fd.set_directory(&st.dst_folder);
                    }
                    if let Some(p) = fd.pick_folder() {
                        st.dst_folder = p.display().to_string();
                    }
                }
                win_group(ui, dlu(client, 7, 86, 405, 51), "Настройки отображения времени для Comtrade 2013");
                if win_radio(ui, dlu(client, 23, 104, 80, 10), "Отображать в UTC", st.utc) {
                    st.utc = true;
                }
                if win_radio(ui, dlu(client, 23, 120, 110, 10), "Отображать местное время", !st.utc) {
                    st.utc = false;
                }
                win_group(ui, dlu(client, 7, 148, 405, 83), "Отображать согласно кодировке:");
                for ((e, l), (y, w)) in ENCODINGS.into_iter().zip([(164, 35), (180, 63), (197, 40), (213, 39)]) {
                    if win_radio(ui, dlu(client, 23, y, w, 10), l, st.encoding == e) {
                        st.encoding = e;
                    }
                }
                win_label(ui, dlu(client, 135, 176, 211, 22), "Внимание! При смене кодировки изменения вступают в силу после переоткрытия файла осциллограммы!", false, true);
            });
        let result = result.or_else(|| self.dialog_keys(ctx));
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
        crate::dialogs::dlg(ctx, "Комментарии к диаграмме")
            .id(egui::Id::new("fv_hdr_comment"))
            .margin(0.0)
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                // The code-page combo stays hidden: the dialog's mode
                // (`+0x9c`) is always 0 (`0x43e110`, `0x43e380`).
                result = crate::dialogs::comment_dialog_body(ui, egui::Id::new("fv_hdr_comment_text"), &mut st.text);
            });
        match result {
            // OK: CRs removed, `UserData` written, modified every time.
            Some(true) => {
                if let Some(d) = self.active_doc_mut() {
                    d.set.tree.section_mut("Comments").set_str("UserData", &st.text.replace('\r', ""));
                    d.set.modified = true;
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
        // The rows from the bar's top (VM: the first centred 10 px down).
        // `0x47ed30` in the bar window's coordinates, whose top is 4 px
        // above the box: row k from y 5 + k(h + 5), h + 5 high.
        let row = |k: usize| (rect.top() + 1.0 + k as f32 * (h + 5.0), rect.top() + 6.0 + (k + 1) as f32 * h + k as f32 * 5.0);
        let x0 = rect.left();
        let painter = ui.painter().clone();
        let label = |text: &str, x1: f32, k: usize| {
            let (y0, y1) = row(k);
            // Drawn right-aligned with their trailing spaces (`0x47ed30`), so
            // «Номер регистратора: » ends a space before its column's edge.
            painter.text(egui::pos2(x0 + x1, (y0 + y1) / 2.0), egui::Align2::RIGHT_CENTER, text, font.clone(), pal.text);
        };
        label(lab[0], w + 15.0, 0);
        label(lab[1], w + 15.0, 1);
        label(lab[2], w + w2 + 465.0, 0);
        label(lab[3], w + w2 + 465.0, 1);
        label(lab[4], w + w2 + 465.0, 2);
        // Created in MFC's message-bar mode (`0x474dc5`: Create(…, 810, −1,
        // TRUE)): its close button hides the bar, as 142 does (`0x47ecf0`).
        if crate::dialogs::caption_close(ui, rect, ui.id().with("fv_param_close")) {
            self.dlg.param_bar = false;
            return;
        }
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
    fn a_new_rate_changes_the_windows_not_the_time_axis() {
        let mut app = demo_app();
        let d = &app.docs[0];
        let (time, n, r) = (d.osc.time.clone(), d.osc.samples_per_cycle(), d.osc.rate_r);
        let half = (r / 2.0) as i32;
        assert_eq!(app.apply_file_rate(0, &half.to_string()), Ok(()));
        let d = &app.docs[0];
        assert_eq!(d.osc.time, time, "the ruler stays");
        assert_eq!(d.osc.samples_per_cycle(), n / 2);
        assert!(d.set.modified);
        assert_eq!(d.undo.len(), 1);
        // The same value again: nothing.
        app.docs[0].set.modified = false;
        assert_eq!(app.apply_file_rate(0, &format!("{half} Гц")), Ok(()));
        assert!(!app.docs[0].set.modified);
        assert_eq!(app.docs[0].undo.len(), 1);
    }

    #[test]
    fn a_rate_under_the_line_frequency_is_a_format_error_after_the_store() {
        let mut app = demo_app();
        assert_eq!(app.apply_file_rate(0, "0"), Err(false));
        assert_eq!(app.dlg.alert(), Some("Неправильный формат файла"));
        let d = &app.docs[0];
        assert_eq!((d.osc.rate_r, d.undo.len(), d.set.modified), (0.0, 1, false), "stored and undoable, not modified");
        assert!(d.osc.freq_track.iter().all(|f| f.is_infinite()));
        // «Закрыть» again: equal to the stored R, it closes.
        app.dlg.alerts.clear();
        assert_eq!(app.apply_file_rate(0, "0"), Ok(()));
        assert!(app.dlg.alerts.is_empty());
        assert_eq!(app.apply_file_rate(0, "x"), Err(true));
    }

    #[test]
    fn a_negative_rate_divides_unsigned_and_leaves_no_window() {
        let mut app = demo_app();
        assert_eq!(app.apply_file_rate(0, "-5"), Ok(()));
        let d = &app.docs[0];
        assert_eq!(d.osc.rate_r, -5.0);
        assert_eq!(fvb_core::signal::samples_per_period(-5.0, 50.0), Some(85_899_345));
        assert_eq!(d.osc.samples_per_cycle(), d.osc.sample_count() + 1);
        assert!(d.analysis.phasors.iter().all(|p| p.iter().all(|z| z.norm() == 0.0)));
        assert!(d.set.modified);
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
    fn a_1251_name_read_as_866_takes_the_best_fit_and_comes_back() {
        let mut app = demo_app();
        let d = app.active_doc_mut().unwrap();
        d.codepage = Encoding::Cp1251;
        d.osc.analog[0].id = "IA БЛОКА 3".into();
        d.osc.analog[1].id = "UA I CШ".into();
        d.init_set_keys();
        d.set_codepage(Encoding::Cp866);
        assert_eq!((d.osc.analog[0].id.as_str(), d.osc.analog[1].id.as_str()), ("IA +T+¦L 3", "UA I C+"));
        assert_eq!(d.original_name(crate::app::ChanKey::Analog(0)), None, "still the file's name");
        d.set_codepage(Encoding::Cp1251);
        assert_eq!((d.osc.analog[0].id.as_str(), d.osc.analog[1].id.as_str()), ("IA БЛОКА 3", "UA I CШ"));
        // A name renamed meanwhile is re-read from its own text.
        d.set_codepage(Encoding::Cp866);
        d.osc.analog[0].id = "Ток".into();
        d.set_codepage(Encoding::Cp1251);
        assert_eq!(d.osc.analog[0].id, "’®Є");
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

    /// A record whose `.cfg` fixed UTF-8 (issue #8): that is its code page
    /// under any program encoding — no guess re-decodes its names — and its
    /// UTF-8 `.hdr` is read as UTF-8 and written back byte for byte, while
    /// `Comments/Codepage` keeps the guess under the program encoding.
    #[test]
    fn a_utf8_record_keeps_utf8_names_and_hdr() {
        let dir = std::env::temp_dir().join(format!("fv_utf8_hdr_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let hdr = "Коментар: синтетичний запис\r\nПримітка: тест\r\n";
        std::fs::write(dir.join("rec.hdr"), hdr).unwrap();
        let mut osc = comtrade::parse(DEMO_CFG, DEMO_DAT).unwrap();
        osc.text_enc = Some(Encoding::Utf8);
        osc.station = "Підстанція".into();
        osc.analog[0].id = "Струм фази А".into();
        osc.analog[1].id = "Гр.\u{FFFD}.:Напруга".into();
        let mut d = Doc::new(osc, "x".into());
        d.path = Some(dir.join("rec.cfg"));
        d.init_record_text(Encoding::Cp1251);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(d.codepage, Encoding::Utf8);
        assert_eq!((d.osc.station.as_str(), d.osc.analog[0].id.as_str(), d.osc.analog[1].id.as_str()), ("Підстанція", "Струм фази А", "Гр.\u{FFFD}.:Напруга"));
        assert_eq!((d.hdr.as_deref(), d.hdr_cp), (Some(hdr), Encoding::Utf8));
        assert_eq!(d.hdr_file_bytes().unwrap(), hdr.as_bytes());
        let guess = crate::setfile::cp_code(comtrade::detect_codepage(hdr.as_bytes()));
        assert_eq!(d.set.tree.section("Comments").unwrap().get_int("Codepage", -1), guess);
        // «Кодировка» then re-reads the UTF-8 bytes in the code page picked.
        d.set_codepage(Encoding::Cp1251);
        assert_eq!(d.osc.station, comtrade::decode_bytes("Підстанція".as_bytes(), Encoding::Cp1251));
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
        // The dialog edits `Comments/UserData`, never the `.hdr` text.
        d.hdr = Some("hdr".into());
        app.dlg.hdr_edit = None;
        app.dispatch(Command::HdrComment);
        assert_eq!(app.dlg.hdr_edit.as_ref().unwrap().text, "");
        app.dlg.hdr_edit.as_mut().unwrap().text = "a\r\nb".into();
        let d = app.active_doc_mut().unwrap();
        d.set.modified = false;
        let st = app.dlg.hdr_edit.take().unwrap();
        let d = app.active_doc_mut().unwrap();
        d.set.tree.section_mut("Comments").set_str("UserData", &st.text.replace('\r', ""));
        assert_eq!(d.set.tree.section("Comments").unwrap().get_str("UserData", ""), "a\nb");
        assert_eq!(d.hdr.as_deref(), Some("hdr"));
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
