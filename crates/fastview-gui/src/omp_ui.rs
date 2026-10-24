//! ОМП (fault location) user interface, as the original presents it
//! (`notes/UI_BEHAVIOUR_SPEC.md` §8): the ribbon's «Анализ» is a toggle
//! (32806, `0x449890`), the result is a message bar under the ribbon
//! (CMsgCaptionBar 221) whose text follows В1/В2 live and whose «Сохранить...»
//! (32879, `0x4a1930`) writes the `.txt` report; hiding the bar clears the
//! result (32830, `0x475210` → `0x4a17a0`). «Параметры» (32807, `0x454240`)
//! is the modal DIALOG 336 «Параметры ОМП» (WrongPlaceLocatorDlg): toolbar,
//! element tree and property grid over a working copy of the document's
//! settings; OK validates, stores and runs the analysis. The engine is
//! `fvb_core::omp`, the settings model `fvb_core::omp_params`.

use crate::app::{App, Computed, Doc};
use eframe::egui::{self, Color32, RichText};
use fvb_core::omp::{NeutralMode, OmpResult, Section, Trace};
use fvb_core::omp_params::{self as params, OmpParams, SectionParams, DOUBLE_ENDED, GROUNDED, ISOLATED};
use std::collections::HashSet;

/// Names of the result channels: «Lкз» (analog, metres) and the phase flags
/// as digital channels, added in the order C, B, А (`0x4a1538`..`0x4a15f6`;
/// string 3225 has a Cyrillic «А», 3223/3224 Latin B/C).
pub const L_KZ: &str = "Lкз";
pub const KZ_PHASES: [&str; 3] = ["КЗ фазы А", "КЗ фазы B", "КЗ фазы C"];

/// Title of the application's message boxes (string 128).
use crate::app::APP_TITLE;

/// The ОМП object of a document (`doc+0x308`): its settings, the last run and
/// the «result shown» flag (`doc+0x690`, `obj+0x388`).
#[derive(Default)]
pub struct OmpDoc {
    pub params: OmpParams,
    pub run: Option<OmpRun>,
    pub shown: bool,
}

/// A finished run: the per-sample trace and the sections it used (the live
/// text re-reads them when В1/В2 move).
pub struct OmpRun {
    pub method: u32,
    pub trace: Trace,
    pub sections: Vec<Section>,
}

/// Frame-level ОМП state: the result bar, the parameters dialog and the
/// pending message box.
#[derive(Default)]
pub struct OmpUi {
    /// Text of the result bar while it is shown.
    pub bar: Option<String>,
    /// The document the bar was last bound to (view activation re-binds it).
    pub bar_doc: Option<usize>,
    pub dialog: Option<ParamsDialog>,
    /// A message box (errors of the run, validation of the dialog).
    pub message: Option<String>,
    /// Screen rectangle of the bar's close button at the last frame.
    pub close_rect: Option<egui::Rect>,
}

/// Is a pasted channel still waiting to be merged (flag bit 14)? Our paste
/// adds such channels as computed channels marked «объединение» and the
/// merge step does not exist yet, so they stay pending.
pub fn has_pending_paste(d: &Doc) -> bool {
    d.computed.iter().any(|c| c.formula == "объединение")
}

/// Samples of the channel with exactly this name (file channels first, then
/// computed ones), as the run looks inputs up by the station's names.
fn refs(v: &[Vec<f64>; 3]) -> [&[f64]; 3] {
    [v[0].as_slice(), v[1].as_slice(), v[2].as_slice()]
}

/// The channel the engine takes for `name` (`0x42edf0`): every channel
/// whose name matches ignoring case (CompareNoCase, mfc #2954), the one
/// matching exactly (Compare, #2948) preferred, else the first — file
/// channels, then computed ones; `Err(j)` a computed channel.
fn find_chan(d: &Doc, name: &str) -> Option<Result<usize, usize>> {
    if name.is_empty() {
        return None;
    }
    let low = name.to_lowercase();
    let cands: Vec<(Result<usize, usize>, &str)> = d
        .osc
        .analog
        .iter()
        .enumerate()
        .map(|(i, c)| (Ok(i), c.id.as_str()))
        .chain(d.computed.iter().enumerate().map(|(j, c)| (Err(j), c.name.as_str())))
        .filter(|(_, n)| n.to_lowercase() == low)
        .collect();
    cands.iter().find(|(_, n)| *n == name).or(cands.first()).map(|(k, _)| *k)
}

fn chan_samples(d: &Doc, name: &str) -> Option<Vec<f64>> {
    // The flagged values as `0x426020` returns them (ADC / remove-DC /
    // invert, times the shown factor k — a «P» record shown in secondary
    // values is divided by its ratio; the cache ch+0x20 the engine
    // copies), ledger L9.7 / L9.134; VM: WIN_1999 with Inom 1000 gives
    // «КЗ отсутствует.».
    match find_chan(d, name)? {
        Ok(i) => {
            let k = d.shown_k(i);
            d.flagged.analog.get(i).map(|c| c.samples.iter().map(|x| x * k).collect())
        }
        Err(j) => Some(d.computed[j].samples.clone()),
    }
}

impl App {
    /// 32806 «Анализ»: clear a shown result, else run interactively.
    pub fn omp_toggle(&mut self) {
        let Some(di) = self.active else { return };
        if self.docs[di].omp.shown {
            self.omp_clear(di);
        } else if !has_pending_paste(&self.docs[di]) {
            self.run_omp();
        }
    }

    /// Interactive run (`0x4a00b0(0)`) on the active document; an error is
    /// shown in a message box and clears the result flag (the bar is left as
    /// it was).
    pub fn run_omp(&mut self) {
        let Some(di) = self.active else { return };
        match self.omp_compute(di) {
            Ok(()) => {
                self.omp_ui.bar_doc = Some(di);
                self.omp_ui.bar = self.omp_result().map(|r| r.text);
            }
            Err(id) => {
                self.docs[di].omp.shown = false;
                self.omp_ui.message = Some(params::message(id).to_string());
            }
        }
    }

    /// The silent run (`0x4a00b0(1)`) that «Объединить» and a sample-rate
    /// change make on document `di` whether or not a result is shown: on
    /// success the banner shows, an error only clears the result flag.
    pub fn run_omp_silent(&mut self, di: usize) {
        if di >= self.docs.len() {
            return;
        }
        match self.omp_compute(di) {
            Ok(()) => {
                self.omp_ui.bar_doc = Some(di);
                let active = self.active;
                self.active = Some(di);
                self.omp_ui.bar = self.omp_result().map(|r| r.text);
                self.active = active;
            }
            Err(_) => self.docs[di].omp.shown = false,
        }
    }

