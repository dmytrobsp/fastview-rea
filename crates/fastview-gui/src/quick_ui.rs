//! The quick-analysis property sheet (`CQAnPropSheet`): one form per tab whose
//! OK button generates computed channels (`fvb_core::quick`).

use crate::app::{AnalysisTab, App};
use eframe::egui;
use fvb_core::recset::SetNode;
use fvb_core::quick::{self, Arm, Braking, Currents, GroupComp, PhaseComp, Reduction, Voltages, ZeroCurrent, ZeroVoltage};

/// Form state of every tab.
#[derive(Debug, Clone)]
pub struct QuickForms {
    pub ia: String,
    pub ib: String,
    pub ic: String,
    pub calc_ib: bool,
    pub currents_off: bool,
    pub voltages_off: bool,
    pub line_voltages: bool,
    pub ua: String,
    pub ub: String,
    pub uc: String,
    pub uab: String,
    pub ubc: String,
    /// Zero sequence (537): 1 calculated, 2 measured (current); 1 from
    /// Ua,Ub,Uc, 2 from Uни,Uик, 3 measured (voltage); each group's own
    /// «Отключить».
    pub zero_i: u8,
    pub zero_u: u8,
    pub zero_i_off: bool,
    pub zero_u_off: bool,
    pub i3o: String,
    pub u3o: String,
    pub uni: String,
    pub uik: String,
    pub itr: String,
    pub i0_parallel: String,
    pub per_unit: bool,
    pub r0: String,
    pub r1: String,
    pub rm: String,
    pub x0: String,
    pub x1: String,
    pub xm: String,
    pub k0: String,
    pub km: String,
    pub f0: String,
    pub fm: String,
    pub dif_phases: usize,
    /// «Количество плеч» (2…24).
    pub dif_arms: usize,
    /// The page's 24 arm slots (`0x47fb40`): lowering the count keeps the
    /// slots past it.
    pub arms: Vec<ArmForm>,
    /// «Приведение плеч», page-wide (radios 2080–2083 are never stored per
    /// arm, `0x482ca0`): 0 CoeffPriv, 1 CoeffTT, 2 CoeffPowerPriv, 3
    /// CoeffAmpPriv — the one the page's radios set.
    pub dif_reduction: u8,
    /// Further radios checked by a `.set` with more than one method key
    /// (the load checks each independently, `0x4844a0`); cleared by a click.
    pub dif_also: Vec<u8>,
    /// «Учет группы соединений», page-wide: 0 none, 1 GrsPosl, 2
    /// GrsTriangle.
    pub dif_group: u8,
    /// 0 sum (полусумма модулей), 1 arms 1–2, 2 maximum.
    pub braking: u8,
    pub k_torm: String,
    /// 539's «Текущее плечо» (0-based).
    pub cur_arm: usize,
}

#[derive(Debug, Clone)]
pub struct ArmForm {
    pub currents: [String; 3],
    /// Each reduction block's own edits: «По мощности:» Kтт / Sном / Uном,
    /// «По номинальному току:» Kтт / Iном, «По коэффициенту приведения:»
    /// Kприв, «По коэффициенту трансформации ТТ:» Kтт / Kттбаз.
    pub pw_ktt: String,
    pub snom: String,
    pub unom: String,
    pub amp_ktt: String,
    pub inom: String,
    pub kpriv: String,
    pub tt_ktt: String,
    pub tt_base: String,
    /// The «Группа:» combos of «По последовательностям» (2127, `+0x4f0`)
    /// and «Цифровой треугольник» (2129, `+0x550`): two numbers per arm.
    pub seq_group: String,
    pub tri_group: String,
    pub remove_i0: bool,
}

/// The number of arm slots of page 539.
pub const ARM_SLOTS: usize = 24;

impl Default for ArmForm {
    fn default() -> Self {
        ArmForm {
            currents: Default::default(),
            pw_ktt: "1.".into(),
            snom: "1.".into(),
            unom: "1.".into(),
            amp_ktt: "1.".into(),
            inom: "1.".into(),
            kpriv: "1.".into(),
            tt_ktt: "1.".into(),
            tt_base: "1.".into(),
            seq_group: "0".into(),
            tri_group: "0".into(),
            remove_i0: true,
        }
    }
}

impl Default for QuickForms {
    fn default() -> Self {
        let s = String::new;
        QuickForms {
            ia: s(),
            ib: s(),
            ic: s(),
            calc_ib: false,
            currents_off: false,
            voltages_off: false,
            line_voltages: false,
            ua: s(),
            ub: s(),
            uc: s(),
            uab: s(),
            ubc: s(),
            zero_i: 1,
            zero_u: 1,
            zero_i_off: false,
            zero_u_off: false,
            i3o: s(),
            u3o: s(),
            uni: s(),
            uik: s(),
            // `0x411f40`: Iтр «0.25», R / X «1.», K0 / Ф0 / Rm / Xm / Km / Фm
            // «0.» (`0x412688`–`0x4126e2`).
            itr: "0.25".into(),
            i0_parallel: s(),
            per_unit: true,
            r0: "1.".into(),
            r1: "1.".into(),
            rm: "0.".into(),
            x0: "1.".into(),
            x1: "1.".into(),
            xm: "0.".into(),
            k0: "0.".into(),
            km: "0.".into(),
            f0: "0.".into(),
            fm: "0.".into(),
            dif_phases: 3,
            dif_arms: 2,
            arms: vec![ArmForm::default(); ARM_SLOTS],
            dif_reduction: 2,
            dif_also: Vec::new(),
            dif_group: 0,
            braking: 0,
            k_torm: "0.5".into(),
            cur_arm: 0,
        }
    }
}

/// A group box: its caption and, at the top right, an «Отключить» check
/// (`enabled` — each group's check greys the other's, so never both).
fn group(ui: &mut egui::Ui, title: &str, off: Option<(&mut bool, bool)>, body: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_width(470.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(title).strong());
            if let Some((v, enabled)) = off {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_enabled(enabled, egui::Checkbox::new(v, "Отключить"));
                });
            }
        });
        body(ui);
    });
}

/// A labelled channel combo as one grid cell, greyed unless `enabled`.
fn cell_combo(ui: &mut egui::Ui, enabled: bool, id: &str, label: &str, value: &mut String, names: &[String]) {
    ui.add_enabled_ui(enabled, |ui| {
        ui.horizontal(|ui| {
            ui.add_sized([30.0, 18.0], egui::Label::new(label));
            egui::ComboBox::from_id_salt(id).selected_text(value.as_str()).width(140.0).show_ui(ui, |ui| {
                // The placeholder inserted at the top (`0x411c70`).
                if value == fvb_core::addformula::NO_CHANNEL || value == fvb_core::addformula::NOT_COUNTED {
                    let v = value.clone();
                    ui.selectable_value(value, v.clone(), v);
                }
                for n in names {
                    ui.selectable_value(value, n.clone(), n);
                }
            });
        });
    });
}

/// A labelled edit as one grid cell, greyed unless `enabled`.
fn cell_edit(ui: &mut egui::Ui, enabled: bool, label: &str, value: &mut String) {
    ui.add_enabled_ui(enabled, |ui| {
        ui.horizontal(|ui| {
            ui.add_sized([80.0, 18.0], egui::Label::new(label));
            ui.add(egui::TextEdit::singleline(value).desired_width(90.0));
        });
    });
}

/// A combo's channel counts as chosen (not empty, not «нет канала»).
fn chosen(s: &str) -> bool {
    !s.is_empty() && s != fvb_core::addformula::NO_CHANNEL
}

impl QuickForms {
    /// 539's checked reduction methods in the generation's order (cast 0,
    /// TT 1, power 2, nominal current 3 — `0x47ff60`).
    pub fn dif_methods(&self) -> Vec<u8> {
        (0..4).filter(|m| *m == self.dif_reduction || self.dif_also.contains(m)).collect()
    }

    /// The method whose edit block is enabled: the enablers run per checked
    /// radio in the order power, nominal current, cast, TT (`0x4844a0`), so
    /// the last checked one in that order.
    pub fn dif_edit_method(&self) -> u8 {
        let m = self.dif_methods();
        [2, 3, 0, 1].into_iter().rev().find(|x| m.contains(x)).unwrap_or(self.dif_reduction)
    }

