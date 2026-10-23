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
    /// Zero sequence: 0 off, 1 calculated, 2 measured (current);
    /// 0 off, 1 from Ua,Ub,Uc, 2 from Uни,Uик, 3 measured (voltage).
    pub zero_i: u8,
    pub zero_u: u8,
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
}

#[derive(Debug, Clone)]
pub struct ArmForm {
    pub currents: [String; 3],
    /// 0 CoeffPriv, 1 CoeffTT, 2 CoeffPowerPriv, 3 CoeffAmpPriv.
    pub reduction: u8,
    pub p1: String,
    pub p2: String,
    pub p3: String,
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
            p1: "1".into(),
            p2: "1".into(),
            p3: "1".into(),
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
            i3o: s(),
            u3o: s(),
            uni: s(),
            uik: s(),
            itr: "0".into(),
            i0_parallel: s(),
            per_unit: true,
            r0: "0".into(),
            r1: "0".into(),
            rm: "0".into(),
            x0: "0".into(),
            x1: "0".into(),
            xm: "0".into(),
            k0: "0".into(),
            km: "0".into(),
            f0: "0".into(),
            fm: "0".into(),
            dif_phases: 3,
            arms: vec![ArmForm::default(), ArmForm::default()],
            braking: 0,
            k_torm: "0.5".into(),
        }
    }
}

fn combo(ui: &mut egui::Ui, id: &str, label: &str, value: &mut String, names: &[String]) {
    ui.label(label);
    egui::ComboBox::from_id_salt(id).selected_text(value.as_str()).width(140.0).show_ui(ui, |ui| {
        for n in names {
            ui.selectable_value(value, n.clone(), n);
        }
    });
}

fn field(ui: &mut egui::Ui, label: &str, value: &mut String) {
    ui.label(label);
    ui.add(egui::TextEdit::singleline(value).desired_width(70.0));
}