    /// The run (`0x4a00b0`): inputs by `_wtof` from the stored strings, the
    /// channels by the names in the station properties, the engine, the
    /// window written to В1 (start) / В2 (start + P − 1), and the result
    /// channels. Errors are string-table ids (349, 354).
    fn omp_compute(&mut self, di: usize) -> Result<(), u32> {
        let d = &self.docs[di];
        let pr = &d.omp.params;
        let (st1, st2) = (&pr.stations[0], &pr.stations[1]);
        let inom1 = params::wtof(&st1.inom);
        if st1.sections.is_empty() || inom1 == 0.0 {
            return Err(349);
        }
        let p = d.osc.samples_per_cycle().max(2);
        let get = |name: &str| chan_samples(d, name).ok_or(354u32);
        let three = |a: &str, b: &str, c: &str| -> Result<[Vec<f64>; 3], u32> { Ok([get(a)?, get(b)?, get(c)?]) };
        let method = pr.method;
        // Each slot's phasors (`0x497090` → `0x477280`): the tracked mode
        // for a channel with its own frequency vector (`0x42f250`, set by a
        // paste / «Объединить»), else the fixed path.
        let t_us: Vec<i64> = d.osc.time.iter().map(|t| (t * 1e6).round() as i64).collect();
        let slot_name = |s: fvb_core::omp::Slot| -> &str {
            let st = &pr.stations[s.station.min(1)];
            match (s.current, s.station == 0 && method == ISOLATED) {
                (true, _) => [&st.ia, &st.ib, &st.ic][s.phase.min(2)].as_str(),
                (false, true) => [&st.uab, &st.ubc, &st.uca][s.phase.min(2)].as_str(),
                (false, false) => [&st.ua, &st.ub, &st.uc][s.phase.min(2)].as_str(),
            }
        };
        let phasor = |s: fvb_core::omp::Slot, x: &[f64]| -> Vec<num_complex::Complex64> {
            let name = slot_name(s);
            let own = match find_chan(d, name) {
                Some(Ok(i)) => Some(&d.osc.analog[i]),
                _ => None,
            };
            match own.filter(|c| !c.own_freq.is_empty()) {
                Some(c) => fvb_core::omp::omp_phasors_tracked(x, p, &t_us, &c.own_freq, &d.osc.freq_track),
                None => fvb_core::omp::omp_phasors(x, p),
            }
        };
        let (trace, sections, result) = match method {
            ISOLATED | GROUNDED => {
                let i = three(&st1.ia, &st1.ib, &st1.ic)?;
                // The isolated method reads the line-voltage channels by name.
                let u = if method == ISOLATED { three(&st1.uab, &st1.ubc, &st1.uca)? } else { three(&st1.ua, &st1.ub, &st1.uc)? };
                let sections = pr.sections(|n| chan_samples(d, n).map(|s| fvb_core::omp::omp_phasors(&s, p)))?;
                let neutral = if method == ISOLATED { NeutralMode::Isolated } else { NeutralMode::Grounded };
                let mut t = fvb_core::omp::single_ended(&fvb_core::omp::SingleEnded {
                    neutral,
                    u: refs(&u),
                    i: refs(&i),
                    p,
                    inom: inom1,
                    sections: &sections,
                    phasor: Some(&phasor),
                });
                let r = fvb_core::omp::locate(&mut t, neutral, &sections, p);
                (t, sections, r)
            }
            DOUBLE_ENDED => {
                // Both stations from the same document (the remote record is
                // pasted and merged first, §8.6).
                let (u1, i1) = (three(&st1.ua, &st1.ub, &st1.uc)?, three(&st1.ia, &st1.ib, &st1.ic)?);
                let (u2, i2) = (three(&st2.ua, &st2.ub, &st2.uc)?, three(&st2.ia, &st2.ib, &st2.ic)?);
                let sections = pr.sections(|n| chan_samples(d, n).map(|s| fvb_core::omp::omp_phasors(&s, p)))?;
                let t = fvb_core::omp::double_ended(&fvb_core::omp::DoubleEnded {
                    u1: refs(&u1),
                    i1: refs(&i1),
                    u2: refs(&u2),
                    i2: refs(&i2),
                    p,
                    inom1,
                    // Station 2's Inom is stored negated (`obj+0x78`); the
                    // engine reads its magnitude.
                    inom2: -params::wtof(&st2.inom),
                    sections: &sections,
                    phasor: Some(&phasor),
                });
                let r = fvb_core::omp::locate_double(&t, &sections, p);
                (t, sections, r)
            }
            _ => return Err(349),
        };
        let lkz: Vec<f64> = trace.dist.iter().map(|x| x * 1000.0).collect();
        let flags = [&trace.pa, &trace.pb, &trace.pc];
        let d = &mut self.docs[di];
        let color = crate::app::channel_color(L_KZ, "", d.osc.analog.len() + d.computed.len());
        d.main_cursor = result.window.0;
        d.aux_cursor = result.window.1;
        d.v_shown[0] = true;
        d.v_shown[1] = true;
        // Same-named channels are replaced in place, others appended.
        match d.computed.iter().position(|c| c.name == L_KZ) {
            Some(j) => {
                d.computed[j].set_values(lkz, None);
                d.reflag_computed(j);
            }
            None => {
                let mut c = Computed::new(L_KZ.into(), "ОМП".into(), lkz, color);
                c.unit = "m".into();
                d.computed.push(c);
            }
        }
        // `0x42ead0` inserts each new flag at the first digital place, made
        // in the order C, B, A (`0x4a158a`…`0x4a15f6`): they read А, B, C
        // above the record's own digital channels.
        let (na, nd) = (d.osc.analog.len(), d.osc.digital.len());
        for k in [0, 1, 2] {
            let samples = flags[k].clone();
            match d.osc.digital.iter_mut().find(|c| c.id == KZ_PHASES[k]) {
                Some(c) => c.samples = samples,
                None => {
                    let index = d.osc.digital.len() as u32 + 1;
                    d.osc.digital.push(fvb_core::DigitalChannel { index, id: KZ_PHASES[k].into(), phase: String::new(), normal: 0, samples });
                }
            }
        }
        if d.osc.digital.len() > nd {
            d.digital_visible.resize(d.osc.digital.len(), true);
            d.move_appended(na, na, 0, nd);
        }
        d.channels_changed();
        d.omp.run = Some(OmpRun { method, trace, sections });
        d.omp.shown = true;
        Ok(())
    }

    /// `0x4a17a0`: the flag off, the bar hidden, the result channels
    /// deleted by name.
    pub fn omp_clear(&mut self, di: usize) {
        let Some(d) = self.docs.get_mut(di) else { return };
        d.omp.shown = false;
        d.omp.run = None;
        let removed = d.removing_channels(|d| {
            d.computed.retain(|c| c.name != L_KZ);
            // The flags lead the digital list: their shown / value flags go
            // with them.
            let keep: Vec<bool> = d.osc.digital.iter().map(|c| !KZ_PHASES.contains(&c.id.as_str())).collect();
            d.retain_digital_keys(&keep);
            let mut it = keep.iter();
            d.digital_visible.retain(|_| *it.next().unwrap_or(&true));
            let mut it = keep.iter();
            d.dig_flags.retain(|_| *it.next().unwrap_or(&true));
            d.osc.digital.retain(|c| !KZ_PHASES.contains(&c.id.as_str()));
        });
        if removed {
            d.channels_changed();
            // Rows were renumbered: drop them from the selection.
            d.selection.retain(|k| matches!(k, crate::app::ChanKey::Analog(_)));
            d.sel_anchor = None;
        }
        if self.active == Some(di) {
            self.omp_ui.bar = None;
        }
    }

    /// The ОМП result of the active document over its current В1/В2 interval
    /// (`0x49d330`, recomputed whenever a визир moves).
    pub fn omp_result(&self) -> Option<OmpResult> {
        let d = self.active_doc()?;
        let run = d.omp.run.as_ref()?;
        let (c1, c2) = (d.main_cursor.min(d.aux_cursor), d.main_cursor.max(d.aux_cursor));
        Some(match run.method {
            DOUBLE_ENDED => fvb_core::omp::result_double_for_window(&run.trace, &run.sections, c1, c2),
            _ => fvb_core::omp::result_for_window(&run.trace, &run.sections, c1, c2),
        })
    }

    /// 32807 «Параметры»: the modal dialog on a copy of the settings.
    pub fn omp_open_params(&mut self) {
        let Some(di) = self.active else { return };
        self.omp_ui.dialog = Some(ParamsDialog::new(di, self.docs[di].omp.params.clone()));
    }

    /// Re-bind the bar to the active document (`0x45f7d0` → `0x454310`) and
    /// refresh its text while the result is shown.
    fn omp_sync_bar(&mut self) {
        if self.omp_ui.bar_doc != self.active || self.active.is_none() {
            self.omp_ui.bar_doc = self.active;
            let shown = self.active_doc().is_some_and(|d| d.omp.shown);
            self.omp_ui.bar = if shown { self.omp_result().map(|r| r.text) } else { None };
        } else if self.active_doc().is_some_and(|d| d.omp.shown) {
            if let Some(r) = self.omp_result() {
                self.omp_ui.bar = Some(r.text);
            }
        }
    }

