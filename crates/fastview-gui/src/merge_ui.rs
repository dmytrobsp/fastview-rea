//! «Объединение» in the GUI (ledger L9.15, `notes/UI_BEHAVIOUR_SPEC.md`
//! §7.8, §4): 57634 copies the selected channels with their own time base,
//! 57637 pastes them as *pending* channels (bit 14) shown at the merge offset,
//! the merge bar (CMergeBar, 70 px) synchronises them by start time, fault
//! component or pre-fault mode, a pending lane can be dragged in time, and
//! «Объединить» resamples everything onto one grid.

use crate::app::{App, ChanKey, ChanView, Computed, Doc};
use eframe::egui;
use fvb_core::merge::{self, Pasted, SyncChan, SyncMsg};

/// A channel on the merge clipboard: its data with its own times, the
/// colour, and its record's start time and network frequency.
#[derive(Debug, Clone)]
pub struct ClipChan {
    pub pasted: Pasted,
    pub color: egui::Color32,
    pub start: Option<f64>,
}

impl ClipChan {
    pub fn name(&self) -> &str {
        match &self.pasted.channel {
            Ok(a) => &a.id,
            Err(d) => &d.id,
        }
    }
}

/// The merge bar's state.
#[derive(Debug, Clone, Default)]
pub struct MergeBar {
    /// 1088: 0 by start time, 1 by the fault component, 2 by the pre-fault mode.
    pub method: u8,
    /// 1089: a base analog channel; 1090: a pending analog channel.
    pub ch1: Option<usize>,
    pub ch2: Option<usize>,
    pub status: String,
}

/// The record start of a document in seconds.
fn doc_start(d: &Doc) -> Option<f64> {
    let s = &d.osc.stamps;
    fvb_core::comtrade::stamp_seconds(&s.start_date, &s.start_time, d.osc.rev_year)
}

fn duration(times: &[f64]) -> f64 {
    times.last().copied().unwrap_or(0.0) - times.first().copied().unwrap_or(0.0)
}

impl Doc {
    /// Rebuild the lanes of the pending channels at the merge offset: each is
    /// shown on the document's grid (linear interpolation, NaN outside).
    pub fn refresh_pending(&mut self) {
        let t0 = self.osc.time.first().copied().unwrap_or(0.0);
        let grid: Vec<f64> = self.osc.time.iter().map(|t| t - t0 - self.merge_offset).collect();
        for p in &self.pending {
            let name = match &p.pasted.channel {
                Ok(a) => &a.id,
                Err(d) => &d.id,
            };
            let Some(c) = self.computed.iter_mut().find(|c| c.formula == PENDING && &c.name == name) else { continue };
            let ts = &p.pasted.times;
            c.samples = grid
                .iter()
                .map(|&t| {
                    if ts.is_empty() || t < ts[0] || t > ts[ts.len() - 1] {
                        return f64::NAN;
                    }
                    let i = ts.partition_point(|&s| s <= t).clamp(1, ts.len() - 1);
                    let v = |k: usize| match &p.pasted.channel {
                        Ok(a) => a.samples[k],
                        Err(d) => f64::from(u8::from(d.samples[k])),
                    };
                    let w = (t - ts[i - 1]) / (ts[i] - ts[i - 1]).max(1e-12);
                    v(i - 1) + (v(i) - v(i - 1)) * w.clamp(0.0, 1.0)
                })
                .collect();
        }
    }

    /// Keep only pending channels that still have their lane (an undo of the
    /// paste removes the lanes).
    pub fn prune_pending(&mut self) {
        let names: Vec<String> = self.computed.iter().filter(|c| c.formula == PENDING).map(|c| c.name.clone()).collect();
        self.pending.retain(|p| names.iter().any(|n| n == p.name()));
        if self.pending.is_empty() {
            self.merge_offset = 0.0;
        }
    }

    /// The allowed merge offsets: the pasted record may slide until one
    /// period of overlap is left.
    pub fn offset_range(&self) -> (f64, f64) {
        let f = self.osc.line_freq.max(1.0);
        let base = duration(&self.osc.time);
        let pasted = self.pending.iter().map(|p| duration(&p.pasted.times)).fold(0.0, f64::max);
        (-(pasted - 1.0 / f), base - 1.0 / f)
    }