    /// Whether «Рассчитать» is enabled on `tab` (the page's check, slot
    /// 0x1d0): 536 `0x48d5a0` — each group not switched off needs its
    /// combos (Ib unless calculated; Ua–Uc or Uab / Ubc by the radio); 538
    /// `0x488550` — Iтр, the currents and the voltages; 543 `0x48b360` — the
    /// combos, Iтр, the active group's edits and, with a parallel line,
    /// its matching edits; 537 `0x48a1f0` — each
    /// live group the combos of its radio (Ia–Ic or 3Io; Ua–Uc, Uни / Uик or
    /// 3Uo). Other pages: always.
    pub fn ok_enabled(&self, tab: AnalysisTab) -> bool {
        let currents = chosen(&self.ia) && (self.calc_ib || chosen(&self.ib)) && chosen(&self.ic);
        let voltages = if self.line_voltages {
            chosen(&self.uab) && chosen(&self.ubc)
        } else {
            chosen(&self.ua) && chosen(&self.ub) && chosen(&self.uc)
        };
        match tab {
            AnalysisTab::Symmetric => (self.currents_off || currents) && (self.voltages_off || voltages),
            // An edit counts by its text's length: blanks fill it (`0x48b360`).
            AnalysisTab::PhasePhase => !self.itr.is_empty() && currents && voltages,
            AnalysisTab::Phase => {
                let filled = |v: &[&String]| v.iter().all(|s| !s.is_empty());
                let comp = if self.per_unit { filled(&[&self.x1, &self.r1, &self.x0, &self.r0]) } else { filled(&[&self.k0, &self.f0]) };
                let parallel = !self.parallel_used() || if self.per_unit { filled(&[&self.xm, &self.rm]) } else { filled(&[&self.km, &self.fm]) };
                let u = chosen(&self.ua) && chosen(&self.ub) && chosen(&self.uc);
                chosen(&self.ia) && chosen(&self.ib) && chosen(&self.ic) && u && !self.itr.is_empty() && comp && parallel
            }
            AnalysisTab::Zero => {
                let i = match self.zero_i {
                    2 => chosen(&self.i3o),
                    _ => chosen(&self.ia) && chosen(&self.ib) && chosen(&self.ic),
                };
                let u = match self.zero_u {
                    2 => chosen(&self.uni) && chosen(&self.uik),
                    3 => chosen(&self.u3o),
                    _ => chosen(&self.ua) && chosen(&self.ub) && chosen(&self.uc),
                };
                (self.zero_i_off || i) && (self.zero_u_off || u)
            }
            _ => true,
        }
    }

    /// 543's «3Ion:» is a channel, not «Не учитывать» (string 3299).
    fn parallel_used(&self) -> bool {
        chosen(&self.i0_parallel) && self.i0_parallel != fvb_core::addformula::NOT_COUNTED
    }

    fn currents(&self) -> Currents {
        Currents { ia: self.ia.clone(), ib: self.ib.clone(), ic: self.ic.clone(), calc_ib: self.calc_ib }
    }

    fn voltages(&self) -> Voltages {
        if self.line_voltages {
            Voltages::Line { uab: self.uab.clone(), ubc: self.ubc.clone() }
        } else {
            Voltages::Phase { ua: self.ua.clone(), ub: self.ub.clone(), uc: self.uc.clone() }
        }
    }

    /// The active tab's channels; `taken` = the record's channel names (the
    /// batch names its own channels as they will be appended).
    pub fn generate(&self, tab: AnalysisTab, taken: &[String]) -> quick::Generated {
        let i = (!self.currents_off).then(|| self.currents());
        let u = (!self.voltages_off).then(|| self.voltages());
        match tab {
            AnalysisTab::Symmetric => quick::symmetric(i.as_ref(), u.as_ref(), taken),
            AnalysisTab::Zero => {
                let zi = match self.zero_i {
                    _ if self.zero_i_off => None,
                    2 => Some(ZeroCurrent::Measured(self.i3o.clone())),
                    _ => Some(ZeroCurrent::Calculated { ia: self.ia.clone(), ib: self.ib.clone(), ic: self.ic.clone() }),
                };
                let zu = match self.zero_u {
                    _ if self.zero_u_off => None,
                    2 => Some(ZeroVoltage::FromUniUik { uni: self.uni.clone(), uik: self.uik.clone() }),
                    3 => Some(ZeroVoltage::Measured(self.u3o.clone())),
                    _ => Some(ZeroVoltage::FromPhases { ua: self.ua.clone(), ub: self.ub.clone(), uc: self.uc.clone() }),
                };
                quick::zero(zi.as_ref(), zu.as_ref(), taken)
            }
            AnalysisTab::PhasePhase => quick::phase_phase(&self.currents(), &self.voltages(), &self.itr, taken),
            AnalysisTab::Phase => {
                let comp = if self.per_unit {
                    PhaseComp::PerUnit {
                        r0: self.r0.clone(),
                        r1: self.r1.clone(),
                        rm: self.rm.clone(),
                        x0: self.x0.clone(),
                        x1: self.x1.clone(),
                        xm: self.xm.clone(),
                    }
                } else {
                    PhaseComp::Coefficients { k0: self.k0.clone(), km: self.km.clone(), f0: self.f0.clone(), fm: self.fm.clone() }
                };
                let i0n = if self.parallel_used() { self.i0_parallel.as_str() } else { "" };
                quick::phase_z(&self.currents(), [&self.ua, &self.ub, &self.uc], i0n, &comp, &self.itr, taken)
            }
            AnalysisTab::Differential => {
                // The methods are the page's radios, the values each
                // arm's own (`0x47ff60`).
                let methods = self.dif_methods();
                let arms: Vec<Arm> = self.arms[..self.dif_arms]
                    .iter()
                    .map(|a| Arm {
                        currents: a.currents[..self.dif_phases].to_vec(),
                        reductions: methods
                            .iter()
                            .map(|m| match m {
                                0 => Reduction::Priv { k: a.kpriv.clone() },
                                1 => Reduction::Tt { ktt: a.tt_ktt.clone(), ktt_base: a.tt_base.clone() },
                                2 => Reduction::PowerPriv { ktt: a.pw_ktt.clone(), snom: a.snom.clone(), unom: a.unom.clone() },
                                _ => Reduction::AmpPriv { ktt: a.amp_ktt.clone(), inom: a.inom.clone() },
                            })
                            .collect(),
                        group: if self.dif_group == 2 { a.tri_group.clone() } else { a.seq_group.clone() },
                        remove_i0: a.remove_i0,
                    })
                    .collect();
                let group = match self.dif_group {
                    _ if self.dif_phases == 1 => GroupComp::None,
                    1 => GroupComp::Posl,
                    2 => GroupComp::Triangle,
                    _ => GroupComp::None,
                };
                let braking = match self.braking {
                    1 => Braking::Sub { k: self.k_torm.clone() },
                    2 => Braking::Max,
                    _ => Braking::Sum { k: self.k_torm.clone() },
                };
                quick::differential(self.dif_phases, &arms, &group, &braking, taken)
            }
            AnalysisTab::Formula => quick::Generated::default(),
        }
    }
}

impl App {
    /// Fill empty channel fields from the channel-role map.
    pub(crate) fn quick_defaults(&mut self) {
        use fvb_core::addformula::{NOT_COUNTED, NO_CHANNEL};
        let Some(d) = self.active_doc() else { return };
        // Every combo is preselected by its channel's name, ignoring case
        // (`0x411c70`, CB_FINDSTRINGEXACT): Ia… Uc, Uab / Ubc, 3I0 / 3U0;
        // a name the record lacks shows «Канал отсутствует» at the top
        // (3Ion «Не учитывать»); Uни / Uик take the first channel.
        let all: Vec<String> = d.osc.analog.iter().map(|c| c.id.clone()).chain(d.computed.iter().map(|c| c.name.clone())).collect();
        let found = |want: &str| all.iter().find(|n| n.to_lowercase() == want).cloned();
        let named = |want: &str| found(want).unwrap_or_else(|| NO_CHANNEL.to_string());
        let (ua, ub, uc) = (named("ua"), named("ub"), named("uc"));
        let (ia, ib, ic) = (named("ia"), named("ib"), named("ic"));
        let (uab, ubc) = (named("uab"), named("ubc"));
        let (i3o, u3o) = (named("3i0"), named("3u0"));
        let i0n = found("3i0n").unwrap_or_else(|| NOT_COUNTED.to_string());
        let first = all.first().cloned().unwrap_or_default();
        let arm = [ia.clone(), ib.clone(), ic.clone()];
        for f in &mut self.quick {
            let vals = [&ua, &ub, &uc, &ia, &ib, &ic, &i3o, &u3o, &i0n, &uab, &ubc, &first, &first];
            for (slot, v) in [&mut f.ua, &mut f.ub, &mut f.uc, &mut f.ia, &mut f.ib, &mut f.ic, &mut f.i3o, &mut f.u3o, &mut f.i0_parallel, &mut f.uab, &mut f.ubc, &mut f.uni, &mut f.uik].into_iter().zip(vals) {
                if slot.is_empty() {
                    slot.clone_from(v);
                }
            }
            for a in &mut f.arms {
                for (slot, v) in a.currents.iter_mut().zip(&arm) {
                    if slot.is_empty() {
                        slot.clone_from(v);
                    }
                }
            }
        }
    }