    /// The result bar under the ribbon: "i" icon, the text (leading spaces
    /// kept), right-aligned «Сохранить...» and the close button.
    pub fn omp_banner(&mut self, ui: &mut egui::Ui) {
        self.omp_sync_bar();
        let Some(text) = self.omp_ui.bar.clone() else {
            self.omp_ui.close_rect = None;
            return;
        };
        let pal = self.skin.palette();
        let mut save = false;
        let mut close = false;
        egui::Frame::none()
            .fill(Color32::from_rgb(255, 255, 225))
            .stroke(egui::Stroke::new(1.0_f32, pal.group_border))
            .inner_margin(egui::Margin::symmetric(6.0, 4.0))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                    ui.painter().circle_filled(r.center(), 7.5, Color32::from_rgb(40, 90, 200));
                    ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, "i", egui::FontId::proportional(12.0), Color32::WHITE);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let x = ui.add(egui::Button::new("✕").frame(false).small());
                        self.omp_ui.close_rect = Some(x.rect);
                        close = x.clicked();
                        save = ui.button("Сохранить...").on_hover_text("Сохранить рерультаты ОМП").clicked();
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.add(egui::Label::new(RichText::new(&text).color(Color32::BLACK)).truncate());
                        });
                    });
                });
            });
        if save {
            self.omp_save_report();
        }
        if close {
            // Hiding the bar posts 32830: the active document's result is reset.
            self.omp_ui.bar = None;
            if let Some(di) = self.active {
                self.omp_clear(di);
            }
        }
    }

    /// The report text of the active document's shown result.
    pub fn omp_report(&self) -> Option<String> {
        let d = self.active_doc()?;
        if !d.omp.shown {
            return None;
        }
        let r = self.omp_result()?;
        // «Осциллограмма:» is `doc+0x24` (`m_strPathName` on MFC 12's layout
        // [I]: the full path); the times are the record's raw date and time
        // tokens joined by ", " (`0x4048c0` / `0x496360`), not reformatted.
        let title = d.path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| d.name.clone());
        let st = &d.osc.stamps;
        let info = params::ReportInfo {
            title,
            station: d.osc.station.clone(),
            recorder: d.osc.device.clone(),
            start: format!("{}, {}", st.start_date, st.start_time),
            trigger: format!("{}, {}", st.trigger_date, st.trigger_time),
        };
        // `0x42edf0`: over the file and computed channels, ignoring case,
        // an exact match first; the line prints the found channel's own
        // name with its coefficients (`0x49db00`; computed ones 1 / 1).
        let ktr = |n: &str| {
            let chans = d.osc.analog.iter().map(|c| (c.id.as_str(), c.primary, c.secondary)).chain(d.computed.iter().map(|c| (c.name.as_str(), 1.0, 1.0)));
            let all: Vec<_> = chans.collect();
            all.iter().find(|c| c.0 == n).or_else(|| all.iter().find(|c| c.0.to_lowercase() == n.to_lowercase())).map(|c| (c.0.to_string(), c.1, c.2))
        };
        Some(params::report(&d.omp.params, &r, &info, ktr))
    }

    /// 32879 «Сохранить...»: only while the result is shown; filter 3444,
    /// default extension txt, no overwrite prompt; UTF-16LE with BOM, CRLF.
    pub fn omp_save_report(&mut self) {
        let Some(text) = self.omp_report() else { return };
        let Some(mut path) = rfd::FileDialog::new()
            .add_filter("Результаты ОМП (*.txt)", &["txt"])
            .add_filter("Все файлы (*.*)", &["*"])
            .save_file()
        else {
            return;
        };
        if path.extension().is_none() {
            path.set_extension("txt");
        }
        if let Err(e) = std::fs::write(&path, params::utf16le_crlf(&text)) {
            self.omp_ui.message = Some(e.to_string());
        }
    }

    /// The dialog's OK: validate station 1's sections (first failure → its
    /// message, the dialog stays), else store the working copy and run
    /// unless pasted channels are pending.
    pub fn omp_dialog_ok(&mut self) {
        let Some(dlg) = self.omp_ui.dialog.take() else { return };
        if let Err(id) = dlg.work.validate() {
            self.omp_ui.message = Some(params::message(id).to_string());
            self.omp_ui.dialog = Some(dlg);
            return;
        }
        let di = dlg.doc;
        if let Some(d) = self.docs.get_mut(di) {
            d.omp.params = dlg.work;
            // 32807 (`0x454240`): modified on every OK.
            d.set.modified = true;
            if !has_pending_paste(d) && self.active == Some(di) {
                self.run_omp();
            }
        }
    }

    /// The parameters dialog and the message box (both modal).
    pub fn omp_windows(&mut self, ctx: &egui::Context) {
        if let Some(mut dlg) = self.omp_ui.dialog.take() {
            let names: Vec<String> = self
                .docs
                .get(dlg.doc)
                .map(|d| d.osc.analog.iter().map(|c| c.id.clone()).chain(d.computed.iter().map(|c| c.name.clone())).collect())
                .unwrap_or_default();
            crate::dialogs::modal_backdrop(ctx, "omp_dialog_block", egui::Order::Middle);
            let mut outcome = None;
            crate::dialogs::dlg(ctx, "Параметры ОМП")
                .id(egui::Id::new("omp_params_dialog"))
                .order(egui::Order::Foreground)
                .collapsible(false)
                .resizable(false)
                .fixed_size([712.0, 447.0])
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| outcome = dlg.ui(ui, &names));
            match outcome {
                Some(DialogAction::Ok) => {
                    self.omp_ui.dialog = Some(dlg);
                    self.omp_dialog_ok();
                }
                Some(DialogAction::Cancel) => {}
                Some(DialogAction::Load) => {
                    // The original reports nothing: an unreadable file leaves
                    // the dialog as it was (`0x4a9010`); one that is no XML
                    // goes through the legacy reader (`0x4a8f10` →
                    // `0x4a45f0`, text in the display encoding).
                    if let Some(path) = omp_file_dialog().pick_file() {
                        if let Ok(b) = std::fs::read(&path) {
                            let legacy = || fvb_core::omp_params::apply_legacy(&fvb_core::comtrade::decode_bytes(&b, self.encoding), &dlg.work);
                            if let Some(p) = fvb_core::recset::omp_from_file_bytes(&b).or_else(legacy) {
                                dlg.load(p);
                            }
                        }
                    }
                    self.omp_ui.dialog = Some(dlg);
                }
                Some(DialogAction::Save) => {
                    if let Some(mut path) = omp_file_dialog().save_file() {
                        if path.extension().is_none() {
                            path.set_extension("omp");
                        }
                        if let Err(e) = std::fs::write(&path, fvb_core::recset::omp_file_bytes(&dlg.work)) {
                            self.omp_ui.message = Some(e.to_string());
                        }
                    }
                    self.omp_ui.dialog = Some(dlg);
                }
                None => self.omp_ui.dialog = Some(dlg),
            }
        }
        if let Some(msg) = self.omp_ui.message.clone() {
            crate::dialogs::modal_backdrop(ctx, "omp_message_block", egui::Order::Foreground);
            let mut ok = false;
            crate::dialogs::dlg(ctx, APP_TITLE)
                .id(egui::Id::new("omp_message"))
                .order(egui::Order::Tooltip)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    crate::dialogs::message_body(ui, crate::dialogs::MsgIcon::Exclamation, &msg);
                    ui.vertical_centered(|ui| ok = ui.button("   OK   ").clicked());
                });
            if ok || ctx.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Escape)) {
                self.omp_ui.message = None;
            }
        }
    }
}

fn omp_file_dialog() -> rfd::FileDialog {
    rfd::FileDialog::new().add_filter("Установки ОМП (*.omp)", &["omp"]).add_filter("Все файлы (*.*)", &["*"])
}

// ------------------------------------------------------------------ dialog

/// An element of the dialog's tree: a station, a section of a station, a
/// parallel line or a tap of a section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Node {
    Station(usize),
    Section(usize, usize),
    Line(usize, usize, usize),
    Tap(usize, usize, usize),
}

/// A property-grid row's value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Name,
    Inom,
    Ia,
    Ib,
    Ic,
    Uab,
    Ubc,
    Uca,
    Ua,
    Ub,
    Uc,
    Length,
    Supports,
    R,
    X,
    R1,
    X1,
    R0,
    X0,
    Comment,
    Rm,
    Xm,
    I0,
}