    pub fn set_merge_offset(&mut self, offset: f64) {
        let (lo, hi) = self.offset_range();
        self.merge_offset = offset.clamp(lo.min(hi), hi.max(lo));
        self.refresh_pending();
    }
}

/// The formula marker of a pending (pasted, not yet merged) lane.
pub const PENDING: &str = "объединение";

impl App {
    /// 57634 (`FUN_00430af0`): the selected file channels, analog and
    /// discrete, with their times, rate and record start.
    pub fn copy_channels_for_merge(&mut self) {
        let Some(d) = self.active_doc() else { return };
        let t0 = d.osc.time.first().copied().unwrap_or(0.0);
        let times: Vec<f64> = d.osc.time.iter().map(|t| t - t0).collect();
        let start = doc_start(d);
        let mut clip = Vec::new();
        for k in &d.selection {
            let (channel, color) = match *k {
                ChanKey::Analog(i) if i < d.osc.analog.len() => (Ok(d.osc.analog[i].clone()), d.chans[i].color),
                ChanKey::Digital(i) if i < d.osc.digital.len() => (Err(d.osc.digital[i].clone()), egui::Color32::from_rgb(0x44, 0x44, 0xAA)),
                _ => continue,
            };
            clip.push(ClipChan { pasted: Pasted { channel, times: times.clone(), rate: d.osc.sample_rate }, color, start });
        }
        if clip.is_empty() {
            self.status = "Не выбраны каналы для копирования.".into();
            return;
        }
        self.status = format!("Скопировано каналов для объединения: {}", clip.len());
        self.channel_clip = clip;
    }

    /// 57637: undo snapshot, clear the ОМП result, deselect all, append the
    /// channels as pending lanes (duplicate names get " (2)", " (3)"…) and
    /// show the merge bar.
    pub fn paste_channels_for_merge(&mut self) {
        if self.channel_clip.is_empty() {
            return;
        }
        let Some(ai) = self.active else { return };
        if crate::omp_ui::has_pending_paste(&self.docs[ai]) {
            return;
        }
        self.snapshot_undo();
        self.omp_clear(ai);
        let clip = self.channel_clip.clone();
        let base = self.docs.len() + 11;
        let d = &mut self.docs[ai];
        d.selection.clear();
        for (j, mut c) in clip.into_iter().enumerate() {
            let taken = |n: &str, d: &Doc| {
                d.osc.analog.iter().any(|a| a.id == n) || d.osc.digital.iter().any(|x| x.id == n) || d.computed.iter().any(|x| x.name == n)
            };
            let mut name = c.name().to_string();
            let mut k = 2;
            while taken(&name, d) {
                name = format!("{} ({k})", c.name());
                k += 1;
            }
            match &mut c.pasted.channel {
                Ok(a) => a.id = name.clone(),
                Err(x) => x.id = name.clone(),
            }
            let mut lane = Computed::new(name, PENDING.into(), Vec::new(), if c.color == egui::Color32::TRANSPARENT { Self::palette(base + j) } else { c.color });
            if let Ok(a) = &c.pasted.channel {
                lane.unit = a.unit.clone();
            }
            d.computed.push(lane);
            d.pending.push(c);
        }
        d.merge_offset = 0.0;
        d.refresh_pending();
        self.merge = Some(MergeBar::default());
    }