impl QuickForms {
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
                    1 => Some(ZeroCurrent::Calculated { ia: self.ia.clone(), ib: self.ib.clone(), ic: self.ic.clone() }),
                    2 => Some(ZeroCurrent::Measured(self.i3o.clone())),
                    _ => None,
                };
                let zu = match self.zero_u {
                    1 => Some(ZeroVoltage::FromPhases { ua: self.ua.clone(), ub: self.ub.clone(), uc: self.uc.clone() }),
                    2 => Some(ZeroVoltage::FromUniUik { uni: self.uni.clone(), uik: self.uik.clone() }),
                    3 => Some(ZeroVoltage::Measured(self.u3o.clone())),
                    _ => None,
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
                quick::phase_z(&self.currents(), [&self.ua, &self.ub, &self.uc], &self.i0_parallel, &comp, &self.itr)
            }
            AnalysisTab::Differential => {
                let arms: Vec<Arm> = self
                    .arms
                    .iter()
                    .map(|a| Arm {
                        currents: a.currents[..self.dif_phases].to_vec(),
                        reduction: match a.reduction {
                            0 => Reduction::Priv { k: a.p1.clone() },
                            1 => Reduction::Tt { ktt: a.p1.clone(), ktt_base: a.p2.clone() },
                            2 => Reduction::PowerPriv { ktt: a.p1.clone(), snom: a.p2.clone(), unom: a.p3.clone() },
                            _ => Reduction::AmpPriv { ktt: a.p1.clone(), inom: a.p2.clone() },
                        },
                        group: match a.group {
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
        let f = &mut self.quick;
        for (slot, v) in [(&mut f.ua, ua), (&mut f.ub, ub), (&mut f.uc, uc), (&mut f.ia, ia), (&mut f.ib, ib), (&mut f.ic, ic)] {
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
                        calc = ui.add_sized([80.0, 22.0], egui::Button::new("Рассчитать")).clicked();
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
        egui::Grid::new("quickform").num_columns(6).spacing([8.0, 6.0]).show(ui, |ui| {
            let currents = |ui: &mut egui::Ui, f: &mut QuickForms| {
                combo(ui, "q_ia", "Ia", &mut f.ia, &names);
                combo(ui, "q_ib", "Ib", &mut f.ib, &names);
                combo(ui, "q_ic", "Ic", &mut f.ic, &names);
                ui.end_row();
                ui.checkbox(&mut f.calc_ib, "Рассчитать Ib по Ia и Iс");
                ui.end_row();
            };
            let voltages = |ui: &mut egui::Ui, f: &mut QuickForms| {
                ui.radio_value(&mut f.line_voltages, false, "Фазные");
                ui.radio_value(&mut f.line_voltages, true, "Линейные");
                ui.end_row();
                if f.line_voltages {
                    combo(ui, "q_uab", "Uab", &mut f.uab, &names);
                    combo(ui, "q_ubc", "Ubc", &mut f.ubc, &names);
                } else {
                    combo(ui, "q_ua", "Ua", &mut f.ua, &names);
                    combo(ui, "q_ub", "Ub", &mut f.ub, &names);
                    combo(ui, "q_uc", "Uc", &mut f.uc, &names);
                }
                ui.end_row();
            };
            match tab {
                AnalysisTab::Symmetric => {
                    ui.checkbox(&mut f.currents_off, "Отключить токи");
                    ui.end_row();
                    if !f.currents_off {
                        currents(ui, f);
                    }
                    ui.checkbox(&mut f.voltages_off, "Отключить напряжения");
                    ui.end_row();
                    if !f.voltages_off {
                        voltages(ui, f);
                    }
                }
                AnalysisTab::Zero => {
                    ui.label("Ток 3I0:");
                    ui.radio_value(&mut f.zero_i, 0, "Отключить");
                    ui.radio_value(&mut f.zero_i, 1, "Расчетный");
                    ui.radio_value(&mut f.zero_i, 2, "Измеренный");
                    ui.end_row();
                    match f.zero_i {
                        1 => currents(ui, f),
                        2 => {
                            combo(ui, "q_3io", "3Io", &mut f.i3o, &names);
                            ui.end_row();
                        }
                        _ => {}
                    }
                    ui.label("Напряжение 3U0:");
                    ui.radio_value(&mut f.zero_u, 0, "Отключить");
                    ui.radio_value(&mut f.zero_u, 1, "из Ua,Ub,Uc");
                    ui.radio_value(&mut f.zero_u, 2, "из Uни,Uик");
                    ui.radio_value(&mut f.zero_u, 3, "Измеренный");
                    ui.end_row();
                    match f.zero_u {
                        1 => {
                            combo(ui, "q_ua", "Ua", &mut f.ua, &names);
                            combo(ui, "q_ub", "Ub", &mut f.ub, &names);
                            combo(ui, "q_uc", "Uc", &mut f.uc, &names);
                        }
                        2 => {
                            combo(ui, "q_uni", "Uни", &mut f.uni, &names);
                            combo(ui, "q_uik", "Uик", &mut f.uik, &names);
                        }
                        3 => combo(ui, "q_3uo", "3Uo", &mut f.u3o, &names),
                        _ => {}
                    }
                    ui.end_row();
                }
                AnalysisTab::PhasePhase => {
                    currents(ui, f);
                    voltages(ui, f);
                    field(ui, "Iтр", &mut f.itr);
                    ui.end_row();
                }
                AnalysisTab::Phase => {
                    currents(ui, f);
                    combo(ui, "q_ua", "Ua", &mut f.ua, &names);
                    combo(ui, "q_ub", "Ub", &mut f.ub, &names);
                    combo(ui, "q_uc", "Uc", &mut f.uc, &names);
                    ui.end_row();
                    let mut with_none = vec![String::new()];
                    with_none.extend(names.iter().cloned());
                    combo(ui, "q_3i0n", "3I0 параллельной линии", &mut f.i0_parallel, &with_none);
                    field(ui, "Iтр", &mut f.itr);
                    ui.end_row();
                    ui.radio_value(&mut f.per_unit, true, "По удельным параметрам");
                    ui.radio_value(&mut f.per_unit, false, "По коэффициентам компенсации");
                    ui.end_row();
                    if f.per_unit {
                        field(ui, "R0", &mut f.r0);
                        field(ui, "R1", &mut f.r1);
                        field(ui, "Rm", &mut f.rm);
                        ui.end_row();
                        field(ui, "X0", &mut f.x0);
                        field(ui, "X1", &mut f.x1);
                        field(ui, "Xm", &mut f.xm);
                    } else {
                        field(ui, "K0", &mut f.k0);
                        field(ui, "Km", &mut f.km);
                        ui.end_row();
                        field(ui, "Ф0", &mut f.f0);
                        field(ui, "Фm", &mut f.fm);
                    }
                    ui.end_row();
                }
                AnalysisTab::Differential => {
                    ui.label("Фаз:");
                    ui.radio_value(&mut f.dif_phases, 1, "1");
                    ui.radio_value(&mut f.dif_phases, 3, "3");
                    let mut n = f.arms.len();
                    ui.label("Плеч:");
                    ui.add(egui::DragValue::new(&mut n).range(2..=24));
                    f.arms.resize_with(n, Default::default);
                    ui.end_row();
                    for (k, arm) in f.arms.iter_mut().enumerate() {
                        ui.label(format!("Плечо {}", k + 1));
                        for (p, letter) in ["a", "b", "c"].iter().enumerate().take(f.dif_phases) {
                            combo(ui, &format!("q_arm{k}_{p}"), &format!("I{letter}"), &mut arm.currents[p], &names);
                        }
                        ui.end_row();
                        egui::ComboBox::from_id_salt(format!("q_red{k}"))
                            .selected_text(["Kприв", "Kтт / Kтт баз", "Kтт, Sном, Uном", "Kтт, Iном"][arm.reduction as usize])
                            .show_ui(ui, |ui| {
                                for (i, t) in ["Kприв", "Kтт / Kтт баз", "Kтт, Sном, Uном", "Kтт, Iном"].iter().enumerate() {
                                    ui.selectable_value(&mut arm.reduction, i as u8, *t);
                                }
                            });
                        ui.add(egui::TextEdit::singleline(&mut arm.p1).desired_width(60.0));
                        if arm.reduction >= 1 {
                            ui.add(egui::TextEdit::singleline(&mut arm.p2).desired_width(60.0));
                        }
                        if arm.reduction == 2 {
                            ui.add(egui::TextEdit::singleline(&mut arm.p3).desired_width(60.0));
                        }
                        ui.end_row();
                        if f.dif_phases == 3 {
                            ui.radio_value(&mut arm.group, 0, "Без поворота");
                            ui.radio_value(&mut arm.group, 1, "Через симм. составляющие");
                            ui.radio_value(&mut arm.group, 2, "Треугольник");
                            if arm.group != 0 {
                                field(ui, "Группа", &mut arm.group_n);
                            }
                            if arm.group == 1 {
                                ui.checkbox(&mut arm.remove_i0, "Удалить I0");
                            }
                            ui.end_row();
                        }
                    }
                    ui.label("Торможение:");
                    ui.radio_value(&mut f.braking, 0, "Полусумма модулей");
                    ui.radio_value(&mut f.braking, 1, "Плечи 1 и 2");
                    ui.radio_value(&mut f.braking, 2, "Максимум");
                    if f.braking != 2 {
                        field(ui, "Kторм", &mut f.k_torm);
                    }
                    ui.end_row();
                }
                AnalysisTab::Formula => {}
            }
        });
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