/// What the dialog asks its owner to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogAction {
    Ok,
    Cancel,
    Load,
    Save,
}

/// The working copy and the tree state of DIALOG 336.
pub struct ParamsDialog {
    pub doc: usize,
    pub work: OmpParams,
    pub sel: Node,
    /// Collapsed elements (every element starts expanded); the state is
    /// the element's (`+0x10`), so it travels with a moved one.
    pub collapsed: HashSet<Node>,
    /// The tree as last built: a delete (32874, `0x4a8eb0`) removes only
    /// its items and leaves the others' labels (`Участок №3` after №1 is
    /// gone) until the next rebuild — add, move, load or method change.
    pub labels: Option<Vec<(usize, Node, String)>>,
}

/// Where an element goes when `removed` is deleted: gone (itself or its
/// children) or shifted down among its siblings.
fn shift_after_delete(node: Node, removed: Node) -> Option<Node> {
    use Node::*;
    match (removed, node) {
        (Section(s, k), Section(s2, k2)) if s == s2 => (k2 != k).then(|| Section(s2, if k2 > k { k2 - 1 } else { k2 })),
        (Section(s, k), Line(s2, k2, n)) if s == s2 => (k2 != k).then(|| Line(s2, if k2 > k { k2 - 1 } else { k2 }, n)),
        (Section(s, k), Tap(s2, k2, n)) if s == s2 => (k2 != k).then(|| Tap(s2, if k2 > k { k2 - 1 } else { k2 }, n)),
        (Line(s, k, n), Line(s2, k2, n2)) if (s, k) == (s2, k2) => (n2 != n).then(|| Line(s2, k2, if n2 > n { n2 - 1 } else { n2 })),
        (Tap(s, k, n), Tap(s2, k2, n2)) if (s, k) == (s2, k2) => (n2 != n).then(|| Tap(s2, k2, if n2 > n { n2 - 1 } else { n2 })),
        _ => Some(node),
    }
}

/// The method combo (id 1075): index → method code.
const METHODS: [(u32, &str); 3] = [
    (ISOLATED, "ОМП для сетей с изолированной нейтралью"),
    (GROUNDED, "ОМП для сетей с заземленной нейтралью"),
    (DOUBLE_ENDED, "ОМП двухстороннего замера"),
];

/// The tree rows `(depth, element, label)`: methods 1/2 one root
/// «Подстанция», method 4 «Подстанция №1» / «№2»; under a station its
/// «Участок №k»; under a section (methods 2/4 only) «Параллельная линия №k»
/// then «Отпайка №k», numbered from 1 within the section.
pub fn tree_rows(p: &OmpParams) -> Vec<(usize, Node, String)> {
    let mut out = Vec::new();
    let n_st = if p.method == DOUBLE_ENDED { 2 } else { 1 };
    for st in 0..n_st {
        let label = if p.method == DOUBLE_ENDED { format!("Подстанция №{}", st + 1) } else { "Подстанция".to_string() };
        out.push((0, Node::Station(st), label));
        for (k, s) in p.stations[st].sections.iter().enumerate() {
            out.push((1, Node::Section(st, k), format!("Участок №{}", k + 1)));
            if p.method == ISOLATED {
                continue;
            }
            for n in 0..s.lines.len() {
                out.push((2, Node::Line(st, k, n), format!("Параллельная линия №{}", n + 1)));
            }
            for n in 0..s.taps.len() {
                out.push((2, Node::Tap(st, k, n), format!("Отпайка №{}", n + 1)));
            }
        }
    }
    out
}

/// The grid rows of an element under a method: (label, field, channel
/// drop-down?). Rows of other methods are hidden, their values kept.
pub fn grid_rows(method: u32, node: Node) -> Vec<(&'static str, Field, bool)> {
    let iso = method == ISOLATED;
    match node {
        Node::Station(_) => {
            let mut v = vec![
                ("Название", Field::Name, false),
                ("Номинальный ток прямой последовательности, A", Field::Inom, false),
                ("Ia", Field::Ia, true),
                ("Ib", Field::Ib, true),
                ("Ic", Field::Ic, true),
            ];
            if iso {
                v.extend([("Uab", Field::Uab, true), ("Ubc", Field::Ubc, true), ("Uca", Field::Uca, true)]);
            } else {
                v.extend([("Ua", Field::Ua, true), ("Ub", Field::Ub, true), ("Uc", Field::Uc, true)]);
            }
            v
        }
        Node::Section(..) => {
            let mut v = vec![("Длина участка", Field::Length, false), ("Число опор", Field::Supports, false)];
            if iso {
                v.extend([("R удельное (Ом/км)", Field::R, false), ("X удельное (Ом/км)", Field::X, false)]);
            } else {
                v.extend([
                    ("R1 удельное (Ом/км)", Field::R1, false),
                    ("X1 удельное (Ом/км)", Field::X1, false),
                    ("R0 удельное (Ом/км)", Field::R0, false),
                    ("X0 удельное (Ом/км)", Field::X0, false),
                ]);
            }
            v.push(("Комментарий", Field::Comment, false));
            v
        }
        Node::Line(..) => vec![
            ("Rm удельное(Ом/км)", Field::Rm, false),
            ("Xm удельное(Ом/км)", Field::Xm, false),
            ("I0", Field::I0, true),
            ("Комментарий", Field::Comment, false),
        ],
        Node::Tap(..) => {
            vec![("R0 (Ом)", Field::R0, false), ("X0 (Ом)", Field::X0, false), ("Комментарий", Field::Comment, false)]
        }
    }
}

/// The stored string behind a grid row.
pub fn field_mut(p: &mut OmpParams, node: Node, f: Field) -> Option<&mut String> {
    Some(match node {
        Node::Station(st) => {
            let s = p.stations.get_mut(st)?;
            match f {
                Field::Name => &mut s.name,
                Field::Inom => &mut s.inom,
                Field::Ia => &mut s.ia,
                Field::Ib => &mut s.ib,
                Field::Ic => &mut s.ic,
                Field::Uab => &mut s.uab,
                Field::Ubc => &mut s.ubc,
                Field::Uca => &mut s.uca,
                Field::Ua => &mut s.ua,
                Field::Ub => &mut s.ub,
                Field::Uc => &mut s.uc,
                _ => return None,
            }
        }
        Node::Section(st, k) => {
            let s = p.stations.get_mut(st)?.sections.get_mut(k)?;
            match f {
                Field::Length => &mut s.length,
                Field::Supports => &mut s.supports,
                Field::R => &mut s.r,
                Field::X => &mut s.x,
                Field::R1 => &mut s.r1,
                Field::X1 => &mut s.x1,
                Field::R0 => &mut s.r0,
                Field::X0 => &mut s.x0,
                Field::Comment => &mut s.comment,
                _ => return None,
            }
        }
        Node::Line(st, k, n) => {
            let l = p.stations.get_mut(st)?.sections.get_mut(k)?.lines.get_mut(n)?;
            match f {
                Field::Rm => &mut l.rm,
                Field::Xm => &mut l.xm,
                Field::I0 => &mut l.i0,
                Field::Comment => &mut l.comment,
                _ => return None,
            }
        }
        Node::Tap(st, k, n) => {
            let t = p.stations.get_mut(st)?.sections.get_mut(k)?.taps.get_mut(n)?;
            match f {
                Field::R0 => &mut t.r0,
                Field::X0 => &mut t.x0,
                Field::Comment => &mut t.comment,
                _ => return None,
            }
        }
    })
}

/// A row's default value (a changed value is shown marked, as the grid does).
fn default_value(node: Node, f: Field) -> &'static str {
    match (node, f) {
        (Node::Station(_), Field::Name) => "Подстанция",
        (Node::Station(_), Field::Inom) => "0",
        (Node::Section(..), Field::Length | Field::Supports) => "1",
        (_, Field::Comment) | (_, Field::I0) => "",
        (Node::Station(_), _) => "",
        _ => "0",
    }
}

impl ParamsDialog {
    pub fn new(doc: usize, work: OmpParams) -> Self {
        ParamsDialog { doc, work, sel: Node::Station(0), collapsed: HashSet::new(), labels: None }
    }