    /// «Быстрый анализ» (CQAnPropSheet `0x48b970`, a `CMFCPropertySheet`
    /// with the Outlook-bar look, 150 px): modal, caption string 159, the
    /// six pages in a left-hand bar (our glyphs — bitmap 549 is the
    /// original's), opened on «Редактор формул»; only «Рассчитать» (OK,
    /// string 164) and «Отмена» (3328). «Рассчитать» generates from the
    /// active page alone (`0x48c0e0`); «Отмена» generates nothing.
    pub fn quick_sheet_window(&mut self, ctx: &egui::Context) {
        if !self.quick_open {
            return;
        }
        if self.active_doc().is_none() {
            self.quick_open = false;
            return;
        }
        let (mut calc, mut cancel, mut open, mut ok_now) = (false, false, true, false);
        let editing = self.formula_page.edit.is_some();
        let enabled = self.formula_dlg.is_none() && self.formula_page.alert.is_none();
        crate::dialogs::dlg(ctx, "Быстрый анализ")
            .id(egui::Id::new("fv_quick_sheet"))
            .collapsible(false)
            .resizable(false)
            .fixed_size(egui::Vec2::new(700.0, 510.0))
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.add_enabled_ui(enabled, |ui| {
                    ui.horizontal_top(|ui| {
                        // The separator spans the row: keep it to the page.
                        ui.set_max_height(470.0);
                        ui.vertical(|ui| {
                            ui.set_width(150.0);
                            for (tab, glyph, title) in PAGES {
                                let sel = self.analysis_tab == tab;
                                let text = egui::RichText::new(format!("{glyph}\n{title}")).size(11.5);
                                let r = ui.add_sized([150.0, 58.0], egui::SelectableLabel::new(sel, text));
                                if r.clicked() && self.analysis_tab != tab {
                                    if self.analysis_tab == AnalysisTab::Formula {
                                        self.fp_kill_active();
                                    }
                                    self.analysis_tab = tab;
                                    self.quick_show_page();
                                }
                            }
                        });
                        ui.separator();
                        ui.vertical(|ui| {
                            ui.set_width(530.0);
                            ui.label(egui::RichText::new(PAGES.iter().find(|p| p.0 == self.analysis_tab).map_or("", |p| p.2)).strong());
                            ui.separator();
                            egui::ScrollArea::both().id_salt("quick_page").max_height(420.0).show(ui, |ui| {
                                ui.set_min_height(420.0);
                                if self.analysis_tab == AnalysisTab::Formula {
                                    self.formula_page_ui(ui);
                                } else {
                                    self.quick_tab_ui(ui);
                                }
                            });
                        });
                    });
                    ui.separator();
                    ui.allocate_ui_with_layout(egui::Vec2::new(ui.available_width(), 26.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        cancel = ui.add_sized([80.0, 22.0], egui::Button::new("Отмена")).clicked();
                        let ok = page_index(self.analysis_tab).is_none_or(|i| self.quick[i].ok_enabled(self.analysis_tab));
                        ok_now = ok;
                        calc = ui.add_enabled(ok, egui::Button::new("Рассчитать").min_size(egui::Vec2::new(80.0, 22.0))).clicked();
                    });
                });
            });
        if let Some(msg) = self.formula_page.alert {
            let mut close = false;
            crate::dialogs::dlg(ctx, crate::app::APP_TITLE)
                .id(egui::Id::new("fv_formula_page_alert"))
                .collapsible(false)
                .resizable(false)
                .order(egui::Order::Foreground)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    // `0x486a9d` / `0x486bd3`: MB_ICONHAND.
                    crate::dialogs::message_body(ui, crate::dialogs::MsgIcon::Error, msg);
                    ui.vertical_centered(|ui| close = ui.button("    OK    ").clicked());
                });
            if close || ctx.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Escape)) {
                self.formula_page.alert = None;
            }
        } else if enabled && !editing && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            cancel = true;
        } else if enabled && !editing && ok_now && ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            // IDOK, the sheet's default button «Рассчитать».
            calc = true;
        }
        if calc {
            self.quick_calc();
        } else if cancel || !open {
            self.quick_save_pages();
            self.quick_open = false;
        } else {
            self.quick_show_page();
        }
    }

    /// A page shown for the first time is created: its `.set` category is
    /// got-or-created (`0x48dc80`) and loaded into it (`0x48b690`).
    pub fn quick_show_page(&mut self) {
        let k = page_index(self.analysis_tab).unwrap_or(5);
        if self.quick_shown[k] {
            return;
        }
        self.quick_shown[k] = true;
        let Some(d) = self.active.and_then(|i| self.docs.get_mut(i)) else { return };
        let items: Vec<String> = d.osc.analog.iter().map(|c| c.id.clone()).chain(d.computed.iter().map(|c| c.name.clone())).collect();
        let sec = d.set.tree.section_mut(PAGE_KEYS[k]);
        if k < 5 {
            load_page(k, &mut self.quick[k], sec, &items);
        }
    }

    /// Every created page saves (OnOK `0x48b710` / OnCancel `0x48b770`);
    /// the record is not marked modified.
    fn quick_save_pages(&mut self) {
        let Some(d) = self.active.and_then(|i| self.docs.get_mut(i)) else { return };
        for k in (0..5).filter(|&k| self.quick_shown[k]) {
            save_page(k, &self.quick[k], d.set.tree.section_mut(PAGE_KEYS[k]));
        }
    }

    /// «Рассчитать» (`0x48b710` → `0x48c0e0`): the active page's channels —
    /// on the formula editor its kill-active, then its list applied — then
    /// every created page saves and the sheet closes.
    pub fn quick_calc(&mut self) {
        if let Some(i) = page_index(self.analysis_tab) {
            let taken: Vec<String> = self.active_doc().map(|d| fvb_core::quick::taken_names(&d.osc, d.computed.iter().map(|c| c.name.as_str()))).unwrap_or_default();
            let g = self.quick[i].generate(self.analysis_tab, &taken);
            self.apply_quick(&g);
        } else {
            self.fp_kill_active();
            self.fp_apply();
        }
        self.quick_save_pages();
        self.quick_open = false;
    }

    /// The form of the active quick-analysis page.
    pub fn quick_tab_ui(&mut self, ui: &mut egui::Ui) {
        self.quick_defaults();
        let Some(d) = self.active_doc() else { return };
        let names: Vec<String> =
            d.osc.analog.iter().map(|c| c.id.clone()).chain(d.computed.iter().map(|c| c.name.clone())).collect();
        let tab = self.analysis_tab;
        let Some(i) = page_index(tab) else { return };
        let f = &mut self.quick[i];
        match tab {
            AnalysisTab::Symmetric => symmetric_page(ui, f, &names, false),
            AnalysisTab::PhasePhase => symmetric_page(ui, f, &names, true),
            AnalysisTab::Zero => zero_page(ui, f, &names),
            AnalysisTab::Phase => phase_page(ui, f, &names),
            AnalysisTab::Differential => differential_page(ui, f, &names),
            AnalysisTab::Formula => {}
        }
    }
}

