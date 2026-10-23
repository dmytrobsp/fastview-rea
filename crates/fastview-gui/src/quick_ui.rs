//! The quick-analysis property sheet (`CQAnPropSheet`): one form per tab whose
//! OK button generates computed channels (`fvb_core::quick`).

use crate::app::{AnalysisTab, App};
use eframe::egui;
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
    pub arms: Vec<ArmForm>,
    /// 0 sum (полусумма модулей), 1 arms 1–2, 2 maximum.
    pub braking: u8,
    pub k_torm: String,
    /// 539's «Текущее плечо» (0-based).
    pub cur_arm: usize,
}

#[derive(Debug, Clone)]
pub struct ArmForm {
    pub currents: [String; 3],
    /// 0 CoeffPriv, 1 CoeffTT, 2 CoeffPowerPriv, 3 CoeffAmpPriv.
    pub reduction: u8,
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
    /// 0 none, 1 GrsPosl, 2 GrsTriangle.
    pub group: u8,
    pub group_n: String,
    pub remove_i0: bool,
}

impl Default for ArmForm {
    fn default() -> Self {
        ArmForm {
            currents: Default::default(),
            reduction: 0,
            pw_ktt: "1.".into(),
            snom: "1.".into(),
            unom: "1.".into(),
            amp_ktt: "1.".into(),
            inom: "1.".into(),
            kpriv: "1.".into(),
            tt_ktt: "1.".into(),
            tt_base: "1.".into(),
            group: 0,
            group_n: "0".into(),
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
            // `0x411f40`: Iтр «0.25», R / X / K «1.», Ф / Rm / Xm / Km / Фm «0.».
            itr: "0.25".into(),
            i0_parallel: s(),
            per_unit: true,
            r0: "1.".into(),
            r1: "1.".into(),
            rm: "0.".into(),
            x0: "1.".into(),
            x1: "1.".into(),
            xm: "0.".into(),
            k0: "1.".into(),
            km: "0.".into(),
            f0: "0.".into(),
            fm: "0.".into(),
            dif_phases: 3,
            arms: vec![ArmForm::default(), ArmForm::default()],
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
            AnalysisTab::PhasePhase => !self.itr.trim().is_empty() && currents && voltages,
            AnalysisTab::Phase => {
                let filled = |v: &[&String]| v.iter().all(|s| !s.trim().is_empty());
                let comp = if self.per_unit { filled(&[&self.x1, &self.r1, &self.x0, &self.r0]) } else { filled(&[&self.k0, &self.f0]) };
                let parallel = !self.parallel_used() || if self.per_unit { filled(&[&self.xm, &self.rm]) } else { filled(&[&self.km, &self.fm]) };
                let u = chosen(&self.ua) && chosen(&self.ub) && chosen(&self.uc);
                chosen(&self.ia) && chosen(&self.ib) && chosen(&self.ic) && u && !self.itr.trim().is_empty() && comp && parallel
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

    /// The active tab's channels.
    pub fn generate(&self, tab: AnalysisTab) -> quick::Generated {
        let i = (!self.currents_off).then(|| self.currents());
        let u = (!self.voltages_off).then(|| self.voltages());
        match tab {
            AnalysisTab::Symmetric => quick::symmetric(i.as_ref(), u.as_ref()),
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
                quick::zero(zi.as_ref(), zu.as_ref())
            }
            AnalysisTab::PhasePhase => quick::phase_phase(&self.currents(), &self.voltages(), &self.itr),
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
                quick::phase_z(&self.currents(), [&self.ua, &self.ub, &self.uc], i0n, &comp, &self.itr)
            }
            AnalysisTab::Differential => {
                let arms: Vec<Arm> = self
                    .arms
                    .iter()
                    .map(|a| Arm {
                        currents: a.currents[..self.dif_phases].to_vec(),
                        reduction: match a.reduction {
                            0 => Reduction::Priv { k: a.kpriv.clone() },
                            1 => Reduction::Tt { ktt: a.tt_ktt.clone(), ktt_base: a.tt_base.clone() },
                            2 => Reduction::PowerPriv { ktt: a.pw_ktt.clone(), snom: a.snom.clone(), unom: a.unom.clone() },
                            _ => Reduction::AmpPriv { ktt: a.amp_ktt.clone(), inom: a.inom.clone() },
                        },
                        group: match a.group {
                            _ if self.dif_phases == 1 => GroupComp::None,
                            1 => GroupComp::Posl { group: a.group_n.clone(), remove_i0: a.remove_i0 },
                            2 => GroupComp::Triangle { group: a.group_n.clone() },
                            _ => GroupComp::None,
                        },
                    })
                    .collect();
                let braking = match self.braking {
                    1 => Braking::Sub { k: self.k_torm.clone() },
                    2 => Braking::Max,
                    _ => Braking::Sum { k: self.k_torm.clone() },
                };
                quick::differential(self.dif_phases, &arms, &braking)
            }
            AnalysisTab::Formula => quick::Generated::default(),
        }
    }
}

impl App {
    /// Fill empty channel fields from the channel-role map.
    pub(crate) fn quick_defaults(&mut self) {
        let Some(d) = self.active_doc() else { return };
        let name = |idx: Option<usize>| idx.and_then(|i| d.osc.analog.get(i)).map(|c| c.id.clone()).unwrap_or_default();
        let (ua, ub, uc) = (name(d.analysis.u_abc[0]), name(d.analysis.u_abc[1]), name(d.analysis.u_abc[2]));
        let (ia, ib, ic) = (name(d.analysis.i_abc[0]), name(d.analysis.i_abc[1]), name(d.analysis.i_abc[2]));
        // The measured-channel combos preselect 3I0 / 3U0 / 3I0n by name,
        // case-insensitively (`0x411c70`), when the record has them.
        let named = |want: &str| {
            d.osc.analog.iter().map(|c| c.id.clone()).chain(d.computed.iter().map(|c| c.name.clone())).find(|n| n.to_lowercase() == want).unwrap_or_default()
        };
        let (i3o, u3o, i0n) = (named("3i0"), named("3u0"), named("3i0n"));
        let f = &mut self.quick;
        for (slot, v) in [(&mut f.ua, ua), (&mut f.ub, ub), (&mut f.uc, uc), (&mut f.ia, ia), (&mut f.ib, ib), (&mut f.ic, ic), (&mut f.i3o, i3o), (&mut f.u3o, u3o), (&mut f.i0_parallel, i0n)] {
            if slot.is_empty() {
                *slot = v;
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
        let (mut calc, mut cancel, mut open) = (false, false, true);
        let enabled = self.formula_dlg.is_none();
        egui::Window::new("Быстрый анализ")
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
                                if r.clicked() {
                                    self.analysis_tab = tab;
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
                                    self.formula_editor_ui(ui);
                                } else {
                                    self.quick_tab_ui(ui);
                                }
                            });
                        });
                    });
                    ui.separator();
                    ui.allocate_ui_with_layout(egui::Vec2::new(ui.available_width(), 26.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        cancel = ui.add_sized([80.0, 22.0], egui::Button::new("Отмена")).clicked();
                        let ok = self.quick.ok_enabled(self.analysis_tab);
                        calc = ui.add_enabled(ok, egui::Button::new("Рассчитать").min_size(egui::Vec2::new(80.0, 22.0))).clicked();
                    });
                });
            });
        if enabled && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            cancel = true;
        }
        if calc {
            self.quick_calc();
        } else if cancel || !open {
            self.quick_open = false;
        }
    }

    /// «Рассчитать» (`0x48b710` → `0x48c0e0`): the active page's channels
    /// (the formula editor's list is already the record's), then close.
    pub fn quick_calc(&mut self) {
        if self.analysis_tab != AnalysisTab::Formula {
            let g = self.quick.generate(self.analysis_tab);
            self.apply_quick(&g);
        }
        self.quick_open = false;
    }

    /// The form of the active quick-analysis page.
    pub fn quick_tab_ui(&mut self, ui: &mut egui::Ui) {
        self.quick_defaults();
        let Some(d) = self.active_doc() else { return };
        let names: Vec<String> =
            d.osc.analog.iter().map(|c| c.id.clone()).chain(d.computed.iter().map(|c| c.name.clone())).collect();
        let tab = self.analysis_tab;
        let f = &mut self.quick;
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
    let n = f.arms.len();
    ui.horizontal(|ui| {
        ui.label("Количество плеч:");
        let mut m = n;
        egui::ComboBox::from_id_salt("q_dif_n").selected_text(m.to_string()).width(50.0).show_ui(ui, |ui| {
            for k in 2..=24 {
                ui.selectable_value(&mut m, k, k.to_string());
            }
        });
        if m != n {
            f.arms.resize_with(m, Default::default);
        }
        ui.add_space(30.0);
        ui.label("Количество фаз:");
        egui::ComboBox::from_id_salt("q_dif_ph").selected_text(f.dif_phases.to_string()).width(50.0).show_ui(ui, |ui| {
            for k in [1, 3] {
                ui.selectable_value(&mut f.dif_phases, k, k.to_string());
            }
        });
    });
    let n = f.arms.len();
    if n > 2 && f.braking == 1 {
        f.braking = 0;
    }
    f.cur_arm = f.cur_arm.min(n - 1);
    let three = f.dif_phases == 3;
    let cur = f.cur_arm;
    let arm = &mut f.arms[cur];
    group(ui, "Приведение плеч", None, |ui| {
        egui::Grid::new("q_dif_red").num_columns(2).show(ui, |ui| {
            ui.radio_value(&mut arm.reduction, 2, "По мощности");
            ui.radio_value(&mut arm.reduction, 0, "По коэффициенту приведения");
            ui.end_row();
            ui.radio_value(&mut arm.reduction, 3, "По номинальному току");
            ui.radio_value(&mut arm.reduction, 1, "По коэффициенту трансформации ТТ");
            ui.end_row();
        });
    });
    group(ui, "Учет группы соединений", None, |ui| {
        ui.add_enabled_ui(three, |ui| {
            ui.horizontal(|ui| {
                ui.radio_value(&mut arm.group, 0, "Не учитывать");
                ui.radio_value(&mut arm.group, 1, "По последовательностям");
                ui.radio_value(&mut arm.group, 2, "Цифровой треугольник");
            });
        });
    });
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
            let r = arm.reduction;
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
            let groups: &[&str] = if arm.group == 2 { &["0", "1", "11"] } else { &["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11"] };
            ui.add_enabled_ui(three && arm.group != 0, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Группа:");
                    egui::ComboBox::from_id_salt("q_dif_grp").selected_text(arm.group_n.as_str()).width(50.0).show_ui(ui, |ui| {
                        for g in groups {
                            ui.selectable_value(&mut arm.group_n, g.to_string(), *g);
                        }
                    });
                });
            });
            ui.add_enabled(three && arm.group == 1, egui::Checkbox::new(&mut arm.remove_i0, "Удалить I0"));
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
