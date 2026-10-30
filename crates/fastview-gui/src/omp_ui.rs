//! ОМП (fault location) user interface, as the original presents it
//! (`docs/original/specs/ui-behaviour.md` §8): the ribbon's «Анализ» is a toggle
//! (32806, `0x449890`), the result is a message bar under the ribbon
//! (CMsgCaptionBar 221) whose text follows В1/В2 live and whose «Сохранить...»
//! (32879, `0x4a1930`) writes the `.txt` report; hiding the bar clears the
//! result (32830, `0x475210` → `0x4a17a0`). «Параметры» (32807, `0x454240`)
//! is the modal DIALOG 336 «Параметры ОМП» (WrongPlaceLocatorDlg): toolbar,
//! element tree and property grid over a working copy of the document's
//! settings; OK validates, stores and runs the analysis. The engine is
//! `fvb_core::omp`, the settings model `fvb_core::omp_params`.

use crate::app::{App, Computed, Doc};
use eframe::egui::{self, Color32};
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
            (i < d.flagged.analog.len()).then(|| d.shown_samples(i).to_vec())
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
        d.v1_placed = true;
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
    pub(crate) fn omp_sync_bar(&mut self) {
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

    /// The result bar (CMsgCaptionBar 221) in its own band under the
    /// ribbon, 33 px (VM, Office 2007): the control-bar face — a gradient
    /// from white between (157, 189, 216) lines; the older looks' INFOBK —
    /// from 4 px in to 22 px short of the window's right (the close button
    /// in that margin), framed; bitmap 230's orange «i» centred 13
    /// px in, the text (its four leading spaces kept) from 25, a flat
    /// «Сохранить...» 93 × 22 at the face's right.
    pub fn omp_banner(&mut self, ui: &mut egui::Ui) {
        // (`omp_bar_h`: 33 px at 100 %)
        use crate::oscgeom::PixelLines;
        self.omp_sync_bar();
        let Some(text) = self.omp_ui.bar.clone() else {
            self.omp_ui.close_rect = None;
            return;
        };
        let pal = self.skin.palette();
        let full = egui::Rect::from_min_size(ui.max_rect().min, egui::vec2(ui.available_width(), omp_bar_h()));
        ui.allocate_rect(full, egui::Sense::hover());
        // 4 px of the brush at the left, the face framed on three sides.
        let face = egui::Rect::from_min_max(full.min + egui::vec2(4.0, 0.0), egui::pos2(full.right() - 21.0, full.bottom()));
        let p = ui.painter().clone();
        match pal.bands.filter(|_| pal.info_bar.is_none()) {
            Some(b) => {
                crate::lanes::gradient(&p, egui::Rect::from_min_max(face.min + egui::vec2(0.0, 1.0), face.max - egui::vec2(0.0, 1.0)), Color32::WHITE, b.bar_bottom);
                let edge = egui::Stroke::new(1.0f32, b.bar_line);
                p.px_rect_stroke(face, 0.0, edge);
            }
            None => {
                p.rect_filled(face, 0.0, pal.info_bar.unwrap_or(Color32::from_rgb(255, 255, 225)));
                p.rect_stroke(face, 0.0, egui::Stroke::new(1.0_f32, pal.group_border));
            }
        }
        let mid = face.center().y;
        // Bitmap 230 (VM): a light orange disc, darker below, a brown «i» —
        // stretched with the images at > 100 %.
        let k = crate::dpi::img(16.0) / 16.0;
        let ic = egui::pos2(face.left() + 13.0 * k, mid);
        p.circle_filled(ic, 6.5 * k, Color32::from_rgb(254, 200, 153));
        p.circle_filled(ic + egui::vec2(0.0, k), 5.5 * k, Color32::from_rgb(253, 175, 120));
        p.text(ic, egui::Align2::CENTER_CENTER, "i", egui::FontId::proportional(12.0 * k), Color32::from_rgb(120, 60, 20));
        // Its text and button follow the system font (as the parameters
        // bar's), the bar's height with them (`omp_bar_h`).
        use crate::dpi::s;
        let br = egui::Rect::from_min_size(egui::pos2(face.right() - s(93.0) - 4.0, mid - (s(22.0) / 2.0).floor()), egui::vec2(s(93.0), s(22.0)));
        p.with_clip_rect(egui::Rect::from_min_max(face.min, egui::pos2(br.left() - 4.0, face.bottom()))).text(egui::pos2(face.left() + 25.0 * k, mid), egui::Align2::LEFT_CENTER, &text, egui::FontId::proportional(crate::dpi::font(11.0)), Color32::from_rgb(30, 57, 91));
        let bresp = ui.interact(br, ui.id().with("fv_omp_save"), egui::Sense::click());
        // Its tip (CMFCCaptionBar's, VM): the plain Office box 18 px under
        // the pointer, 8 px inside the screen's right edge.
        crate::tips::pointer_tip_inset(ui, &bresp, crate::tips::TipImage::None, "", "Сохранить рерультаты ОМП", false, 8.0);
        if bresp.hovered() {
            crate::popmenu::hot_bar(&p, br);
        } else {
            p.rect_filled(br, 0.0, Color32::WHITE);
            p.px_rect_stroke(br, 0.0, egui::Stroke::new(1.0f32, Color32::from_rgb(157, 189, 216)));
        }
        p.text(br.center(), egui::Align2::CENTER_CENTER, "Сохранить...", egui::FontId::proportional(crate::dpi::font(12.0)), Color32::from_rgb(30, 57, 91));
        let save = bresp.clicked();
        let close = crate::dialogs::caption_close(ui, full, ui.id().with("fv_omp_close"));
        self.omp_ui.close_rect = Some(egui::Rect::from_min_size(egui::pos2(full.right() - crate::dpi::s(20.0), full.top()), egui::vec2(crate::dpi::s(17.0), crate::dpi::s(17.0))));

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
        let Some(mut path) = crate::dialogs::file_dialog()
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
            // The dialog's own Enter (OK, the default button) and Esc
            // (Отмена) — not while a grid value is edited (its edit takes
            // them) or the message box is up (VM).
            let keys_free = dlg.edit.is_none() && self.omp_ui.message.is_none();
            let mut outcome = None;
            crate::dialogs::dlg(ctx, "Параметры ОМП")
                .id(egui::Id::new("omp_params_dialog"))
                .order(egui::Order::Foreground)
                .collapsible(false)
                .resizable(false)
                .margin(0.0)
                .show(ctx, |ui| outcome = dlg.ui(ui, &names));
            if outcome.is_none() && keys_free {
                let (enter, esc) = ctx.input_mut(|i| (i.consume_key(egui::Modifiers::NONE, egui::Key::Enter), i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)));
                if enter {
                    outcome = Some(DialogAction::Ok);
                } else if esc {
                    outcome = Some(DialogAction::Cancel);
                }
            }
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
                .margin(0.0)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ok = crate::dialogs::message_box(ui, crate::dialogs::MsgIcon::Exclamation, &msg, &["OK"]).is_some();
                });
            if ok || ctx.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Escape)) {
                self.omp_ui.message = None;
            }
        }
    }
}