    /// 1091 «Синхронизация» (`0x47bb90`) by the chosen method; the result
    /// text goes to the bar.
    fn synchronise(&mut self) {
        let Some(ai) = self.active else { return };
        let Some(bar) = self.merge.clone() else { return };
        let d = &self.docs[ai];
        let result: Result<(f64, SyncMsg), SyncMsg> = match bar.method {
            0 => {
                let p = d.pending.first();
                match (doc_start(d), p.and_then(|p| p.start)) {
                    (Some(b), Some(s)) => merge::sync_by_start(b, duration(&d.osc.time), s, p.map_or(0.0, |p| duration(&p.pasted.times))).map(|o| (o, SyncMsg::Done)),
                    _ => Err(SyncMsg::NoOverlap),
                }
            }
            m => {
                let (Some(c1), Some(c2)) = (bar.ch1, bar.ch2) else {
                    self.merge.as_mut().unwrap().status = "Выберите каналы".into();
                    return;
                };
                let Some(p) = d.pending.get(c2) else { return };
                let Ok(a2) = &p.pasted.channel else { return };
                let t0 = d.osc.time.first().copied().unwrap_or(0.0);
                let t1: Vec<f64> = d.osc.time.iter().map(|t| t - t0).collect();
                let n1 = d.osc.samples_per_cycle().max(2);
                let n2 = ((p.pasted.rate / d.osc.line_freq.max(1.0)).round() as usize).max(2);
                let x1 = &d.flagged.analog[c1].samples;
                let s1 = SyncChan { times: &t1, x: x1, n: n1 };
                let s2 = SyncChan { times: &p.pasted.times, x: &a2.samples, n: n2 };
                let k1 = d.main_cursor;
                let tc = t1.get(k1).copied().unwrap_or(0.0) - d.merge_offset;
                let k2 = p.pasted.times.partition_point(|&t| t < tc).min(p.pasted.times.len().saturating_sub(1));
                if m == 1 {
                    merge::sync_by_fault(&s1, &s2, k1, k2)
                } else {
                    merge::sync_by_prefault(&s1, &s2, k1, k2, d.osc.line_freq)
                }
            }
        };
        let text = match result {
            Ok((offset, msg)) => {
                self.docs[ai].set_merge_offset(offset);
                msg.text()
            }
            Err(msg) => msg.text(),
        };
        if let Some(b) = self.merge.as_mut() {
            b.status = text.into();
        }
    }

    /// 1092 «Объединить» (`0x47d990`): the record and the pending channels
    /// resampled onto one grid; view state kept, comments and визиры moved by
    /// time; the bar is hidden.
    fn merge_records(&mut self) {
        let Some(ai) = self.active else { return };
        let d = &mut self.docs[ai];
        if d.pending.is_empty() {
            return;
        }
        let pasted: Vec<Pasted> = d.pending.iter().map(|p| p.pasted.clone()).collect();
        let merged = merge::merge(&d.osc, &pasted, d.merge_offset);
        let shift = (-d.merge_offset).max(0.0);
        let rate = merged.sample_rate;
        let old_t0 = d.osc.time.first().copied().unwrap_or(0.0);
        let remap = |k: usize, d: &Doc| -> usize {
            let t = d.osc.time.get(k).copied().unwrap_or(old_t0) - old_t0 + shift;
            ((t * rate).round().max(0.0) as usize).min(merged.time.len().saturating_sub(1))
        };
        let (m, a, v3) = (remap(d.main_cursor, d), remap(d.aux_cursor, d), remap(d.v3_cursor, d));
        let mut comments = d.comments.clone();
        for c in &mut comments {
            use crate::comments::CommentKind::*;
            match &mut c.kind {
                Moment { pos } => *pos = remap(*pos, d),
                Level { from, to, .. } | Area { from, to, .. } => {
                    *from = remap(*from, d);
                    *to = remap(*to, d);
                }
            }
        }
        let nb = d.osc.analog.len();
        d.osc = merged;
        for i in nb..d.osc.analog.len() {
            let mut v = ChanView::new(&d.osc.analog[i], i);
            if let Some(p) = d.pending.iter().find(|p| p.name() == d.osc.analog[i].id) {
                v.color = p.color;
            }
            d.chans.push(v);
        }
        d.digital_visible.resize(d.osc.digital.len(), true);
        d.computed.retain(|c| c.formula != PENDING);
        d.pending.clear();
        d.merge_offset = 0.0;
        d.comments = comments;
        (d.main_cursor, d.aux_cursor, d.v3_cursor) = (m, a, v3);
        d.first = 0;
        d.xscale = None;
        d.recompute_signals();
        self.merge = None;
        self.status = "Осциллограммы объединены".into();
    }