    /// Position and sibling count of the selection (stations do not move).
    fn sibling_pos(&self) -> Option<(usize, usize)> {
        let st = |s: usize| &self.work.stations[s];
        Some(match self.sel {
            Node::Station(_) => return None,
            Node::Section(s, k) => (k, st(s).sections.len()),
            Node::Line(s, k, n) => (n, st(s).sections.get(k)?.lines.len()),
            Node::Tap(s, k, n) => (n, st(s).sections.get(k)?.taps.len()),
        })
    }

    /// Участок (32871): only on station 1 (Подстанция №2 never has sections).
    pub fn can_add_section(&self) -> bool {
        self.sel == Node::Station(0)
    }

    /// Параллельная линия / Отпайка (32872 / 32873): on a section, methods 2/4.
    pub fn can_add_child(&self) -> bool {
        matches!(self.sel, Node::Section(..)) && self.work.method != ISOLATED
    }

    /// Удалить (32874): a section, line or tap.
    pub fn can_delete(&self) -> bool {
        !matches!(self.sel, Node::Station(_))
    }

    /// Вниз / Вверх (32876 / 32875): not the last / first among siblings.
    pub fn can_move(&self, up: bool) -> bool {
        self.sibling_pos().is_some_and(|(i, n)| if up { i > 0 } else { i + 1 < n })
    }

    /// Append a default section and select it.
    pub fn add_section(&mut self) {
        if let Node::Station(st) = self.sel {
            self.labels = None;
            let v = &mut self.work.stations[st].sections;
            v.push(SectionParams::default());
            self.sel = Node::Section(st, v.len() - 1);
        }
    }

    /// Append a default parallel line (`tap = false`) or tap to the selected
    /// section and select it (lines are listed before taps).
    pub fn add_child(&mut self, tap: bool) {
        let Node::Section(st, k) = self.sel else { return };
        self.labels = None;
        let s = &mut self.work.stations[st].sections[k];
        self.sel = if tap {
            s.taps.push(Default::default());
            Node::Tap(st, k, s.taps.len() - 1)
        } else {
            s.lines.push(Default::default());
            Node::Line(st, k, s.lines.len() - 1)
        };
    }

    /// The tree's rows as shown: the last build, or a fresh one.
    pub fn rows(&self) -> Vec<(usize, Node, String)> {
        self.labels.clone().unwrap_or_else(|| tree_rows(&self.work))
    }

    /// Delete the selection without confirmation; its parent is selected.
    /// The other items keep their labels (no renumbering until a rebuild).
    pub fn delete(&mut self) {
        let removed = self.sel;
        if !matches!(removed, Node::Station(_)) {
            let rows = self.rows();
            self.labels = Some(rows.into_iter().filter_map(|(d, n, l)| shift_after_delete(n, removed).map(|n| (d, n, l))).collect());
            self.collapsed = self.collapsed.iter().filter_map(|&n| shift_after_delete(n, removed)).collect();
        }
        let stations = &mut self.work.stations;
        self.sel = match self.sel {
            Node::Station(s) => Node::Station(s),
            Node::Section(s, k) => {
                stations[s].sections.remove(k);
                Node::Station(s)
            }
            Node::Line(s, k, n) => {
                stations[s].sections[k].lines.remove(n);
                Node::Section(s, k)
            }
            Node::Tap(s, k, n) => {
                stations[s].sections[k].taps.remove(n);
                Node::Section(s, k)
            }
        };
    }

    /// Swap the selection with its previous / next sibling and keep it selected.
    pub fn move_sel(&mut self, up: bool) {
        if !self.can_move(up) {
            return;
        }
        let j = |i: usize| if up { i - 1 } else { i + 1 };
        self.labels = None;
        // The expanded flag is swapped with the element (`0x4a6ae0`).
        if let Node::Section(s, k) = self.sel {
            let (a, b) = (Node::Section(s, k), Node::Section(s, j(k)));
            let (ca, cb) = (self.collapsed.remove(&a), self.collapsed.remove(&b));
            if ca {
                self.collapsed.insert(b);
            }
            if cb {
                self.collapsed.insert(a);
            }
        }
        let stations = &mut self.work.stations;
        self.sel = match self.sel {
            Node::Station(s) => Node::Station(s),
            Node::Section(s, k) => {
                stations[s].sections.swap(k, j(k));
                Node::Section(s, j(k))
            }
            Node::Line(s, k, n) => {
                stations[s].sections[k].lines.swap(n, j(n));
                Node::Line(s, k, j(n))
            }
            Node::Tap(s, k, n) => {
                stations[s].sections[k].taps.swap(n, j(n));
                Node::Tap(s, k, j(n))
            }
        };
    }

    /// A method change rebuilds the tree and grid; data of hidden kinds is
    /// kept, a hidden selection falls back to its visible ancestor.
    pub fn set_method(&mut self, m: u32) {
        self.work.method = m;
        self.labels = None;
        self.fix_selection();
    }

    /// Settings read from an `.omp` file replace the working copy.
    pub fn load(&mut self, p: OmpParams) {
        self.work = p;
        self.labels = None;
        self.fix_selection();
    }