/// Pages 536 «Симметричные составляющие» and (`interphase`) 538
/// «Междуфазные сопротивления»: «Токи» (Ia / Ib / Ic, «Рассчитать Ib по Ia
/// и Iс *» greying Ib `0x487d70`; 538: «Iтр, А:») and «Напряжения» (Фазные /
/// Линейные radios enabling Ua–Uc or Uab / Ubc `0x487de0`); on 536 each
/// group's «Отключить» greys its block and the other group's check
/// (`0x48c870`, `0x48cb60`).
fn symmetric_page(ui: &mut egui::Ui, f: &mut QuickForms, names: &[String], interphase: bool) {
    let (i_off, u_off) = if interphase { (false, false) } else { (f.currents_off, f.voltages_off) };
    let off_i = (!interphase).then_some((&mut f.currents_off, !u_off));
    group(ui, "Токи", off_i, |ui| {
        ui.add_enabled_ui(!i_off, |ui| {
            egui::Grid::new("q_currents").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                cell_combo(ui, true, "q_ia", "Ia", &mut f.ia, names);
                ui.end_row();
                cell_combo(ui, !f.calc_ib, "q_ib", "Ib", &mut f.ib, names);
                ui.checkbox(&mut f.calc_ib, "Рассчитать Ib по Ia и Iс *");
                ui.end_row();
                cell_combo(ui, true, "q_ic", "Ic", &mut f.ic, names);
                ui.end_row();
                if interphase {
                    ui.horizontal(|ui| {
                        ui.label("Iтр, А:");
                        ui.add(egui::TextEdit::singleline(&mut f.itr).desired_width(140.0));
                    });
                    ui.label("- ток точной работы");
                    ui.end_row();
                }
            });
            ui.label("* - только в сетях с изолированной нейтралью");
        });
    });
    ui.add_space(6.0);
    let off_u = (!interphase).then_some((&mut f.voltages_off, !i_off));
    group(ui, "Напряжения", off_u, |ui| {
        ui.add_enabled_ui(!u_off, |ui| {
            egui::Grid::new("q_voltages").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                ui.radio_value(&mut f.line_voltages, false, "Фазные напряжения");
                ui.radio_value(&mut f.line_voltages, true, "Линейные напряжения");
                ui.end_row();
                let line = f.line_voltages;
                let (ua, ub, uc, uab, ubc) = (&mut f.ua, &mut f.ub, &mut f.uc, &mut f.uab, &mut f.ubc);
                for (k, (u, l)) in [(ua, "Ua"), (ub, "Ub"), (uc, "Uc")].into_iter().enumerate() {
                    cell_combo(ui, !line, &format!("q_u{k}"), l, u, names);
                    match k {
                        0 => cell_combo(ui, line, "q_uab", "Uab", uab, names),
                        1 => cell_combo(ui, line, "q_ubc", "Ubc", ubc, names),
                        _ => {}
                    }
                    ui.end_row();
                }
            });
        });
    });
}

/// Page 537 «Нулевая последовательность»: «Ток 3Io» (Расчетный → Ia–Ic,
/// Измеренный → «3Io:», `0x488f50`) and «Напряжение 3Uo» (из Ua, Ub, Uc /
/// из Uни, Uик / Измеренный, each enabling its own combos, `0x4890b0`); the
/// «Отключить» pair as on 536 (`0x489270`, `0x489510`).
fn zero_page(ui: &mut egui::Ui, f: &mut QuickForms, names: &[String]) {
    let (i_off, u_off) = (f.zero_i_off, f.zero_u_off);
    group(ui, "Ток 3Io", Some((&mut f.zero_i_off, !u_off)), |ui| {
        ui.add_enabled_ui(!i_off, |ui| {
            egui::Grid::new("q_zero_i").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                ui.radio_value(&mut f.zero_i, 1, "Расчетный");
                ui.radio_value(&mut f.zero_i, 2, "Измеренный");
                ui.end_row();
                let calc = f.zero_i != 2;
                cell_combo(ui, calc, "q_zia", "Ia", &mut f.ia, names);
                cell_combo(ui, !calc, "q_z3io", "3Io:", &mut f.i3o, names);
                ui.end_row();
                cell_combo(ui, calc, "q_zib", "Ib", &mut f.ib, names);
                ui.end_row();
                cell_combo(ui, calc, "q_zic", "Ic", &mut f.ic, names);
                ui.end_row();
            });
        });
    });
    ui.add_space(6.0);
    group(ui, "Напряжение 3Uo", Some((&mut f.zero_u_off, !i_off)), |ui| {
        ui.add_enabled_ui(!u_off, |ui| {
            egui::Grid::new("q_zero_u").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                ui.radio_value(&mut f.zero_u, 1, "Расчитанный из Ua, Ub, Uc");
                ui.radio_value(&mut f.zero_u, 2, "Расчитанный из Uни, Uик");
                ui.end_row();
                let m = f.zero_u;
                cell_combo(ui, m == 1, "q_zua", "Ua", &mut f.ua, names);
                cell_combo(ui, m == 2, "q_zuni", "Uни", &mut f.uni, names);
                ui.end_row();
                cell_combo(ui, m == 1, "q_zub", "Ub", &mut f.ub, names);
                cell_combo(ui, m == 2, "q_zuik", "Uик", &mut f.uik, names);
                ui.end_row();
                cell_combo(ui, m == 1, "q_zuc", "Uc", &mut f.uc, names);
                ui.radio_value(&mut f.zero_u, 3, "Измеренный");
                ui.end_row();
                ui.label("");
                cell_combo(ui, m == 3, "q_z3uo", "3Uo:", &mut f.u3o, names);
                ui.end_row();
            });
        });
    });
}

/// Page 543 «Фазные сопротивления»: «Токи» (Ia–Ic, «Iтр, А:»), «Ток 3Io*»
/// (По удельным параметрам → X1 / R1 / X0 / R0; По коэффициентам компенсации
/// → K0 / Ф0, `0x48a910`), «Ток 3Io параллельной линии» («3Ion:» with
/// «Не учитывать» first; its edits — Xm / Rm or Km / Фm by the radio — only
/// with a channel, `0x48ab80`) and «Напряжения» (Ua–Uc).
fn phase_page(ui: &mut egui::Ui, f: &mut QuickForms, names: &[String]) {
    use fvb_core::addformula::NOT_COUNTED;
    group(ui, "Токи", None, |ui| {
        egui::Grid::new("q_ph_i").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
            cell_combo(ui, true, "q_pia", "Ia", &mut f.ia, names);
            ui.end_row();
            cell_combo(ui, true, "q_pib", "Ib", &mut f.ib, names);
            ui.end_row();
            cell_combo(ui, true, "q_pic", "Ic", &mut f.ic, names);
            ui.end_row();
            cell_edit(ui, true, "Iтр, А:", &mut f.itr);
            ui.end_row();
        });
    });
    ui.add_space(6.0);
    group(ui, "Ток 3Io*", None, |ui| {
        ui.label("Расчет коэффициента компенсации тока 3Io");
        egui::Grid::new("q_ph_c").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
            ui.radio_value(&mut f.per_unit, true, "По удельным параметрам");
            ui.radio_value(&mut f.per_unit, false, "По коэффициентам компенсации");
            ui.end_row();
            let pu = f.per_unit;
            cell_edit(ui, pu, "X1, Ом/км", &mut f.x1);
            cell_edit(ui, !pu, "K0", &mut f.k0);
            ui.end_row();
            cell_edit(ui, pu, "R1, Ом/км", &mut f.r1);
            cell_edit(ui, !pu, "Ф0, град", &mut f.f0);
            ui.end_row();
            cell_edit(ui, pu, "X0, Ом/км", &mut f.x0);
            ui.end_row();
            cell_edit(ui, pu, "R0, Ом/км", &mut f.r0);
            ui.end_row();
        });
        ui.label("* Ток нулевой последовательности рассчитывается из фазных токов");
    });
    ui.add_space(6.0);
    group(ui, "Ток 3Io параллельной линии", None, |ui| {
        let mut items = vec![NOT_COUNTED.to_string()];
        items.extend(names.iter().cloned());
        if f.i0_parallel.is_empty() {
            f.i0_parallel = NOT_COUNTED.into();
        }
        let used = f.parallel_used();
        let pu = f.per_unit;
        egui::Grid::new("q_ph_m").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
            cell_combo(ui, true, "q_p3ion", "3Ion:", &mut f.i0_parallel, &items);
            ui.end_row();
            cell_edit(ui, used && pu, "Xm, Ом/км", &mut f.xm);
            cell_edit(ui, used && !pu, "Km", &mut f.km);
            ui.end_row();
            cell_edit(ui, used && pu, "Rm, Ом/км", &mut f.rm);
            cell_edit(ui, used && !pu, "Фm, град", &mut f.fm);
            ui.end_row();
        });
    });
    ui.add_space(6.0);
    group(ui, "Напряжения", None, |ui| {
        egui::Grid::new("q_ph_u").num_columns(1).spacing([8.0, 6.0]).show(ui, |ui| {
            cell_combo(ui, true, "q_pua", "Ua", &mut f.ua, names);
            ui.end_row();
            cell_combo(ui, true, "q_pub", "Ub", &mut f.ub, names);
            ui.end_row();
            cell_combo(ui, true, "q_puc", "Uc", &mut f.uc, names);
            ui.end_row();
        });
    });
}