fn omp_file_dialog() -> rfd::FileDialog {
    crate::dialogs::file_dialog().add_filter("Установки ОМП (*.omp)", &["omp"]).add_filter("Все файлы (*.*)", &["*"])
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
    /// The grid cell being edited in place: the element and its row.
    pub edit: Option<(Node, usize)>,
    /// The grid's values as it was last built (CMFCPropertyGridProperty's
    /// original values): a value differing from them is bold. Built anew
    /// for another element, a load, a method change or a tree rebuild (VM:
    /// an edited value bold, regular again after leaving the element and
    /// coming back; loaded values regular).
    built: Option<(Node, Vec<String>)>,
    /// This opening's own ids (each opening starts with the tree focused).
    salt: u64,
    /// The grid's selected row, kept after its edit closes (VM: the name
    /// cell stays filled after Enter) until the grid is built again.
    row_sel: Option<(Node, usize)>,
    /// The grid holds the focus: its selected row's name (180, 201, 233),
    /// without it (240) (VM: after OK is pressed).
    grid_focus: bool,
    /// The value a grid edit started from: Esc puts it back (VM).
    edit_orig: String,
    /// Where the click that started the edit fell (x): the caret goes there.
    edit_click: Option<f32>,
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

impl ParamsDialog {
    pub fn new(doc: usize, work: OmpParams) -> Self {
        static OPENED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let salt = OPENED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        ParamsDialog { doc, work, sel: Node::Station(0), collapsed: HashSet::new(), labels: None, edit: None, built: None, salt, row_sel: None, grid_focus: false, edit_orig: String::new(), edit_click: None }
    }

    /// The grid's rows for `node` as last built, building them anew for
    /// another element.
    fn built_values(&mut self, node: Node) -> Vec<String> {
        if self.built.as_ref().is_none_or(|(n, _)| *n != node) {
            let values = grid_rows(self.work.method, node).iter().map(|(_, f, _)| field_mut(&mut self.work, node, *f).map(|v| v.clone()).unwrap_or_default()).collect();
            self.built = Some((node, values));
            self.row_sel = None;
        }
        self.built.as_ref().map(|(_, v)| v.clone()).unwrap_or_default()
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
            self.built = None;
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
        self.built = None;
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
        self.built = None;
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
        self.built = None;
        self.fix_selection();
    }

    /// Settings read from an `.omp` file replace the working copy.
    pub fn load(&mut self, p: OmpParams) {
        self.work = p;
        self.labels = None;
        self.built = None;
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
        use crate::dialogs::{win_button, win_combo_list, dlg_font};
        let mut action = None;
        // DIALOG 336: 475 × 275 DLU in MS Shell Dlg 8 — a 713 × 447 client;
        // OnInitDialog `0x4a7c50` places the method combo and the toolbar,
        // the tree and the grid (VM: the combo at (13, 12), 278 × 21; the
        // tree at (10, 44), W/3 − 20 × H − 90; the grid at (W/3, 44),
        // 2W/3 − 10 wide, the thirds whole pixels); OK / Cancel stay at the
        // template's (310, 254) / (392, 254), 76 × 14 DLU.
        // At > 100 % the client in the larger font's dialog units; the tree
        // and the grid from it as OnInitDialog sizes them, in pixels: under
        // the toolbar (10, 10, W − 20, its height) by 21, H − 90 tall.
        let (rect, _) = ui.allocate_exact_size(crate::dialogs::dlu_size(475, 275), egui::Sense::hover());
        // The toolbar's height follows its combo's (VM: 23 / 29 / 34 px, the
        // combo 21 / 27 / 31 at 100 / 125 / 150 %); the combo keeps its 278
        // px, the buttons follow it, their offsets from the combo's end and
        // the text buttons' widths at the system DPI, the icon-only ones the
        // image + 6 (VM, 150 %: «Участок», «Параллельная линия», «Отпайка»
        // from 294, 397, 602).
        let tool_h = crate::dpi::measured(23.0, 29.0, 34.0);
        let combo_h = crate::dpi::measured(21.0, 27.0, 31.0);
        let btn_h = crate::dpi::img(16.0) + 6.0;
        let (tree, grid) = params_layout(rect);
        let tool_x = |x: f32| 292.0 + crate::dpi::s(x - 292.0);
        let tool_y = 10.0 + tool_h - btn_h + 1.0;
        let px = |x: f32, y: f32, w: f32, h: f32| egui::Rect::from_min_size(rect.min + egui::vec2(x, y), egui::vec2(w, h));
        let cur = METHODS.iter().position(|(m, _)| *m == self.work.method).unwrap_or(0);
        let methods: Vec<String> = METHODS.iter().map(|(_, l)| l.to_string()).collect();
        // The method combo is the toolbar's, no tab stop of the dialog; the
        // tree comes first in the tab order, then OK and Отмена.
        if let Some(k) = crate::dialogs::no_tab_stop(ui, |ui| win_combo_list(ui, px(13.0, 10.0 + tool_h - combo_h, 278.0, combo_h), egui::Id::new("omp_method"), &methods, cur, true)) {
            if k != cur {
                self.set_method(METHODS[k].0);
            }
        }
        // TOOLBAR 475 (`0x4a7e40`), flat, right of the combo (VM): text only
        // on the three add buttons — (0, 21, 110), (141) disabled — the rest
        // icons, two separators; the tooltips are the command strings
        // 32871–32878 («description\ntitle»); the icons our own drawings.
        let items: [ToolItem; 8] = [
            (292.0, 67.0, ToolIcon::Section, Some("Участок"), "Добавить участок к линии\nДобавить участок", self.can_add_section()),
            (362.0, 132.0, ToolIcon::Line, Some("Параллельная линия"), "Добавить параллельную линию к участку\nДобавить параллельную линию", self.can_add_child()),
            (498.0, 67.0, ToolIcon::Tap, Some("Отпайка"), "Добавить отпайку от линии к участку\nДобавить отпайку", self.can_add_child()),
            (567.0, 22.0, ToolIcon::Delete, None, "Удалить выделенный элемент\nУдалить элемент", self.can_delete()),
            (597.0, 22.0, ToolIcon::Down, None, "Переместить выделенный элемент вниз\nПереместить вниз", self.can_move(false)),
            (619.0, 22.0, ToolIcon::Up, None, "Переместить выделенный элемент вверх\nПереместить вверх", self.can_move(true)),
            (650.0, 22.0, ToolIcon::Open, None, "Загрузить настройки в отдельном файле\nЗагрузить настройки", true),
            (672.0, 22.0, ToolIcon::Save, None, "Сохранить настройки в отдельном файле\nСохранить настройки", true),
        ];
        for (k, (x, w, icon, text, tip, enabled)) in items.into_iter().enumerate() {
            // Its tip CMFCToolBar's: the prompt's title and description.
            let w = if text.is_some() { crate::dpi::s(w) } else { btn_h };
            let r = tool_button(ui, px(tool_x(x), tool_y, w, btn_h), icon, text, enabled);
            let (desc, title) = tip.split_once('\n').unwrap_or((tip, tip));
            crate::tips::pointer_tip(ui, &r, crate::tips::TipImage::OmpTool(icon), title, desc, true);
            if r.clicked() && enabled {
                match k {
                    0 => self.add_section(),
                    1 => self.add_child(false),
                    2 => self.add_child(true),
                    3 => self.delete(),
                    4 => self.move_sel(false),
                    5 => self.move_sel(true),
                    6 => action = Some(DialogAction::Load),
                    _ => action = Some(DialogAction::Save),
                }
            }
        }
        for x in [593.0, 645.0] {
            let r = px(tool_x(x), tool_y - 1.0, 1.0, btn_h);
            ui.painter().rect_filled(r, 0.0, Color32::from_gray(197));
            ui.painter().rect_filled(r.translate(egui::vec2(1.0, 0.0)), 0.0, Color32::WHITE);
        }
        // The tree (1074, `0x4a7d1b`): buttons and lines but not at the
        // root (style 0x50110003) — a station has no ⊞ / ⊟ (a double-click
        // toggles it); 16 px rows, each the image of its kind (BITMAP 476:
        // our own drawings) at +3, the label from +22, the selection white
        // on (0, 120, 212) (VM).
        let all = self.rows();
        let mut shown = Vec::new();
        let mut hidden_below: Option<usize> = None;
        for (k, (depth, node, _)) in all.iter().enumerate() {
            if let Some(d) = hidden_below {
                if *depth > d {
                    continue;
                }
                hidden_below = None;
            }
            if self.collapsed.contains(node) {
                hidden_below = Some(*depth);
            }
            shown.push(k);
        }
        let rows: Vec<crate::dialogs::TreeRow> = shown
            .iter()
            .map(|&k| {
                let (depth, node, label) = &all[k];
                let has_children = all.get(k + 1).is_some_and(|(d2, _, _)| d2 > depth);
                crate::dialogs::TreeRow { depth: *depth, label: label.clone(), has_children, expanded: !self.collapsed.contains(node), selected: self.sel == *node }
            })
            .collect();
        let ev = crate::dialogs::win_tree(ui, tree, egui::Id::new(("omp_tree", self.salt)), Color32::from_rgb(171, 173, 179), &rows, false, None, true, false, &mut |p, cell, k| tree_icon(p, cell, &all[shown[k]].1));
        if let Some(k) = ev.toggled.or(ev.double_clicked.filter(|&k| rows[k].has_children)) {
            let node = all[shown[k]].1;
            if !self.collapsed.remove(&node) {
                self.collapsed.insert(node);
            }
        }
        if let Some(k) = ev.clicked {
            self.sel = all[shown[k]].1;
        }
        // The grid (`0x4a80e0`, WS_BORDER, no header) (VM): 17 px rows ruled
        // in (240) below each, the names and the values 5 px into their
        // columns, the value column from 231 px; a modified value bold. A
        // click on a value edits it in place: an edit, or the channel list.
        ui.painter().rect_filled(grid, 0.0, Color32::from_rgb(171, 173, 179));
        let inner = grid.shrink(1.0);
        ui.painter().rect_filled(inner, 0.0, Color32::WHITE);
        let split = inner.left() + crate::dpi::dx(231.0);
        // The rows from the font's height (17 px at 100 %).
        let row_h = crate::dpi::base_units().1 as f32 + 4.0;
        let rule = egui::Stroke::new(1.0f32, Color32::from_gray(240));
        ui.painter().vline(split - 0.5, inner.y_range(), rule);
        let node = self.sel;
        if self.edit.is_some_and(|(n, _)| n != node) {
            self.edit = None;
        }
        // A press on the grid gives it the focus, one elsewhere (but on the
        // toolbar) takes it.
        if let Some(p) = ui.input(|i| i.pointer.any_pressed().then(|| crate::lanes::press_pos(i)).flatten()) {
            if !crate::dialogs::keeps_focus_at(ui.ctx(), p) {
                self.grid_focus = grid.contains(p);
            }
        }
        let rows = grid_rows(self.work.method, node);
        let built = self.built_values(node);
        let mut y = inner.top();
        for (k, (label, f, is_chan)) in rows.into_iter().enumerate() {
            let Some(v) = field_mut(&mut self.work, node, f) else { continue };
            let row = egui::Rect::from_min_max(egui::pos2(inner.left(), y), egui::pos2(inner.right(), y + row_h));
            y += row_h;
            ui.painter().hline(inner.left() + 1.0..=inner.right() - 1.0, row.bottom() - 0.5, rule);
            let name = egui::Rect::from_min_max(row.min, egui::pos2(split - 1.0, row.bottom() - 1.0));
            let cell = egui::Rect::from_min_max(egui::pos2(split, row.top()), egui::pos2(row.right(), row.bottom() - 1.0));
            // Seen on the VM: the names left-aligned, a value changed since
            // the grid was built (not its name) bold.
            let modified = built.get(k).is_some_and(|b| b != v);
            let font = dlg_font();
            ui.painter().with_clip_rect(name).text(egui::pos2(name.left() + 5.0, name.center().y), egui::Align2::LEFT_CENTER, ellipsize_to(ui, label, name.width() - 7.0), font.clone(), Color32::BLACK);
            // A click selects the row (CMFCPropertyGridCtrl, VM): its name
            // on (180, 201, 233) and its value in place — a number in a
            // frameless edit at once, a channel with a drop button (14 px,
            // (226, 238, 255) → (196, 221, 255), a (86, 125, 177) arrow)
            // opening the list.
            let editing = self.edit == Some((node, k));
            if editing || self.row_sel == Some((node, k)) {
                let fill = if (editing || self.grid_focus) && !crate::dialogs::under_message(ui.ctx()) { Color32::from_rgb(180, 201, 233) } else { Color32::from_gray(240) };
                ui.painter().rect_filled(egui::Rect::from_min_max(egui::pos2(name.left() + 1.0, name.top()), name.max), 0.0, fill);
                ui.painter().with_clip_rect(name).text(egui::pos2(name.left() + 5.0, name.center().y), egui::Align2::LEFT_CENTER, ellipsize_to(ui, label, name.width() - 7.0), font.clone(), Color32::BLACK);
            }
            if editing && !is_chan {
                let id = egui::Id::new(("omp_cell", k));
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(cell));
                let mut out = egui::TextEdit::singleline(v).id(id).frame(false).font(font.clone()).margin(egui::Margin { left: 5.0, right: 2.0, top: 2.0, bottom: 1.0 }).desired_width(cell.width() - 7.0).show(&mut child);
                let resp = out.response.clone();
                if !resp.has_focus() && !resp.lost_focus() {
                    // The grid hands the click on to the new edit: the caret
                    // where it fell (VM: «5» typed before the «1» clicked
                    // at its left).
                    let n = v.chars().count();
                    let at = self.edit_click.take().map(|x| out.galley.cursor_from_pos(egui::vec2(x - out.galley_pos.x, 0.0)).ccursor).unwrap_or(egui::text::CCursor::new(n));
                    out.state.cursor.set_char_range(Some(egui::text::CCursorRange::one(at)));
                    out.state.store(ui.ctx(), id);
                    resp.request_focus();
                }
                if resp.lost_focus() {
                    // Enter keeps the value, Esc puts the old one back; the
                    // dialog stays either way (VM: «5», Esc → «1»).
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        *v = self.edit_orig.clone();
                    }
                    self.edit = None;
                }
                continue;
            }
            if editing {
                let btn = egui::Rect::from_min_max(egui::pos2(inner.right() - crate::dpi::s(14.0), cell.top()), egui::pos2(inner.right(), cell.bottom()));
                let mut mesh = egui::Mesh::default();
                for (pt, c) in [(btn.left_top(), (226, 238, 255)), (btn.right_top(), (226, 238, 255)), (btn.right_bottom(), (196, 221, 255)), (btn.left_bottom(), (196, 221, 255))] {
                    mesh.colored_vertex(pt, Color32::from_rgb(c.0, c.1, c.2));
                }
                mesh.add_triangle(0, 1, 2);
                mesh.add_triangle(0, 2, 3);
                ui.painter().add(mesh);
                let c = btn.center() + egui::vec2(0.0, -1.0);
                ui.painter().add(egui::Shape::convex_polygon(vec![c + egui::vec2(-2.5, -1.0), c + egui::vec2(2.5, -1.0), c + egui::vec2(0.0, 1.5)], Color32::from_rgb(86, 125, 177), egui::Stroke::NONE));
                let list_id = egui::Id::new(("omp_chan", k));
                let open_id = list_id.with("open");
                if ui.interact(btn, ui.id().with(("omp_drop", k)), egui::Sense::click()).clicked() {
                    let open = !ui.data(|d| d.get_temp::<bool>(open_id).unwrap_or(false));
                    ui.data_mut(|d| d.insert_temp(open_id, open));
                }
                if ui.data(|d| d.get_temp::<bool>(open_id).unwrap_or(false)) {
                    let cur = channels.iter().position(|c| c == v);
                    if let Some(pick) = crate::dialogs::combo_popup(ui, cell, list_id, channels, cur) {
                        *v = channels[pick].clone();
                    }
                } else {
                    // The in-place list's Enter / Esc end the edit (Esc
                    // restoring) without reaching the dialog.
                    let (enter, esc) = ui.input_mut(|i| (i.consume_key(egui::Modifiers::NONE, egui::Key::Enter), i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)));
                    if esc {
                        *v = self.edit_orig.clone();
                    }
                    if enter || esc {
                        self.edit = None;
                    }
                }
            }
            let p = ui.painter().with_clip_rect(cell);
            let at_text = egui::pos2(cell.left() + 5.0, cell.center().y);
            if modified {
                // The grid's bold font (Tahoma Bold).
                let h = ui.fonts(|f| f.row_height(&font));
                crate::lanes::bold_text(&p, at_text - egui::vec2(0.0, h / 2.0), v.as_str(), font.clone(), Color32::BLACK);
            } else {
                p.text(at_text, egui::Align2::LEFT_CENTER, v.as_str(), font.clone(), Color32::BLACK);
            }
            if !editing && ui.interact(egui::Rect::from_min_max(row.min, egui::pos2(row.right(), row.bottom() - 1.0)), ui.id().with(("omp_cell_click", k)), egui::Sense::click()).clicked() {
                self.edit_orig = v.clone();
                self.edit_click = ui.input(|i| i.pointer.interact_pos()).filter(|p| cell.contains(*p)).map(|p| p.x);
                self.edit = Some((node, k));
                self.row_sel = Some((node, k));
            }
        }
        if win_button(ui, crate::dialogs::dlu(rect, 310, 254, 76, 14), "OK", true).clicked() {
            action = Some(DialogAction::Ok);
        }
        if win_button(ui, crate::dialogs::dlu(rect, 392, 254, 76, 14), "Отмена", false).clicked() {
            action = Some(DialogAction::Cancel);
        }
        action
    }
}