    fn fix_selection(&mut self) {
        let visible = tree_rows(&self.work);
        if !visible.iter().any(|(_, n, _)| *n == self.sel) {
            self.sel = match self.sel {
                Node::Line(s, k, _) | Node::Tap(s, k, _) if visible.iter().any(|(_, n, _)| *n == Node::Section(s, k)) => {
                    Node::Section(s, k)
                }
                _ => Node::Station(0),
            };
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, channels: &[String]) -> Option<DialogAction> {
        let mut action = None;
        // DIALOG 336: 475 × 275 DLU in MS Shell Dlg 8 — a 712 × 447 client;
        // OnInitDialog `0x4a7c50` places the toolbar, the tree and the grid
        // (tb = the toolbar's height, ours 22); OK / Cancel stay at the
        // template's (310, 254) / (392, 254), 76 × 14 DLU. No separators.
        const W: f32 = 712.0;
        const H: f32 = 447.0;
        const TB: f32 = 22.0;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(W, H), egui::Sense::hover());
        let at = |x: f32, y: f32, w: f32, h: f32| egui::Rect::from_min_size(rect.min + egui::vec2(x, y), egui::vec2(w, h));
        // Toolbar 475 at (10, 10), W − 20 wide (`0x4a7cd9`).
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(at(10.0, 10.0, W - 20.0, TB)), |ui| ui.horizontal(|ui| {
            let cur = METHODS.iter().position(|(m, _)| *m == self.work.method).unwrap_or(0);
            let mut idx = cur;
            egui::ComboBox::from_id_salt("omp_method").width(280.0).selected_text(METHODS[cur].1).show_ui(ui, |ui| {
                for (i, (_, label)) in METHODS.iter().enumerate() {
                    ui.selectable_value(&mut idx, i, *label);
                }
            });
            if idx != cur {
                self.set_method(METHODS[idx].0);
            }
            // TOOLBAR 475 (`0x4a7e40`): no separator after the combo; text
            // only on the three add buttons, the rest icons; the tooltips
            // are the command strings 32870–32878.
            if ui.add_enabled(self.can_add_section(), egui::Button::new("Участок")).on_hover_text("Добавить участок к линии").clicked() {
                self.add_section();
            }
            if ui.add_enabled(self.can_add_child(), egui::Button::new("Параллельная линия")).on_hover_text("Добавить параллельную линию к участку").clicked() {
                self.add_child(false);
            }
            if ui.add_enabled(self.can_add_child(), egui::Button::new("Отпайка")).on_hover_text("Добавить отпайку от линии к участку").clicked() {
                self.add_child(true);
            }
            if ui.add_enabled(self.can_delete(), egui::Button::new("✖")).on_hover_text("Удалить выделенный элемент").clicked() {
                self.delete();
            }
            ui.separator();
            if ui.add_enabled(self.can_move(false), egui::Button::new("⬇")).on_hover_text("Переместить выделенный элемент вниз").clicked() {
                self.move_sel(false);
            }
            if ui.add_enabled(self.can_move(true), egui::Button::new("⬆")).on_hover_text("Переместить выделенный элемент вверх").clicked() {
                self.move_sel(true);
            }
            ui.separator();
            if ui.button("📂").on_hover_text("Загрузить настройки в отдельном файле").clicked() {
                action = Some(DialogAction::Load);
            }
            if ui.button("💾").on_hover_text("Сохранить настройки в отдельном файле").clicked() {
                action = Some(DialogAction::Save);
            }
        }));
        // The tree (1074, `0x4a7d1b`): (11, tb + 21), (W/3 − 22) × (H − 92);
        // buttons and lines but not at the root (style 0x50110003) — a
        // station has no ⊞ / ⊟ (a double-click toggles it); each item the
        // image of its kind (BITMAP 476: station, section, parallel line,
        // tap — our own drawings).
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(at(11.0, TB + 21.0, W / 3.0 - 22.0, H - 92.0)), |ui| {
            egui::Frame::none().stroke(egui::Stroke::new(1.0_f32, Color32::from_rgb(171, 173, 179))).fill(Color32::WHITE).show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                egui::ScrollArea::vertical().id_salt("omp_tree").show(ui, |ui| {
                    let mut hidden_below: Option<usize> = None;
                    let rows = self.rows();
                    for (i, (depth, node, label)) in rows.iter().enumerate() {
                        if let Some(d) = hidden_below {
                            if *depth > d {
                                continue;
                            }
                            hidden_below = None;
                        }
                        let has_children = rows.get(i + 1).is_some_and(|(d2, _, _)| d2 > depth);
                        let collapsed = self.collapsed.contains(node);
                        let toggle = |me: &mut Self| {
                            if me.collapsed.contains(node) {
                                me.collapsed.remove(node);
                            } else {
                                me.collapsed.insert(*node);
                            }
                        };
                        ui.horizontal(|ui| {
                            ui.add_space(4.0 + 16.0 * depth.saturating_sub(1) as f32);
                            if *depth > 0 {
                                if has_children {
                                    if ui.add(egui::Button::new(if collapsed { "⊞" } else { "⊟" }).frame(false).small()).clicked() {
                                        toggle(self);
                                    }
                                } else {
                                    ui.add_space(14.0);
                                }
                            }
                            let (ir, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                            tree_icon(ui.painter(), ir, node);
                            let r = ui.selectable_label(self.sel == *node, label.as_str());
                            if r.clicked() {
                                self.sel = *node;
                            }
                            if r.double_clicked() && has_children {
                                toggle(self);
                            }
                        });
                        if has_children && self.collapsed.contains(node) {
                            hidden_below = Some(*depth);
                        }
                    }
                });
            });
        });
        // The grid (`0x4a80e0`, WS_BORDER, no header): (W/3, tb + 20),
        // (2W/3 − 10) × (H − 90); modified values bold.
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(at(W / 3.0, TB + 20.0, 2.0 * W / 3.0 - 10.0, H - 90.0)), |ui| {
            egui::Frame::none().stroke(egui::Stroke::new(1.0_f32, Color32::from_rgb(171, 173, 179))).show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                egui::ScrollArea::vertical().id_salt("omp_grid").show(ui, |ui| {
                    egui::Grid::new("omp_grid_rows").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                        let node = self.sel;
                        for (label, f, is_chan) in grid_rows(self.work.method, node) {
                            let Some(v) = field_mut(&mut self.work, node, f) else { continue };
                            // Seen on the VM: the names left-aligned, a
                            // modified value (not its name) bold.
                            let modified = v.as_str() != default_value(node, f);
                            ui.allocate_ui_with_layout(egui::vec2(230.0, 18.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                ui.set_min_width(230.0);
                                ui.add(egui::Label::new(label).truncate());
                            });
                            if is_chan {
                                let shown = if modified { RichText::new(v.as_str()).strong() } else { RichText::new(v.as_str()) };
                                egui::ComboBox::from_id_salt(("omp_chan", label))
                                    .width(190.0)
                                    .selected_text(shown)
                                    .show_ui(ui, |ui| {
                                        for n in channels {
                                            ui.selectable_value(v, n.clone(), n.as_str());
                                        }
                                    });
                            } else {
                                ui.add(egui::TextEdit::singleline(v).desired_width(190.0));
                            }
                            ui.end_row();
                        }
                    });
                });
            });
        });
        if ui.put(at(465.0, 413.0, 114.0, 23.0), egui::Button::new("OK")).clicked() {
            action = Some(DialogAction::Ok);
        }
        if ui.put(at(588.0, 413.0, 114.0, 23.0), egui::Button::new("Отмена")).clicked() {
            action = Some(DialogAction::Cancel);
        }
        action
    }
}