/// Page 539 «Дифференциальные токи»: arm and phase counts; per arm (kept
/// per arm) «Приведение плеч» (its radio enables only its edit block,
/// `0x4835b0`…) and «Учет группы соединений» (3 phases only, `0x482c10`:
/// По последовательностям → «Группа» 0…11 + «Удалить I0», Цифровой
/// треугольник → {0, 1, 11}, Не учитывать → 0…11 greyed, `0x483ab0`); «<<» /
/// «Текущее плечо:» / «>>» (greyed at the ends, `0x4831b0`); the arm's group
/// «Плечо %d» (string 198); «Расчет тока торможения» — «Полуразность токов»
/// only with 2 arms (else forced to «Полусумма», `0x482b60`), «Kторм:» {1, 0.5}
/// for Полусумма or Полуразность with 2 arms (`0x483da0`).
fn differential_page(ui: &mut egui::Ui, f: &mut QuickForms, names: &[String]) {
    ui.horizontal(|ui| {
        ui.label("Количество плеч:");
        // A changed count keeps every slot (`0x482b60` → `0x482a40`).
        egui::ComboBox::from_id_salt("q_dif_n").selected_text(f.dif_arms.to_string()).width(50.0).show_ui(ui, |ui| {
            for k in 2..=ARM_SLOTS {
                ui.selectable_value(&mut f.dif_arms, k, k.to_string());
            }
        });
        ui.add_space(30.0);
        ui.label("Количество фаз:");
        egui::ComboBox::from_id_salt("q_dif_ph").selected_text(f.dif_phases.to_string()).width(50.0).show_ui(ui, |ui| {
            for k in [1, 3] {
                ui.selectable_value(&mut f.dif_phases, k, k.to_string());
            }
        });
    });
    let n = f.dif_arms;
    if n > 2 && f.braking == 1 {
        f.braking = 0;
    }
    f.cur_arm = f.cur_arm.min(n - 1);
    let three = f.dif_phases == 3;
    let cur = f.cur_arm;
    group(ui, "Приведение плеч", None, |ui| {
        egui::Grid::new("q_dif_red").num_columns(2).show(ui, |ui| {
            let checked = f.dif_methods();
            let mut radio = |ui: &mut egui::Ui, m: u8, text: &str| {
                if ui.radio(checked.contains(&m), text).clicked() {
                    f.dif_reduction = m;
                    f.dif_also.clear();
                }
            };
            radio(ui, 2, "По мощности");
            radio(ui, 0, "По коэффициенту приведения");
            ui.end_row();
            radio(ui, 3, "По номинальному току");
            radio(ui, 1, "По коэффициенту трансформации ТТ");
            ui.end_row();
        });
    });
    group(ui, "Учет группы соединений", None, |ui| {
        ui.add_enabled_ui(three, |ui| {
            ui.horizontal(|ui| {
                ui.radio_value(&mut f.dif_group, 0, "Не учитывать");
                ui.radio_value(&mut f.dif_group, 1, "По последовательностям");
                ui.radio_value(&mut f.dif_group, 2, "Цифровой треугольник");
            });
        });
    });
    let (r, grp) = (f.dif_edit_method(), f.dif_group);
    ui.horizontal(|ui| {
        if ui.add_enabled(cur > 0, egui::Button::new("<<")).clicked() {
            f.cur_arm -= 1;
        }
        ui.label("Текущее плечо:");
        egui::ComboBox::from_id_salt("q_dif_cur").selected_text((cur + 1).to_string()).width(50.0).show_ui(ui, |ui| {
            for k in 0..n {
                ui.selectable_value(&mut f.cur_arm, k, (k + 1).to_string());
            }
        });
        if ui.add_enabled(cur + 1 < n, egui::Button::new(">>")).clicked() {
            f.cur_arm += 1;
        }
    });
    let arm = &mut f.arms[cur];
    group(ui, &format!("Плечо {}", cur + 1), None, |ui| {
        egui::Grid::new("q_dif_arm").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
            let [ia, ib, ic] = &mut arm.currents;
            cell_combo(ui, true, "q_dia", "Ia", ia, names);
            ui.end_row();
            cell_combo(ui, three, "q_dib", "Ib", ib, names);
            ui.end_row();
            cell_combo(ui, three, "q_dic", "Ic", ic, names);
            ui.end_row();
            ui.label("По мощности:");
            ui.end_row();
            cell_edit(ui, r == 2, "Kтт", &mut arm.pw_ktt);
            cell_edit(ui, r == 2, "Sном, кВА", &mut arm.snom);
            ui.end_row();
            cell_edit(ui, r == 2, "Uном, кВ", &mut arm.unom);
            ui.end_row();
            ui.label("По номинальному току:");
            ui.end_row();
            cell_edit(ui, r == 3, "Kтт", &mut arm.amp_ktt);
            cell_edit(ui, r == 3, "Iном, А", &mut arm.inom);
            ui.end_row();
            ui.label("По коэффициенту приведения:");
            ui.end_row();
            cell_edit(ui, r == 0, "Kприв", &mut arm.kpriv);
            ui.end_row();
            ui.label("По коэффициенту трансформации ТТ:");
            ui.end_row();
            cell_edit(ui, r == 1, "Kтт", &mut arm.tt_ktt);
            cell_edit(ui, r == 1, "Kттбаз", &mut arm.tt_base);
            ui.end_row();
            // Two «Группа:» combos at one place, one per method.
            // With one phase the sequence combo shows, whatever the radio
            // (`0x483ab0`).
            let (groups, value): (&[&str], &mut String) = if three && grp == 2 {
                (&["0", "1", "11"], &mut arm.tri_group)
            } else {
                (&["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11"], &mut arm.seq_group)
            };
            ui.add_enabled_ui(three && grp != 0, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Группа:");
                    egui::ComboBox::from_id_salt(("q_dif_grp", grp)).selected_text(value.as_str()).width(50.0).show_ui(ui, |ui| {
                        for g in groups {
                            ui.selectable_value(value, g.to_string(), *g);
                        }
                    });
                });
            });
            ui.add_enabled(three && grp == 1, egui::Checkbox::new(&mut arm.remove_i0, "Удалить I0"));
            ui.end_row();
        });
    });
    group(ui, "Расчет тока торможения", None, |ui| {
        ui.horizontal(|ui| {
            ui.radio_value(&mut f.braking, 0, "Полусумма модулей токов");
            ui.add_enabled_ui(n == 2, |ui| ui.radio_value(&mut f.braking, 1, "Полуразность токов"));
            ui.radio_value(&mut f.braking, 2, "Максимальный из токов");
        });
        let kt = f.braking == 0 || (f.braking == 1 && n == 2);
        ui.add_enabled_ui(kt, |ui| {
            ui.horizontal(|ui| {
                ui.label("Kторм:");
                egui::ComboBox::from_id_salt("q_dif_kt").selected_text(f.k_torm.as_str()).width(50.0).show_ui(ui, |ui| {
                    for k in ["1", "0.5"] {
                        ui.selectable_value(&mut f.k_torm, k.to_string(), k);
                    }
                });
            });
        });
    });
}

/// The `.set` category of each page (`0x48b690` → vslot 0x1dc), in
/// [`page_index`] order, then the formula editor's.
pub const PAGE_KEYS: [&str; 6] = ["SymmetricItemsPage", "NullSequencePage", "InterPhaseResistPage", "PhaseResistPage", "DiffCurrentsPage", "FormulaEditorPage"];