/// A grid name cut to `width` with «...» (DT_END_ELLIPSIS), whole when it
/// fits.
fn ellipsize_to(ui: &egui::Ui, text: &str, width: f32) -> String {
    let font = crate::dialogs::dlg_font();
    if ui.fonts(|f| f.layout_no_wrap(text.to_owned(), font.clone(), Color32::BLACK).size().x) <= width {
        return text.to_owned();
    }
    crate::dialogs::ellipsize(ui, text, font, width)
}

/// A toolbar button: x and width in the client, image, text, tooltip,
/// enabled.
type ToolItem<'a> = (f32, f32, ToolIcon, Option<&'a str>, &'a str, bool);

/// TOOLBAR 475's images (our own drawings).
#[derive(Clone, Copy)]
pub(crate) enum ToolIcon {
    Section,
    Line,
    Tap,
    Delete,
    Down,
    Up,
    Open,
    Save,
}

/// A flat toolbar button (VM): the 16 px image 3 px in, the text after it
/// in (0, 21, 110) — (141) and a pale image when disabled; under the
/// pointer the Office 2007 hot face (approximated).
fn tool_button(ui: &mut egui::Ui, r: egui::Rect, icon: ToolIcon, text: Option<&str>, enabled: bool) -> egui::Response {
    let resp = ui.interact(r, ui.id().with(("omp_tool", r.left() as i32)), egui::Sense::click());
    // A toolbar button takes no focus: the tree keeps its marked selection
    // through «Участок» (VM: the new section selected in blue).
    crate::dialogs::keeps_focus(ui, r);
    let p = ui.painter();
    if enabled && resp.hovered() {
        p.rect_filled(r, 2.0, Color32::from_rgb(255, 189, 105));
        p.rect_filled(r.shrink(1.0), 2.0, Color32::from_rgb(255, 231, 162));
    }
    let side = crate::dpi::img(16.0);
    let cell = egui::Rect::from_min_size(egui::pos2(r.left() + 3.0, r.center().y - side / 2.0), egui::vec2(side, side));
    let ink = if enabled { Color32::from_rgb(0, 21, 110) } else { Color32::from_gray(141) };
    paint_tool_icon(p, cell, icon, enabled);
    if let Some(t) = text {
        p.text(egui::pos2(cell.right() + 3.0, r.center().y), egui::Align2::LEFT_CENTER, t, crate::dialogs::dlg_font(), ink);
    }
    resp
}