/// A tree item's image (BITMAP 476's four cells, our own drawings on
/// white): a station's two rings, a section's conductor with its ends, a
/// parallel line's two conductors, a tap's branch.
fn tree_icon(p: &egui::Painter, r: egui::Rect, node: &Node) {
    let c = r.center();
    let v = |x: f32, y: f32| c + egui::vec2(x, y);
    let st = |w: f32, col| egui::Stroke::new(w, col);
    match node {
        // Two overlapping rings, a transformer's mark (seen on the VM).
        Node::Station(_) => {
            for x in [-2.5, 2.5] {
                p.circle_stroke(v(x, 0.0), 4.5, st(1.5, Color32::from_rgb(40, 40, 100)));
            }
        }
        Node::Section(..) => {
            p.line_segment([v(-5.0, 0.0), v(5.0, 0.0)], st(2.0, Color32::from_rgb(170, 160, 90)));
            for x in [-6.5, 6.5] {
                p.rect_filled(egui::Rect::from_center_size(v(x, 0.0), egui::vec2(3.0, 5.0)), 0.0, Color32::from_rgb(230, 120, 170));
            }
        }
        Node::Line(..) => {
            for x in [-2.5, 2.5] {
                p.line_segment([v(x, -6.0), v(x, 6.0)], st(1.5, Color32::from_rgb(90, 90, 90)));
            }
        }
        Node::Tap(..) => {
            p.line_segment([v(-2.0, -6.0), v(-2.0, 6.0)], st(1.5, Color32::from_rgb(90, 90, 90)));
            p.line_segment([v(-2.0, 0.0), v(5.0, 0.0)], st(1.5, Color32::from_rgb(90, 90, 90)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{ChanKey, Command};

    /// `0x42edf0`: ignoring case, the exact spelling preferred, else the
    /// first; nothing for an empty or unknown name.
    #[test]
    fn channel_lookup_ignores_case_and_prefers_the_exact_name() {
        let mut app = demo_app();
        let d = app.active_doc_mut().unwrap();
        d.osc.analog[0].id = "Ia".into();
        d.osc.analog[1].id = "IA".into();
        assert_eq!(find_chan(d, "IA"), Some(Ok(1)), "exact");
        assert_eq!(find_chan(d, "ia"), Some(Ok(0)), "the first ignoring case");
        d.osc.analog[0].id = "Ток А".into();
        assert_eq!(find_chan(d, "ТОК а"), Some(Ok(0)), "Cyrillic folds too");
        assert_eq!(find_chan(d, ""), None);
        assert_eq!(find_chan(d, "нет"), None);
    }

    fn demo_app() -> App {
        let mut app = App::new();
        app.dispatch(Command::Demo);
        app
    }

    /// Grounded settings naming the demo record's channels, one 50 km section.
    fn grounded(app: &mut App) {
        let p = &mut app.active_doc_mut().unwrap().omp.params;
        p.method = GROUNDED;
        let st = &mut p.stations[0];
        st.inom = "100".into();
        (st.ia, st.ib, st.ic) = ("Ia".into(), "Ib".into(), "Ic".into());
        (st.ua, st.ub, st.uc) = ("Ua".into(), "Ub".into(), "Uc".into());
        st.sections.push(SectionParams {
            length: "50".into(),
            r1: "0.1".into(),
            x1: "0.4".into(),
            r0: "0.3".into(),
            x0: "1.2".into(),
            ..Default::default()
        });
    }

    #[test]
    fn defaults_are_method_1_and_a_1km_1_support_section() {
        let p = OmpParams::default();
        assert_eq!(p.method, ISOLATED);
        let mut dlg = ParamsDialog::new(0, p);
        dlg.add_section();
        let s = &dlg.work.stations[0].sections[0];
        assert_eq!((s.length.as_str(), s.supports.as_str()), ("1", "1"));
        assert_eq!(dlg.sel, Node::Section(0, 0));
    }

    /// 32874 removes only its items: the rest keep their numbers until a
    /// rebuild; a collapsed section stays collapsed through a move.
    #[test]
    fn delete_keeps_the_labels_until_a_rebuild() {
        let mut dlg = ParamsDialog::new(0, OmpParams::default());
        for _ in 0..3 {
            dlg.sel = Node::Station(0);
            dlg.add_section();
        }
        let labels = |d: &ParamsDialog| d.rows().into_iter().map(|(_, _, l)| l).collect::<Vec<_>>();
        dlg.sel = Node::Section(0, 0);
        dlg.delete();
        assert_eq!(labels(&dlg), ["Подстанция", "Участок №2", "Участок №3"]);
        assert_eq!(dlg.rows()[1].1, Node::Section(0, 0), "the element moved up, its label did not");
        dlg.collapsed.insert(Node::Section(0, 0));
        dlg.sel = Node::Section(0, 0);
        dlg.move_sel(false);
        assert_eq!(labels(&dlg), ["Подстанция", "Участок №1", "Участок №2"], "a move rebuilds");
        assert!(dlg.collapsed.contains(&Node::Section(0, 1)) && !dlg.collapsed.contains(&Node::Section(0, 0)));
    }

    #[test]
    fn toolbar_enable_rules_and_tree() {
        let mut dlg = ParamsDialog::new(0, OmpParams::default());
        assert!(dlg.can_add_section() && !dlg.can_delete() && !dlg.can_add_child());
        dlg.add_section();
        assert!(!dlg.can_add_child(), "method 1 has no lines or taps");
        dlg.set_method(GROUNDED);
        assert!(dlg.can_add_child() && dlg.can_delete() && !dlg.can_move(true) && !dlg.can_move(false));
        dlg.add_child(true);
        dlg.sel = Node::Section(0, 0);
        dlg.add_child(false);
        let labels: Vec<String> = tree_rows(&dlg.work).into_iter().map(|(_, _, l)| l).collect();
        assert_eq!(labels, ["Подстанция", "Участок №1", "Параллельная линия №1", "Отпайка №1"]);
        dlg.set_method(DOUBLE_ENDED);
        dlg.sel = Node::Station(1);
        assert!(!dlg.can_add_section(), "station 2 never has sections");
        assert_eq!(tree_rows(&dlg.work)[0].2, "Подстанция №1");
        assert_eq!(tree_rows(&dlg.work).last().unwrap().2, "Подстанция №2");
        // Switching back to method 1 hides lines/taps but keeps them.
        dlg.sel = Node::Tap(0, 0, 0);
        dlg.set_method(ISOLATED);
        assert_eq!(dlg.sel, Node::Section(0, 0));
        assert_eq!(dlg.work.stations[0].sections[0].taps.len(), 1);
        assert_eq!(tree_rows(&dlg.work).len(), 2);
    }

    #[test]
    fn move_and_delete() {
        let mut dlg = ParamsDialog::new(0, OmpParams::default());
        dlg.add_section();
        dlg.work.stations[0].sections[0].comment = "first".into();
        dlg.sel = Node::Station(0);
        dlg.add_section();
        assert!(dlg.can_move(true) && !dlg.can_move(false));
        dlg.move_sel(true);
        assert_eq!(dlg.sel, Node::Section(0, 0));
        assert_eq!(dlg.work.stations[0].sections[1].comment, "first");
        dlg.delete();
        assert_eq!(dlg.sel, Node::Station(0));
        assert_eq!(dlg.work.stations[0].sections.len(), 1);
    }

    #[test]
    fn grid_rows_follow_the_method() {
        let names = |m, n| grid_rows(m, n).into_iter().map(|(l, _, _)| l).collect::<Vec<_>>();
        assert!(names(ISOLATED, Node::Station(0)).contains(&"Uab"));
        assert!(names(GROUNDED, Node::Station(0)).contains(&"Ua"));
        assert_eq!(names(ISOLATED, Node::Section(0, 0)).len(), 5);
        assert_eq!(names(DOUBLE_ENDED, Node::Section(0, 0)).len(), 7);
        let mut p = OmpParams::default();
        *field_mut(&mut p, Node::Station(0), Field::Inom).unwrap() = "5".into();
        assert_eq!(p.stations[0].inom, "5");
    }

    #[test]
    fn analysis_without_data_shows_message_349() {
        let mut app = demo_app();
        app.dispatch(Command::OmpAnalysis);
        assert_eq!(app.omp_ui.message.as_deref(), Some("Данные для определения места повреждения не введены."));
        assert!(!app.active_doc().unwrap().omp.shown);
        assert!(app.omp_ui.bar.is_none());
    }

    #[test]
    fn missing_channel_name_shows_message_354() {
        let mut app = demo_app();
        grounded(&mut app);
        app.active_doc_mut().unwrap().omp.params.stations[0].ub = "нет".into();
        app.run_omp();
        assert_eq!(app.omp_ui.message.as_deref(), Some("Имя канала задано неправильно."));
        // The isolated method reads Uab/Ubc/Uca channels by name.
        let mut app = demo_app();
        grounded(&mut app);
        app.active_doc_mut().unwrap().omp.params.method = ISOLATED;
        app.active_doc_mut().unwrap().omp.params.stations[0].sections[0].r = "0.1".into();
        app.run_omp();
        assert_eq!(app.omp_ui.message.as_deref(), Some("Имя канала задано неправильно."));
        app.omp_ui.message = None;
        app.append_computed(&[
            ("Uab".into(), "Ua-Ub".into()),
            ("Ubc".into(), "Ub-Uc".into()),
            ("Uca".into(), "Uc-Ua".into()),
        ])
        .unwrap();
        let st = &mut app.active_doc_mut().unwrap().omp.params.stations[0];
        (st.uab, st.ubc, st.uca) = ("Uab".into(), "Ubc".into(), "Uca".into());
        app.run_omp();
        assert!(app.omp_ui.message.is_none(), "{:?}", app.omp_ui.message);
        assert!(app.active_doc().unwrap().omp.shown);
    }

    /// The flag channels carry bit 13: the copy's CMDUI does not count them.
    #[test]
    fn flag_channels_are_not_copied() {
        let mut app = demo_app();
        grounded(&mut app);
        app.active_doc_mut().unwrap().selection.insert(ChanKey::Analog(0));
        app.dispatch(Command::OmpAnalysis);
        let d = app.active_doc_mut().unwrap();
        d.selection = std::collections::BTreeSet::from([ChanKey::Digital(0)]);
        assert!(!d.can_copy());
        assert!(crate::merge_ui::clip_record(d).channels.is_empty());
    }

    #[test]
    fn analysis_toggle_runs_and_clears() {
        let mut app = demo_app();
        grounded(&mut app);
        app.active_doc_mut().unwrap().selection.insert(ChanKey::Analog(0));
        app.dispatch(Command::OmpAnalysis);
        assert!(app.omp_ui.message.is_none(), "{:?}", app.omp_ui.message);
        let d = app.active_doc().unwrap();
        let p = d.osc.samples_per_cycle();
        assert_eq!(d.aux_cursor, d.main_cursor + p - 1, "В1 at the start, В2 one period later");
        assert!(d.omp.shown && d.v_shown[0] && d.v_shown[1]);
        let lkz = d.computed.iter().find(|c| c.name == L_KZ).unwrap();
        assert_eq!(lkz.unit, "m");
        let digital: Vec<&str> = d.osc.digital.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(&digital[..3], ["КЗ фазы А", "КЗ фазы B", "КЗ фазы C"], "above the record's own");
        assert!(d.computed.iter().all(|c| !c.name.starts_with("КЗ фазы")), "flags are digital channels");
        // ...made by a computation (bit 13): no rename.
        assert!(crate::chandlg::can_rename(d, ChanKey::Digital(0)).is_err());
        let text = app.omp_result().unwrap().text;
        assert_eq!(app.omp_ui.bar.as_deref(), Some(text.as_str()));
        let n_dig = d.osc.digital.len();
        // A second run replaces the channels in place.
        app.run_omp();
        assert_eq!(app.active_doc().unwrap().osc.digital.len(), n_dig);
        // Анализ again clears the result and deletes the channels.
        app.dispatch(Command::OmpAnalysis);
        let d = app.active_doc().unwrap();
        assert!(!d.omp.shown && d.omp.run.is_none() && app.omp_ui.bar.is_none());
        assert!(d.computed.iter().all(|c| c.name != L_KZ));
        assert!(d.osc.digital.iter().all(|c| !KZ_PHASES.contains(&c.id.as_str())));
        assert!(app.omp_result().is_none());
    }

    #[test]
    fn double_ended_reads_both_stations_from_the_same_document() {
        let mut app = demo_app();
        grounded(&mut app);
        let p = &mut app.active_doc_mut().unwrap().omp.params;
        p.method = DOUBLE_ENDED;
        p.stations[1] = p.stations[0].clone();
        p.stations[1].sections.clear();
        app.run_omp();
        assert!(app.omp_ui.message.is_none(), "{:?}", app.omp_ui.message);
        assert!(app.active_doc().unwrap().omp.shown);
    }

    #[test]
    fn paste_clears_the_result_and_blocks_analysis() {
        let mut app = demo_app();
        grounded(&mut app);
        app.run_omp();
        app.active_doc_mut().unwrap().selection.insert(ChanKey::Analog(0));
        app.dispatch(Command::Copy);
        app.dispatch(Command::Paste);
        let d = app.active_doc().unwrap();
        assert!(!d.omp.shown && has_pending_paste(d));
        app.dispatch(Command::OmpAnalysis);
        assert!(!app.active_doc().unwrap().omp.shown, "Анализ is disabled while pasted channels are pending");
    }

    /// Neither the ОМП run (`0x4a00b0`) nor paste (57637, `0x45ad02`)
    /// reaches SetModifiedFlag; «Объединить» does (L9.19).
    #[test]
    fn paste_and_a_run_leave_the_document_unmodified() {
        let mut app = demo_app();
        grounded(&mut app);
        app.track_settings();
        app.active_doc_mut().unwrap().set.modified = false;
        app.run_omp();
        app.track_settings();
        assert!(app.active_doc().unwrap().omp.shown);
        assert!(!app.active_doc().unwrap().set.modified, "a run does not mark");
        app.active_doc_mut().unwrap().selection.insert(ChanKey::Analog(0));
        app.dispatch(Command::Copy);
        app.dispatch(Command::Paste);
        app.track_settings();
        let d = app.active_doc().unwrap();
        assert!(has_pending_paste(d));
        assert!(!d.set.modified, "paste does not mark");
    }

    #[test]
    fn dialog_ok_validates_then_stores_and_runs() {
        let mut app = demo_app();
        grounded(&mut app);
        let good = app.active_doc().unwrap().omp.params.clone();
        app.active_doc_mut().unwrap().omp.params = OmpParams::default();
        app.dispatch(Command::OmpParams);
        let mut bad = good.clone();
        bad.stations[0].sections[0].length = "0".into();
        app.omp_ui.dialog.as_mut().unwrap().work = bad;
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| app.omp_windows(ctx));
        assert!(app.omp_ui.dialog.is_some());
        app.omp_dialog_ok();
        assert_eq!(app.omp_ui.message.as_deref(), Some("Длина участка не может быть равна нулю"));
        assert!(app.omp_ui.dialog.is_some(), "the dialog stays open");
        app.omp_ui.message = None;
        app.omp_ui.dialog.as_mut().unwrap().work = good.clone();
        app.omp_dialog_ok();
        assert!(app.omp_ui.dialog.is_none());
        let d = app.active_doc().unwrap();
        assert_eq!(d.omp.params, good);
        assert!(d.omp.shown, "OK runs the analysis");
    }

    /// The bar renders under a panel, follows the active document and its
    /// close button clears the result.
    #[test]
    fn banner_follows_the_document_and_close_clears() {
        let mut app = demo_app();
        grounded(&mut app);
        app.run_omp();
        let ctx = egui::Context::default();
        let frame = |app: &mut App, click: Option<egui::Pos2>| {
            let mut raw = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 600.0))),
                ..Default::default()
            };
            if let Some(p) = click {
                raw.events = vec![
                    egui::Event::PointerMoved(p),
                    egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() },
                    egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() },
                ];
            }
            let _ = ctx.run(raw, |ctx| {
                egui::TopBottomPanel::top("t").show(ctx, |ui| app.omp_banner(ui));
            });
        };
        frame(&mut app, None);
        assert!(app.omp_ui.bar.as_deref().is_some_and(|t| t.contains("КЗ")));
        // Another document hides the bar; coming back shows it again.
        app.dispatch(Command::Demo);
        frame(&mut app, None);
        assert!(app.omp_ui.bar.is_none());
        app.dispatch(Command::ActivateDoc(0));
        frame(&mut app, None);
        let close = app.omp_ui.close_rect.expect("close button drawn").center();
        frame(&mut app, Some(close));
        assert!(app.omp_ui.bar.is_none());
        assert!(!app.docs[0].omp.shown && app.docs[0].omp.run.is_none());
    }

    /// The dialog and the message box render in every method, and a click
    /// outside the modal dialog does not reach the window below.
    #[test]
    fn dialog_renders_modally_in_every_method() {
        let mut app = demo_app();
        grounded(&mut app);
        app.dispatch(Command::OmpParams);
        {
            let dlg = app.omp_ui.dialog.as_mut().unwrap();
            dlg.sel = Node::Section(0, 0);
            dlg.add_child(false);
            dlg.sel = Node::Section(0, 0);
            dlg.add_child(true);
        }
        let ctx = egui::Context::default();
        let frame = |app: &mut App, click: Option<egui::Pos2>| {
            let mut raw = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1320.0, 860.0))),
                ..Default::default()
            };
            if let Some(p) = click {
                raw.events = vec![
                    egui::Event::PointerMoved(p),
                    egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() },
                    egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() },
                ];
            }
            let mut below = false;
            let _ = ctx.run(raw, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    below |= ui.allocate_rect(ui.max_rect(), egui::Sense::click()).clicked();
                });
                app.omp_windows(ctx);
            });
            below
        };
        for m in [ISOLATED, GROUNDED, DOUBLE_ENDED] {
            app.omp_ui.dialog.as_mut().unwrap().set_method(m);
            for _ in 0..2 {
                frame(&mut app, None);
            }
        }
        assert!(!frame(&mut app, Some(egui::pos2(5.0, 5.0))), "the dialog is modal");
        assert!(app.omp_ui.dialog.is_some());
        app.omp_ui.message = Some(params::message(3187).into());
        frame(&mut app, None);
        frame(&mut app, None);
    }

    #[test]
    fn report_of_the_shown_result() {
        let mut app = demo_app();
        assert!(app.omp_report().is_none());
        grounded(&mut app);
        app.run_omp();
        let t = app.omp_report().unwrap();
        assert!(t.starts_with("Механотроника FastView\n\nОсциллограмма: demo_fault\n\nАлгоритм: ОМП для сетей с заземленной нейтралью\n\n"));
        // The record's raw stamp tokens joined by ", " (`0x4048c0`).
        assert!(t.contains("\nНачало записи:  01/01/2024, 00:00:00.000000\nМомент запуска: 01/01/2024, 00:00:00.030000\n\n"), "{t}");
        let d = app.active_doc().unwrap();
        let ia = d.osc.analog.iter().find(|c| c.id == "Ia").unwrap();
        assert!(t.contains(&format!("Канал Ia:  Ia         Ктр = {}/", fvb_core::views::fmt_g(ia.primary).replace("e+0", "e+00"))), "{t}");
    }
}