    /// The merge bar (CMergeBar, 70 px): method, the two channels,
    /// «Синхронизация», «Объединить», the status text. Hiding it while
    /// channels are pending undoes the paste.
    pub fn merge_bar_ui(&mut self, ui: &mut egui::Ui) {
        let Some(ai) = self.active else { return };
        self.docs[ai].prune_pending();
        if self.docs[ai].pending.is_empty() {
            self.merge = None;
            return;
        }
        let Some(mut bar) = self.merge.clone() else { return };
        let (mut sync, mut join, mut close) = (false, false, false);
        let d = &self.docs[ai];
        ui.horizontal(|ui| {
            ui.label("Синхронизация");
            egui::ComboBox::from_id_salt("merge_method")
                .width(170.0)
                .selected_text(["По времени начала записи", "По авар. составляющей", "По доаварийному режиму"][bar.method as usize])
                .show_ui(ui, |ui| {
                    for (k, t) in ["По времени начала записи", "По авар. составляющей", "По доаварийному режиму"].into_iter().enumerate() {
                        ui.selectable_value(&mut bar.method, k as u8, t);
                    }
                });
            ui.add_enabled_ui(bar.method != 0, |ui| {
                ui.label("Канал 1");
                let name1 = bar.ch1.map(|i| d.osc.analog[i].id.clone()).unwrap_or_default();
                egui::ComboBox::from_id_salt("merge_ch1").width(90.0).selected_text(name1).show_ui(ui, |ui| {
                    for (i, c) in d.osc.analog.iter().enumerate() {
                        ui.selectable_value(&mut bar.ch1, Some(i), &c.id);
                    }
                });
                ui.label("Канал 2");
                let name2 = bar.ch2.and_then(|i| d.pending.get(i)).map(|p| p.name().to_string()).unwrap_or_default();
                egui::ComboBox::from_id_salt("merge_ch2").width(90.0).selected_text(name2).show_ui(ui, |ui| {
                    for (i, p) in d.pending.iter().enumerate().filter(|(_, p)| p.pasted.channel.is_ok()) {
                        ui.selectable_value(&mut bar.ch2, Some(i), p.name());
                    }
                });
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                close = ui.small_button("✕").on_hover_text("Скрыть (отменяет вставку)").clicked();
                join = ui.button("Объединить").clicked();
                sync = ui.button("Синхронизация").clicked();
            });
        });
        ui.label(egui::RichText::new(&bar.status).small());
        self.merge = Some(bar);
        if sync {
            self.synchronise();
        }
        if join {
            self.merge_records();
        } else if close {
            self.undo();
            self.docs[ai].prune_pending();
            self.merge = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Command;

    fn demo_app() -> App {
        let mut app = App::new();
        app.dispatch(Command::Demo);
        app
    }

    #[test]
    fn paste_is_pending_then_merges_onto_one_grid() {
        let mut app = demo_app();
        let n0 = app.active_doc().unwrap().osc.analog.len();
        app.active_doc_mut().unwrap().selection.insert(ChanKey::Analog(0));
        app.copy_channels_for_merge();
        app.paste_channels_for_merge();
        let d = app.active_doc().unwrap();
        assert_eq!(d.pending.len(), 1);
        assert!(d.computed.iter().any(|c| c.formula == PENDING && c.name.ends_with(" (2)")), "duplicate names get (2)");
        assert!(crate::omp_ui::has_pending_paste(d), "ОМП and another paste are blocked");
        assert!(app.merge.is_some(), "the merge bar shows");
        // Shift by 2 ms; out-of-range offsets are clamped.
        app.active_doc_mut().unwrap().set_merge_offset(0.002);
        app.active_doc_mut().unwrap().set_merge_offset(1e9);
        let (lo, hi) = app.active_doc().unwrap().offset_range();
        assert!(app.active_doc().unwrap().merge_offset <= hi && lo <= 0.0);
        app.active_doc_mut().unwrap().set_merge_offset(0.002);
        app.merge_records();
        let d = app.active_doc().unwrap();
        assert_eq!(d.osc.analog.len(), n0 + 1, "the pasted channel became a file channel");
        assert!(d.pending.is_empty() && !crate::omp_ui::has_pending_paste(d));
        assert_eq!(d.chans.len(), d.osc.analog.len());
        assert!(d.osc.analog.iter().all(|c| c.samples.len() == d.osc.time.len()));
        assert!(app.merge.is_none(), "the bar is hidden");
    }

    #[test]
    fn closing_the_bar_with_pending_channels_undoes_the_paste() {
        let mut app = demo_app();
        app.active_doc_mut().unwrap().selection.insert(ChanKey::Analog(0));
        app.copy_channels_for_merge();
        app.paste_channels_for_merge();
        app.undo();
        let d = app.active_doc_mut().unwrap();
        d.prune_pending();
        assert!(d.pending.is_empty());
        assert!(!crate::omp_ui::has_pending_paste(d));
    }
}