/// A TOOLBAR 475 image in its 16 px cell (our drawings; greyed disabled).
pub(crate) fn paint_tool_icon(p: &egui::Painter, cell: egui::Rect, icon: ToolIcon, enabled: bool) {
    let c = cell.center();
    let st = egui::Stroke::new(1.5f32, if enabled { Color32::from_rgb(60, 60, 60) } else { Color32::from_gray(170) });
    // Disabled images in grey (VM); the delete cross red while enabled.
    let grey = (!enabled).then(|| Color32::from_gray(190));
    match icon {
        ToolIcon::Section => tree_icon_ink(p, cell, &Node::Section(0, 0), grey),
        ToolIcon::Line => tree_icon_ink(p, cell, &Node::Line(0, 0, 0), grey),
        ToolIcon::Tap => tree_icon_ink(p, cell, &Node::Tap(0, 0, 0), grey),
        ToolIcon::Delete => {
            let st = if enabled { egui::Stroke::new(1.5f32, Color32::from_rgb(225, 30, 30)) } else { st };
            p.line_segment([c + egui::vec2(-5.0, -5.0), c + egui::vec2(5.0, 5.0)], st);
            p.line_segment([c + egui::vec2(5.0, -5.0), c + egui::vec2(-5.0, 5.0)], st);
        }
        ToolIcon::Down | ToolIcon::Up => {
            let s = if matches!(icon, ToolIcon::Down) { 1.0 } else { -1.0 };
            let tri = vec![c + egui::vec2(-4.5, -2.5 * s), c + egui::vec2(4.5, -2.5 * s), c + egui::vec2(0.0, 2.5 * s)];
            p.add(egui::Shape::convex_polygon(tri, st.color, egui::Stroke::NONE));
        }
        ToolIcon::Open => {
            p.rect_filled(egui::Rect::from_min_max(c + egui::vec2(-7.0, -4.0), c + egui::vec2(7.0, 6.0)), 1.0, Color32::from_rgb(232, 182, 70));
            p.rect_filled(egui::Rect::from_min_max(c + egui::vec2(-7.0, -6.0), c + egui::vec2(-1.0, -3.0)), 1.0, Color32::from_rgb(232, 182, 70));
        }
        ToolIcon::Save => {
            let r = egui::Rect::from_min_max(c + egui::vec2(-6.5, -6.5), c + egui::vec2(6.5, 6.5));
            p.rect_filled(r, 1.0, Color32::from_rgb(40, 60, 170));
            p.rect_filled(egui::Rect::from_min_max(r.min + egui::vec2(3.0, 1.0), egui::pos2(r.right() - 3.0, r.top() + 6.0)), 0.0, Color32::WHITE);
        }
    }
}