/// Combo load (`0x414780`): a non-empty stored text that the list has
/// (case-insensitively) selects the first item STARTING with it (the
/// combo's prefix search); anything else leaves the selection.
fn load_text(stored: Option<&String>, items: &[String], slot: &mut String) {
    let Some(v) = stored.filter(|v| !v.is_empty()) else { return };
    let lv = v.to_lowercase();
    if items.iter().any(|n| n.to_lowercase() == lv) {
        if let Some(n) = items.iter().find(|n| n.to_lowercase().starts_with(&lv)) {
            slot.clone_from(n);
        }
    }
}

/// A channel combo kept in a child node `key` as `channelName` (flag 1;
/// the child is created on load too).
fn load_chan(sec: &mut SetNode, key: &str, items: &[String], slot: &mut String) {
    let c = sec.child_mut(key);
    load_text(c.attrs.get("channelName"), items, slot);
}

fn save_chan(sec: &mut SetNode, key: &str, slot: &str) {
    sec.child_mut(key).set_str("channelName", slot);
}

/// An edit (`0x4149f0`): only a non-empty stored text is applied.
fn load_edit(sec: &SetNode, key: &str, slot: &mut String) {
    if let Some(v) = sec.attrs.get(key).filter(|v| !v.is_empty()) {
        slot.clone_from(v);
    }
}

/// A check / radio: `%i`, missing → `None` (the control is left alone).
fn load_check(sec: &SetNode, key: &str) -> Option<bool> {
    sec.attrs.get(key).map(|v| fvb_core::recset::wtoi(v) != 0)
}

fn save_check(sec: &mut SetNode, key: &str, on: bool) {
    sec.set_int(key, i32::from(on));
}

const CHAN_KEYS: [&str; 8] = ["IA", "IB", "IC", "UA", "UB", "UC", "UAB", "UBC"];

fn chan_slots(f: &mut QuickForms) -> [&mut String; 8] {
    [&mut f.ia, &mut f.ib, &mut f.ic, &mut f.ua, &mut f.ub, &mut f.uc, &mut f.uab, &mut f.ubc]
}

/// Page `i`'s load from its `.set` category (vslot 0x1d4: 536 `0x48ce30`,
/// 537 `0x4897f0`, 538 `0x487f50`, 543 `0x48ac90`, 539 `0x4841c0`), with
/// the forced radio defaults when none is stored.
pub fn load_page(i: usize, f: &mut QuickForms, sec: &mut SetNode, items: &[String]) {
    match i {
        0 | 2 => {
            if i == 2 {
                load_edit(sec, "itr", &mut f.itr);
            }
            let keys = CHAN_KEYS;
            for (k, slot) in keys.iter().zip(chan_slots(f)) {
                load_chan(sec, k, items, slot);
            }
            if i == 0 {
                if let Some(v) = load_check(sec, "currOffState") {
                    f.currents_off = v;
                }
                if let Some(v) = load_check(sec, "voltOffState") {
                    f.voltages_off = v;
                }
            }
            if let Some(v) = load_check(sec, "calcIBState") {
                f.calc_ib = v;
            }
            let phase = load_check(sec, "phaseVoltState") == Some(true);
            let line = load_check(sec, "lineVoltState") == Some(true);
            f.line_voltages = !phase && line;
        }
        1 => {
            for (k, slot) in CHAN_KEYS[..6].iter().zip(chan_slots(f)) {
                load_chan(sec, k, items, slot);
            }
            load_chan(sec, "3I0_0", items, &mut f.i3o);
            load_chan(sec, "UNI", items, &mut f.uni);
            load_chan(sec, "UIK", items, &mut f.uik);
            load_chan(sec, "3U0", items, &mut f.u3o);
            f.zero_i = if load_check(sec, "measuredCurrState") == Some(true) && load_check(sec, "calcCurrState") != Some(true) { 2 } else { 1 };
            f.zero_u = if load_check(sec, "calcUABCState") == Some(true) {
                1
            } else if load_check(sec, "calcUniUicState") == Some(true) {
                2
            } else if load_check(sec, "measuredVoltState") == Some(true) {
                3
            } else {
                1
            };
            if let Some(v) = load_check(sec, "currOffState") {
                f.zero_i_off = v;
            }
            if let Some(v) = load_check(sec, "voltOffState") {
                f.zero_u_off = v;
            }
        }
        3 => {
            for (k, slot) in [("R0", &mut f.r0), ("R1", &mut f.r1), ("X0", &mut f.x0), ("X1", &mut f.x1), ("K0", &mut f.k0), ("F0", &mut f.f0), ("RM", &mut f.rm), ("XM", &mut f.xm), ("KM", &mut f.km), ("FM", &mut f.fm), ("ITR", &mut f.itr)] {
                load_edit(sec, k, slot);
            }
            for (k, slot) in CHAN_KEYS[..6].iter().zip(chan_slots(f)) {
                load_chan(sec, k, items, slot);
            }
            let mut with_none = vec![fvb_core::addformula::NOT_COUNTED.to_string()];
            with_none.extend(items.iter().cloned());
            load_chan(sec, "3I0n", &with_none, &mut f.i0_parallel);
            f.per_unit = !(load_check(sec, "compensateState") == Some(true) && load_check(sec, "specificState") != Some(true));
        }
        _ => load_diff(f, sec, items),
    }
}

/// Page `i`'s save (vslot 0x1d8, on «Рассчитать» and «Отмена» alike; the
/// record is not marked modified): every key, `""` for an empty combo.
pub fn save_page(i: usize, f: &QuickForms, sec: &mut SetNode) {
    let chans = [&f.ia, &f.ib, &f.ic, &f.ua, &f.ub, &f.uc, &f.uab, &f.ubc];
    match i {
        0 | 2 => {
            if i == 2 {
                sec.set_str("itr", &f.itr);
            }
            for (k, v) in CHAN_KEYS.iter().zip(chans) {
                save_chan(sec, k, v);
            }
            if i == 0 {
                save_check(sec, "currOffState", f.currents_off);
                save_check(sec, "voltOffState", f.voltages_off);
            }
            save_check(sec, "calcIBState", f.calc_ib);
            save_check(sec, "phaseVoltState", !f.line_voltages);
            save_check(sec, "lineVoltState", f.line_voltages);
        }
        1 => {
            for (k, v) in CHAN_KEYS[..6].iter().zip(chans) {
                save_chan(sec, k, v);
            }
            for (k, v) in [("3I0_0", &f.i3o), ("UNI", &f.uni), ("UIK", &f.uik), ("3U0", &f.u3o)] {
                save_chan(sec, k, v);
            }
            save_check(sec, "calcCurrState", f.zero_i != 2);
            save_check(sec, "measuredCurrState", f.zero_i == 2);
            save_check(sec, "calcUABCState", f.zero_u == 1);
            save_check(sec, "calcUniUicState", f.zero_u == 2);
            save_check(sec, "measuredVoltState", f.zero_u == 3);
            save_check(sec, "currOffState", f.zero_i_off);
            save_check(sec, "voltOffState", f.zero_u_off);
        }
        3 => {
            for (k, v) in [("R0", &f.r0), ("R1", &f.r1), ("X0", &f.x0), ("X1", &f.x1), ("K0", &f.k0), ("F0", &f.f0), ("RM", &f.rm), ("XM", &f.xm), ("KM", &f.km), ("FM", &f.fm), ("ITR", &f.itr)] {
                sec.set_str(k, v);
            }
            for (k, v) in CHAN_KEYS[..6].iter().zip(chans) {
                save_chan(sec, k, v);
            }
            save_chan(sec, "3I0n", &f.i0_parallel);
            save_check(sec, "specificState", f.per_unit);
            save_check(sec, "compensateState", !f.per_unit);
        }
        _ => save_diff(f, sec),
    }
}