/// A tree item's image (BITMAP 476's four cells, our own drawings on
/// white): a station's two rings, a section's conductor with its ends, a
/// parallel line's two conductors, a tap's branch.
fn tree_icon(p: &egui::Painter, r: egui::Rect, node: &Node) {
    tree_icon_ink(p, r, node, None);
}

/// [`tree_icon`] all in `grey` when given (a disabled toolbar button's
/// image, VM).
fn tree_icon_ink(p: &egui::Painter, r: egui::Rect, node: &Node, grey: Option<Color32>) {
    let c = r.center();
    let ink = |col: Color32| grey.unwrap_or(col);
    let v = |x: f32, y: f32| c + egui::vec2(x, y);
    let st = |w: f32, col| egui::Stroke::new(w, col);
    match node {
        // Two overlapping rings, a transformer's mark (seen on the VM).
        Node::Station(_) => {
            for x in [-2.5, 2.5] {
                p.circle_stroke(v(x, 0.0), 4.5, st(1.5, ink(Color32::from_rgb(40, 40, 100))));
            }
        }
        Node::Section(..) => {
            p.line_segment([v(-5.0, 0.0), v(5.0, 0.0)], st(2.0, ink(Color32::from_rgb(170, 160, 90))));
            for x in [-6.5, 6.5] {
                p.rect_filled(egui::Rect::from_center_size(v(x, 0.0), egui::vec2(3.0, 5.0)), 0.0, ink(Color32::from_rgb(230, 120, 170)));
            }
        }
        Node::Line(..) => {
            for x in [-2.5, 2.5] {
                p.line_segment([v(x, -6.0), v(x, 6.0)], st(1.5, ink(Color32::from_rgb(90, 90, 90))));
            }
        }
        Node::Tap(..) => {
            p.line_segment([v(-2.0, -6.0), v(-2.0, 6.0)], st(1.5, ink(Color32::from_rgb(90, 90, 90))));
            p.line_segment([v(-2.0, 0.0), v(5.0, 0.0)], st(1.5, ink(Color32::from_rgb(90, 90, 90))));
        }
    }
}

/// The ОМП result bar's height. The app creates bar 221 with height −1 in
/// message-bar mode (`0x474cbd`), so CMFCCaptionBar sizes it from its
/// font's height (mfc120u `0x1006fd4c`: its margins + the text height,
/// the image's if taller): 33 px at 100 % (VM), the 9 pt font's height
/// 16 / 20 / 24 px adding 4 / 8 at 125 / 150 %.
pub fn omp_bar_h() -> f32 {
    crate::dpi::measured(33.0, 37.0, 41.0)
}

/// «Параметры ОМП»'s tree and grid in its client `rect` (OnInitDialog
/// `0x4a7c50`, in pixels from the client): from the toolbar's bottom + 11
/// (the toolbar at 10, its height following its combo's: 23 / 29 / 34 px
/// at 100 / 125 / 150 %, VM), H − 90 tall; the tree from 10, W/3 − 20
/// wide, the grid from W/3, 2W/3 − 10 wide (the thirds whole pixels).
pub(crate) fn params_layout(rect: egui::Rect) -> (egui::Rect, egui::Rect) {
    let (w, h) = (rect.width(), rect.height());
    let third = (w / 3.0).floor();
    let top = 10.0 + crate::dpi::measured(23.0, 29.0, 34.0) + 11.0;
    let tree = egui::Rect::from_min_size(rect.min + egui::vec2(10.0, top), egui::vec2(third - 20.0, h - 90.0));
    let grid = egui::Rect::from_min_size(rect.min + egui::vec2(third, top), egui::vec2((2.0 * w / 3.0).floor() - 10.0, h - 90.0));
    (tree, grid)
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

    /// The КЗ flags lead the digital list without a recompute: a formula
    /// on a record's own digital channel still reads that channel.
    #[test]
    fn a_formula_reads_its_digital_channel_after_omp() {
        let mut app = demo_app();
        grounded(&mut app);
        let d = app.active_doc().unwrap();
        let name = d.osc.digital[0].id.clone();
        let want: Vec<f64> = d.flagged.digital[0].samples.iter().map(|&b| if b { 2.0 } else { 0.0 }).collect();
        assert!(!app.active_doc().unwrap().v1_placed);
        app.run_omp();
        assert!(app.active_doc().unwrap().osc.digital.len() > 1, "the flags went in");
        assert!(app.active_doc().unwrap().v1_placed, "ОМП places В1 on its window");
        assert!(app.append_computed(&[("X".into(), format!("\"{name}\"*2"))]).is_some(), "{:?}", app.dlg.alert());
        let d = app.active_doc().unwrap();
        assert_eq!(d.computed.iter().find(|c| c.name == "X").unwrap().samples, want);
        // A formula on a КЗ flag itself reads it, also after a re-run gave
        // the flags new values in place (`omp_compute`, no recompute).
        let flag = |d: &Doc| -> Vec<f64> { d.osc.digital.iter().find(|c| c.id == KZ_PHASES[0]).unwrap().samples.iter().map(|&b| f64::from(u8::from(b))).collect() };
        let first = flag(d);
        assert!(app.append_computed(&[("K".into(), format!("\"{}\"*1", KZ_PHASES[0]))]).is_some(), "{:?}", app.dlg.alert());
        assert_eq!(app.active_doc().unwrap().computed.iter().find(|c| c.name == "K").unwrap().samples, first);
        let d = app.active_doc_mut().unwrap();
        let k = d.osc.digital.iter().position(|c| c.id == KZ_PHASES[0]).unwrap();
        d.osc.digital[k].samples.iter_mut().for_each(|v| *v = !*v);
        let flipped = flag(d);
        assert!(app.append_computed(&[("K2".into(), format!("\"{}\"*1", KZ_PHASES[0]))]).is_some());
        assert_eq!(app.active_doc().unwrap().computed.iter().find(|c| c.name == "K2").unwrap().samples, flipped);
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

    /// The dialog driven by the pointer (VM, «Участок» then OK): the
    /// toolbar leaves the tree's focus, a click on a value gives the grid
    /// the focus and OK takes it — the row then marked in (240).
    #[test]
    fn the_toolbar_takes_no_focus_and_ok_takes_the_grids() {
        let mut d = ParamsDialog::new(0, OmpParams { method: ISOLATED, ..Default::default() });
        let ctx = egui::Context::default();
        let mut t = 0.0;
        let mut pass = |d: &mut ParamsDialog, events: Vec<egui::Event>| {
            t += 0.05;
            let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0))), time: Some(t), events, ..Default::default() };
            let _ = ctx.run(raw, |ctx| {
                egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| d.ui(ui, &[]));
            });
        };
        let click = |p: egui::Pos2| {
            let b = |pressed| egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
            [vec![egui::Event::PointerMoved(p), b(true)], vec![b(false)]]
        };
        let salt = d.salt;
        let tree_focus = |ctx: &egui::Context| ctx.data(|m| m.get_temp::<bool>(egui::Id::new(("omp_tree", salt)).with("fv_tree_focus")));
        pass(&mut d, vec![]);
        pass(&mut d, vec![]);
        for e in click(egui::pos2(292.0 + 30.0, 12.0 + 11.0)) {
            pass(&mut d, e);
        }
        assert_eq!(d.sel, Node::Section(0, 0), "«Участок» added a section");
        assert_eq!(tree_focus(&ctx), Some(true), "the tree keeps the focus");
        for e in click(egui::pos2(237.0 + 300.0, 44.0 + 8.0)) {
            pass(&mut d, e);
        }
        assert!(d.grid_focus && d.row_sel.is_some());
        assert_eq!(tree_focus(&ctx), Some(false));
        pass(&mut d, vec![]);
        for e in click(egui::pos2(465.0 + 50.0, 413.0 + 10.0)) {
            pass(&mut d, e);
        }
        assert!(!d.grid_focus && d.row_sel.is_some(), "OK takes the focus, the row stays selected");
        assert!(d.edit.is_none(), "the edit ended with its focus");
        // A click at the left of the «1» puts the caret before it; Esc in
        // the edit puts the old value back (VM: «5», Esc → «1»).
        for e in click(egui::pos2(475.0, 44.0 + 8.0)) {
            pass(&mut d, e);
        }
        pass(&mut d, vec![]);
        assert!(d.edit.is_some());
        pass(&mut d, vec![egui::Event::Text("5".into())]);
        assert_eq!(d.work.stations[0].sections[0].length, "51", "the caret where the click fell, left of «1»");
        pass(&mut d, vec![egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }]);
        assert_eq!(d.work.stations[0].sections[0].length, "1");
        assert!(d.edit.is_none());
    }

    /// Neither the ОМП run (`0x4a00b0`) nor paste (57637, `0x45ad02`)
    /// reaches SetModifiedFlag; «Объединить» does (L9.19).
    #[test]
    fn a_value_is_bold_until_the_grid_is_built_again() {
        // VM: loaded values regular; an edited one bold; regular again
        // after another element and back.
        let mut d = ParamsDialog::new(0, OmpParams { method: ISOLATED, ..Default::default() });
        d.add_section();
        let st = Node::Station(0);
        let inom = grid_rows(d.work.method, st).iter().position(|r| r.1 == Field::Inom).unwrap();
        let before = d.built_values(st);
        *field_mut(&mut d.work, st, Field::Inom).unwrap() = "10005".into();
        assert_ne!(d.built_values(st)[inom], "10005", "edited: differs from the built value");
        d.row_sel = Some((st, inom));
        let _ = d.built_values(Node::Section(0, 0));
        assert_eq!(d.row_sel, None, "a rebuilt grid has no selected row");
        assert_eq!(d.built_values(st)[inom], "10005", "built anew on coming back");
        let mut loaded = d.work.clone();
        *field_mut(&mut loaded, st, Field::Inom).unwrap() = "7".into();
        d.load(loaded);
        assert_eq!(d.built_values(st)[inom], "7", "a load builds anew");
        assert_eq!(before.len(), d.built_values(st).len());
    }

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