/// An arm's edits in a `Leg_%d` node, by the controls the keys name
/// (DIALOG 539: R0 Kприв, R1 Kттбаз, R2 Kтт (ТТ), R3 Kтт (мощность), R4 Kтт
/// (ном. ток), R5 Sном, R6 Iном, X0 Uном).
fn leg_edits(a: &mut ArmForm) -> [(&'static str, &mut String); 8] {
    [
        ("R0", &mut a.kpriv),
        ("R1", &mut a.tt_base),
        ("R2", &mut a.tt_ktt),
        ("R3", &mut a.pw_ktt),
        ("R4", &mut a.amp_ktt),
        ("R5", &mut a.snom),
        ("R6", &mut a.inom),
        ("X0", &mut a.unom),
    ]
}

const REDUCTION_KEYS: [(&str, u8); 4] = [("byPowerState", 2), ("byNomCurState", 3), ("byCastCoeffState", 0), ("byTransCoeffState", 1)];
const GROUP_KEYS: [(&str, u8); 3] = [("doNotConsider", 0), ("bySequence", 1), ("byTriangle", 2)];
const BRAKE_KEYS: [(&str, u8); 3] = [("bySumState", 0), ("bySubState", 1), ("byMaxState", 2)];

/// The first stored-and-set radio of a group, else the forced first one.
fn load_radio(sec: &mut SetNode, keys: &[(&str, u8)]) -> u8 {
    keys.iter().find(|(k, _)| load_check(sec, k) == Some(true)).map_or(keys[0].1, |&(_, v)| v)
}

/// An arm slot as the page's load fills all 24 (`0x4857a0`) from the
/// controls' initial state: the edits «1.» (`0x411f40`), groups «0», «Удалить
/// I0» on, the current combos preselected by name, case-insensitively
/// («Ia», «Ib», «Ic», `0x411c70`).
fn initial_arm(items: &[String]) -> ArmForm {
    let named = |want: &str| items.iter().find(|n| n.to_lowercase() == want).cloned().unwrap_or_default();
    ArmForm { currents: [named("ia"), named("ib"), named("ic")], ..Default::default() }
}

/// 539 (`0x4841c0`): the three option groups (`legOption`,
/// `groupConnOption`, `brakeCurrOption`; none set → «По мощности», «Не
/// учитывать»), the counts and Kторм as item texts, every slot set to the
/// initial arm, then each `Leg_%d` over its own slot — a missing value
/// keeps the initial one; the last arm is the one shown.
fn load_diff(f: &mut QuickForms, sec: &mut SetNode, items: &[String]) {
    // Each stored key checks its radio on its own (`0x4844a0`); none
    // checked → «По мощности».
    let opt = sec.child_mut("legOption");
    let on: Vec<u8> = REDUCTION_KEYS.iter().filter(|(k, _)| load_check(opt, k) == Some(true)).map(|&(_, v)| v).collect();
    f.dif_reduction = on.first().copied().unwrap_or(REDUCTION_KEYS[0].1);
    f.dif_also = on.iter().skip(1).copied().collect();
    f.dif_group = load_radio(sec.child_mut("groupConnOption"), &GROUP_KEYS);
    f.braking = load_radio(sec.child_mut("brakeCurrOption"), &BRAKE_KEYS);
    let counts: Vec<String> = (2..=24).map(|k| k.to_string()).collect();
    let mut n = f.dif_arms.to_string();
    load_text(sec.attrs.get("legCount"), &counts, &mut n);
    let mut ph = f.dif_phases.to_string();
    load_text(sec.attrs.get("phaseCount"), &["1".into(), "3".into()], &mut ph);
    f.dif_phases = ph.parse().unwrap_or(3);
    load_text(sec.attrs.get("brakeCoeff"), &["1".into(), "0.5".into()], &mut f.k_torm);
    f.dif_arms = n.parse().unwrap_or(2);
    f.arms = vec![initial_arm(items); ARM_SLOTS];
    let seq: Vec<String> = (0..12).map(|k| k.to_string()).collect();
    let tri: Vec<String> = ["0", "1", "11"].iter().map(|s| s.to_string()).collect();
    for i in 0..f.dif_arms {
        let leg = sec.child_mut(&format!("Leg_{i}"));
        let arm = &mut f.arms[i];
        let [ia, ib, ic] = &mut arm.currents;
        for (k, slot) in [("IA", ia), ("IB", ib), ("IC", ic)] {
            load_text(leg.attrs.get(k), items, slot);
        }
        for (k, slot) in leg_edits(arm) {
            load_edit(leg, k, slot);
        }
        load_text(leg.attrs.get("seqGroup"), &seq, &mut arm.seq_group);
        load_text(leg.attrs.get("triangleGroup"), &tri, &mut arm.tri_group);
        if let Some(v) = load_check(leg, "I0State") {
            arm.remove_i0 = v;
        }
    }
    f.cur_arm = f.dif_arms - 1;
}

/// 539's save (`0x484330`): the page's options, the counts, then every
/// arm's `Leg_%d` with both group numbers (higher `Leg_` nodes of a former,
/// larger count stay).
fn save_diff(f: &QuickForms, sec: &mut SetNode) {
    let methods = f.dif_methods();
    let c = sec.child_mut("legOption");
    for (k, kv) in REDUCTION_KEYS {
        save_check(c, k, methods.contains(&kv));
    }
    for (node, keys, v) in [("groupConnOption", &GROUP_KEYS[..], f.dif_group), ("brakeCurrOption", &BRAKE_KEYS[..], f.braking)] {
        let c = sec.child_mut(node);
        for (k, kv) in keys {
            save_check(c, k, *kv == v);
        }
    }
    sec.set_str("legCount", &f.dif_arms.to_string());
    sec.set_str("phaseCount", &f.dif_phases.to_string());
    sec.set_str("brakeCoeff", &f.k_torm);
    for (i, arm) in f.arms[..f.dif_arms].iter().enumerate() {
        let mut a = arm.clone();
        let leg = sec.child_mut(&format!("Leg_{i}"));
        let [ia, ib, ic] = &a.currents;
        for (k, v) in [("IA", ia), ("IB", ib), ("IC", ic)] {
            leg.set_str(k, v);
        }
        leg.set_str("seqGroup", &a.seq_group);
        leg.set_str("triangleGroup", &a.tri_group);
        save_check(leg, "I0State", a.remove_i0);
        for (k, v) in leg_edits(&mut a) {
            leg.set_str(k, v);
        }
    }
}

/// The form of a generator page: each page keeps its own fields, as the
/// original's pages do (their own controls and `.set` categories).
pub fn page_index(tab: AnalysisTab) -> Option<usize> {
    match tab {
        AnalysisTab::Symmetric => Some(0),
        AnalysisTab::Zero => Some(1),
        AnalysisTab::PhasePhase => Some(2),
        AnalysisTab::Phase => Some(3),
        AnalysisTab::Differential => Some(4),
        AnalysisTab::Formula => None,
    }
}

/// The sheet's pages in the original's order (AddPage ×6, `0x48b970`):
/// tab, our glyph, the page dialog's caption.
const PAGES: [(AnalysisTab, &str, &str); 6] = [
    (AnalysisTab::Symmetric, "I₁ I₂", "Симметричные составляющие"),
    (AnalysisTab::Zero, "3I₀", "Нулевая последовательность"),
    (AnalysisTab::PhasePhase, "Zab", "Междуфазные сопротивления"),
    (AnalysisTab::Phase, "Za", "Фазные сопротивления"),
    (AnalysisTab::Differential, "Iдиф", "Дифференциальные токи"),
    (AnalysisTab::Formula, "f(x)", "Редактор формул"),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// The `.set` contract of 536: channel combos as `<IA channelName=…>`
    /// children, checks as `%i`; a stored name loads only when the list has
    /// it, and then selects the first item starting with it.
    #[test]
    fn symmetric_page_round_trips_through_the_set() {
        let items: Vec<String> = ["Ia1", "Ia", "Ib", "Ic", "Ua", "Ub", "Uc"].iter().map(|s| s.to_string()).collect();
        let mut f = QuickForms { ia: "Ia".into(), ib: "Ib".into(), line_voltages: true, voltages_off: true, ..Default::default() };
        let mut sec = SetNode::default();
        save_page(0, &f, &mut sec);
        assert_eq!(sec.child("IA").unwrap().attrs["channelName"], "Ia");
        assert_eq!(sec.child("IC").unwrap().attrs["channelName"], "");
        assert_eq!((sec.attrs["voltOffState"].as_str(), sec.attrs["lineVoltState"].as_str(), sec.attrs["phaseVoltState"].as_str()), ("1", "1", "0"));
        let mut g = QuickForms::default();
        load_page(0, &mut g, &mut sec, &items);
        assert_eq!(g.ia, "Ia1", "the combo's prefix search picks the first item starting with «Ia»");
        assert_eq!(g.ib, "Ib");
        assert!(g.line_voltages && g.voltages_off && !g.currents_off);
        // Nothing stored: the phase radio is forced, a name the record lacks is ignored.
        let mut empty = SetNode::default();
        empty.child_mut("UA").set_str("channelName", "Нет такого");
        f = QuickForms { ua: "Ua".into(), line_voltages: true, ..Default::default() };
        load_page(0, &mut f, &mut empty, &items);
        assert!(!f.line_voltages);
        assert_eq!(f.ua, "Ua");
    }

    /// 539: each `Leg_%d` over its own slot — a missing leg keeps the
    /// initial arm (`0x4857a0`); the edits by their control's key; the
    /// methods page-wide, two group numbers per arm; the last arm shown.
    #[test]
    fn differential_page_legs_load_over_initial_arms() {
        let items: Vec<String> = ["I1a", "I2a", "IA", "ib"].iter().map(|s| s.to_string()).collect();
        let mut f = QuickForms::default();
        f.arms[0].currents[0] = "I1a".into();
        f.arms[0].kpriv = "2.5".into();
        f.arms[0].seq_group = "5".into();
        f.arms[0].tri_group = "11".into();
        f.arms[1].currents[0] = "I2a".into();
        f.braking = 2;
        f.dif_group = 2;
        let mut sec = SetNode::default();
        save_page(4, &f, &mut sec);
        let leg0 = sec.child("Leg_0").unwrap();
        assert_eq!(leg0.attrs["R0"], "2.5", "R0 = Kприв");
        assert_eq!((leg0.attrs["seqGroup"].as_str(), leg0.attrs["triangleGroup"].as_str()), ("5", "11"));
        assert_eq!(sec.attrs["legCount"], "2");
        assert!(sec.child("Leg_2").is_none());
        sec.set_str("legCount", "3");
        let mut g = QuickForms::default();
        load_page(4, &mut g, &mut sec, &items);
        assert_eq!((g.dif_arms, g.arms.len(), g.cur_arm), (3, ARM_SLOTS, 2));
        assert_eq!(g.arms[1].currents[0], "I2a");
        assert_eq!(g.arms[2].currents, ["IA".to_string(), "ib".into(), String::new()], "Leg_2 missing: the initial arm");
        assert_eq!(g.arms[2].kpriv, "1.");
        assert_eq!((g.arms[0].kpriv.as_str(), g.arms[0].seq_group.as_str(), g.arms[0].tri_group.as_str()), ("2.5", "5", "11"));
        assert_eq!((g.braking, g.dif_group, g.dif_reduction), (2, 2, 2), "no reduction key: «По мощности»");
    }

    /// A `.set` with two method keys (`0x4844a0`): both radios checked,
    /// the last enabler's block (power, nominal, cast, TT order) editable,
    /// both reductions emitted in the generation's order (`0x47ff60`).
    #[test]
    fn two_stored_methods_both_reduce() {
        let mut sec = SetNode::default();
        let opt = sec.child_mut("legOption");
        opt.set_int("byPowerState", 1);
        opt.set_int("byCastCoeffState", 1);
        let mut f = QuickForms::default();
        load_page(4, &mut f, &mut sec, &[]);
        assert_eq!((f.dif_methods(), f.dif_edit_method()), (vec![0, 2], 0));
        let g = f.generate(AnalysisTab::Differential, &[]);
        let firsts: Vec<&str> = g.channels.iter().map(|(_, t)| t.as_str()).take(6).collect();
        assert!(firsts[0].starts_with("CoeffPriv") && firsts[2].starts_with("CoeffPriv"), "{firsts:?}");
        let n_power = g.channels.iter().filter(|(_, t)| t.starts_with("CoeffPowerPriv")).count();
        assert_eq!(n_power, 2 * 3, "every arm and phase");
        let mut back = SetNode::default();
        save_page(4, &f, &mut back);
        let o = back.child("legOption").unwrap();
        assert_eq!((o.attrs["byPowerState"].as_str(), o.attrs["byCastCoeffState"].as_str(), o.attrs["byTransCoeffState"].as_str()), ("1", "1", "0"));
    }

    /// 539's generation: the page's methods for every arm, each arm's own
    /// values and the group number of the chosen method (`0x47ff60`).
    #[test]
    fn differential_methods_are_page_wide() {
        let mut f = QuickForms { dif_reduction: 0, dif_group: 2, ..Default::default() };
        for (a, ch) in f.arms.iter_mut().zip(["A", "B"]) {
            a.currents = [format!("{ch}a"), format!("{ch}b"), format!("{ch}c")];
        }
        f.arms[0].kpriv = "2".into();
        f.arms[1].kpriv = "3".into();
        f.arms[1].tri_group = "11".into();
        f.arms[1].seq_group = "7".into();
        f.cur_arm = 1;
        let mut q = f.clone();
        q.cur_arm = 0;
        let (a, b) = (f.generate(AnalysisTab::Differential, &[]), q.generate(AnalysisTab::Differential, &[]));
        assert_eq!(a.channels, b.channels, "the shown arm does not matter");
        let all = a.channels.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>().join("\n");
        assert!(all.contains("CoeffPriv") && all.contains('3') && all.contains("11"), "{all}");
        assert!(!all.contains("GrsPosl"), "{all}");
    }

    /// «Рассчитать» greying of 536 / 538 (`0x48d5a0`, `0x488550`).
    #[test]
    fn calculate_needs_the_pages_channels() {
        let mut f = QuickForms::default();
        assert!(!f.ok_enabled(AnalysisTab::Symmetric), "no channels chosen");
        assert!(f.ok_enabled(AnalysisTab::Differential), "539 has no check");
        for (slot, v) in [(&mut f.ia, "Ia"), (&mut f.ic, "Ic"), (&mut f.ua, "Ua"), (&mut f.ub, "Ub"), (&mut f.uc, "Uc")] {
            *slot = v.into();
        }
        assert!(!f.ok_enabled(AnalysisTab::Symmetric), "Ib missing");
        f.calc_ib = true;
        assert!(f.ok_enabled(AnalysisTab::Symmetric), "Ib calculated");
        f.line_voltages = true;
        assert!(!f.ok_enabled(AnalysisTab::Symmetric), "line voltages not chosen");
        f.voltages_off = true;
        assert!(f.ok_enabled(AnalysisTab::Symmetric), "voltages switched off");
        assert!(!f.ok_enabled(AnalysisTab::PhasePhase), "538 has no «Отключить»");
        f.line_voltages = false;
        assert!(f.ok_enabled(AnalysisTab::PhasePhase));
        f.itr.clear();
        assert!(!f.ok_enabled(AnalysisTab::PhasePhase), "Iтр empty");
        // 537 (`0x48a1f0`): the radio's combos of each live group.
        let mut z = QuickForms::default();
        for (slot, v) in [(&mut z.ia, "Ia"), (&mut z.ib, "Ib"), (&mut z.ic, "Ic"), (&mut z.ua, "Ua"), (&mut z.ub, "Ub"), (&mut z.uc, "Uc")] {
            *slot = v.into();
        }
        assert!(z.ok_enabled(AnalysisTab::Zero));
        z.zero_i = 2;
        assert!(!z.ok_enabled(AnalysisTab::Zero), "3Io not chosen");
        z.zero_i_off = true;
        z.zero_u = 3;
        assert!(!z.ok_enabled(AnalysisTab::Zero), "3Uo not chosen");
        z.u3o = "3U0".into();
        assert!(z.ok_enabled(AnalysisTab::Zero));
        // 543 (`0x48b360`): the active group's edits; a parallel line's own.
        assert!(z.ok_enabled(AnalysisTab::Phase), "defaults fill every edit");
        z.x1.clear();
        assert!(!z.ok_enabled(AnalysisTab::Phase));
        z.per_unit = false;
        assert!(z.ok_enabled(AnalysisTab::Phase), "X1 is not needed by the coefficients");
        z.i0_parallel = "3I0n".into();
        z.km.clear();
        assert!(!z.ok_enabled(AnalysisTab::Phase), "Km needed with a parallel line");
        z.i0_parallel = fvb_core::addformula::NOT_COUNTED.into();
        assert!(z.ok_enabled(AnalysisTab::Phase));
    }
}
