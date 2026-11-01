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
        let all: Vec<String> = d.quick_names();
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
        // VM: a 670 × 692 client whose left-top is where a sheet 144 px
        // narrower and 38 px taller would be centred (the Outlook bar is
        // added after; READ the place, INFERRED the cause): (1016, 317).
        // At > 100 % (VM, 125 %) the Outlook bar keeps its 148 px and its
        // 26 px images, its titles grow with the font (80 px for a
        // two-line item); the page and the buttons are the sheet's dialog
        // units (the page 604 px wide).
        use crate::dpi::{dx, dy};
        crate::dialogs::dlg(ctx, "Быстрый анализ")
            .id(egui::Id::new("fv_quick_sheet"))
            .margin(0.0)
            .nudge(egui::vec2(72.0, -dy(19.0)))
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(ctx, |ui| {
                // Everything sits at its own place: no spacing after the client.
                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                // The page (520 × 658 at 100 %) right of the 150 px bar, the
                // button strip (34) under it.
                let (page_w, page_h) = (dx(520.0), dy(658.0));
                let (client, _) = ui.allocate_exact_size(egui::vec2(150.0 + page_w, page_h + dy(34.0)), egui::Sense::hover());
                let at = |x: f32, y: f32, w: f32, h: f32| egui::Rect::from_min_size(client.min + egui::vec2(x, y), egui::vec2(w, h));
                ui.add_enabled_ui(enabled, |ui| {
                    // The Outlook bar (VM): (1, 1) 148 × 657 in a (185, 208,
                    // 237) border, (227, 239, 255) down its first quarter,
                    // then shading to (177, 211, 255) at its bottom; each
                    // page a 26 px image (our glyphs — bitmap 549 is the
                    // original's) 7 px below the item's top, its title
                    // centred under it in (0, 21, 110), 13 px lines; items
                    // 42 px + the lines; no mark on the selected one.
                    let bar = at(1.0, 1.0, 148.0, page_h - 1.0);
                    let p = ui.painter().clone();
                    p.rect_filled(bar, 0.0, egui::Color32::from_rgb(185, 208, 237));
                    let inner = bar.shrink(1.0);
                    let q = inner.top() + (inner.height() / 4.0).round();
                    let (c0, c1) = (egui::Color32::from_rgb(227, 239, 255), egui::Color32::from_rgb(177, 211, 255));
                    p.rect_filled(egui::Rect::from_min_max(inner.min, egui::pos2(inner.right(), q)), 0.0, c0);
                    let mut mesh = egui::Mesh::default();
                    for (pt, c) in [(egui::pos2(inner.left(), q), c0), (egui::pos2(inner.right(), q), c0), (inner.right_bottom(), c1), (inner.left_bottom(), c1)] {
                        mesh.colored_vertex(pt, c);
                    }
                    mesh.add_triangle(0, 1, 2);
                    mesh.add_triangle(0, 2, 3);
                    p.add(mesh);
                    let ink = egui::Color32::from_rgb(0, 21, 110);
                    let mut y = inner.top();
                    for (tab, glyph, title) in PAGES {
                        let g = ui.fonts(|f| f.layout_job(bar_title(f, title, ink, inner.width())));
                        let lines = g.rows.len().max(1) as f32;
                        // (the line pitch 13 px, 19 / 23 at 125 / 150 %: VM,
                        // the items 88 px apart at 150 %)
                        let item = egui::Rect::from_min_size(egui::pos2(inner.left(), y), egui::vec2(inner.width(), 42.0 + crate::dpi::measured(13.0, 19.0, 23.0) * lines));
                        let icon = egui::Rect::from_center_size(egui::pos2(inner.center().x, y + 7.0 + 13.0), egui::vec2(26.0, 26.0));
                        if ui.rect_contains_pointer(item) {
                            // The Office 2007 hot face under the pointer (VM):
                            // 5 px round the image, a (135, 171, 217) border,
                            // (255, 251, 232) → (250, 229, 169) above,
                            // (255, 208, 73) → (255, 225, 149) below.
                            let b = icon.expand(5.0);
                            p.rect_filled(b, 2.0, egui::Color32::from_rgb(135, 171, 217));
                            let f = b.shrink(1.0);
                            let mid = f.top() + (f.height() / 2.0).floor() - 2.0;
                            let mut mesh = egui::Mesh::default();
                            for (y0, y1, a, c) in [(f.top(), mid, (255, 251, 232), (250, 229, 169)), (mid, f.bottom(), (255, 208, 73), (255, 225, 149))] {
                                let k = mesh.vertices.len() as u32;
                                let (a, c) = (egui::Color32::from_rgb(a.0, a.1, a.2), egui::Color32::from_rgb(c.0, c.1, c.2));
                                for (pt, col) in [(egui::pos2(f.left(), y0), a), (egui::pos2(f.right(), y0), a), (egui::pos2(f.right(), y1), c), (egui::pos2(f.left(), y1), c)] {
                                    mesh.colored_vertex(pt, col);
                                }
                                mesh.add_triangle(k, k + 1, k + 2);
                                mesh.add_triangle(k, k + 2, k + 3);
                            }
                            p.add(mesh);
                        }
                        p.rect_filled(icon, 0.0, egui::Color32::from_rgb(149, 153, 173));
                        p.rect_filled(icon.shrink(1.0), 0.0, egui::Color32::from_rgb(242, 239, 250));
                        p.text(icon.center(), egui::Align2::CENTER_CENTER, glyph, egui::FontId::proportional(9.0), ink);
                        // Cut at the bar's edges (VM, 150 %: «Дифференциальны»).
                        p.with_clip_rect(p.clip_rect().intersect(inner)).galley(egui::pos2(inner.center().x, icon.bottom() + 8.0), g, ink);
                        if ui.interact(item, ui.id().with(("fv_quick_page", title)), egui::Sense::click()).clicked() && self.analysis_tab != tab {
                            if self.analysis_tab == AnalysisTab::Formula {
                                self.fp_kill_active();
                            }
                            self.analysis_tab = tab;
                            self.quick_show_page();
                        }
                        y = item.bottom();
                    }
                    // The page: (150, 0) to the client's right, (249) with a
                    // (229) line at its left and bottom; the strip below in
                    // COLOR_3DFACE with a (243) line at its top.
                    let page = at(150.0, 0.0, page_w, page_h);
                    p.rect_filled(page, 0.0, egui::Color32::from_gray(249));
                    p.vline(page.left() + 0.5, page.y_range(), egui::Stroke::new(1.0f32, egui::Color32::from_gray(229)));
                    p.hline(page.x_range(), page.bottom() - 0.5, egui::Stroke::new(1.0f32, egui::Color32::from_gray(229)));
                    p.hline(client.x_range(), page.bottom() + 0.5, egui::Stroke::new(1.0f32, egui::Color32::from_gray(243)));
                    // A page's template from (154, −1) in the client (VM:
                    // DIALOG 340's list at 7, 7 DLU lands at 1182, 353).
                    let origin = client.min + egui::vec2(150.0 + dx(4.0), -1.0);
                    if self.analysis_tab == AnalysisTab::Formula {
                        self.formula_page_ui(ui, origin);
                    } else {
                        self.quick_tab_ui(ui, origin);
                    }
                    let ok = page_index(self.analysis_tab).is_none_or(|i| self.quick[i].ok_enabled(self.analysis_tab));
                    ok_now = ok;
                    let (bw, bh, by) = (dx(75.0), dy(23.0), page_h + dy(6.0));
                    // The sheet's own buttons are another dialog's span than the page's.
                    crate::dialogs::group_start(ui);
                    calc = crate::dialogs::win_button_en(ui, at(client.width() - dx(161.0), by, bw, bh), "Рассчитать", true, ok).clicked();
                    cancel = crate::dialogs::win_button(ui, at(client.width() - dx(80.0), by, bw, bh), "Отмена", false).clicked();
                });
            });
        if let Some(msg) = self.formula_page.alert {
            let mut close = false;
            crate::dialogs::dlg(ctx, crate::app::APP_TITLE)
                .id(egui::Id::new("fv_formula_page_alert"))
                .margin(0.0)
                .collapsible(false)
                .resizable(false)
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    // `0x486a9d` / `0x486bd3`: MB_ICONHAND.
                    close = crate::dialogs::message_box(ui, crate::dialogs::MsgIcon::Error, msg, &["OK"]).is_some();
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
        let items: Vec<String> = d.quick_names();
        let sec = d.set.tree.section_mut(PAGE_KEYS[k]);
        if k < 5 {
            load_page(k, &mut self.quick[k], sec, &items);
        }
    }

    /// Every created page saves (OnOK `0x48b710` / OnCancel `0x48b770`);
    /// the record is not marked modified. The first page is created by
    /// then whatever was shown (VM: the `.set` gets «SymmetricItemsPage»,
    /// after the pages used, with the sheet never on it).
    fn quick_save_pages(&mut self) {
        if !self.quick_shown[0] {
            let tab = self.analysis_tab;
            self.analysis_tab = AnalysisTab::Symmetric;
            self.quick_show_page();
            self.analysis_tab = tab;
        }
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
            let taken: Vec<String> = self.active_doc().map(|d| fvb_core::quick::taken_names(&d.osc, d.computed.iter().map(|c| c.name.as_str()).chain(d.loose_defs.iter().map(|l| l.0.as_str())))).unwrap_or_default();
            let g = self.quick[i].generate(self.analysis_tab, &taken);
            self.apply_quick(&g);
        } else {
            self.fp_kill_active();
            self.fp_apply();
        }
        self.quick_save_pages();
        self.quick_open = false;
        // The command (137, `0x4543a0`) ends in the refresh-all when the sheet is accepted.
        self.refresh_blocks();
    }

    /// The form of the active quick-analysis page.
    pub fn quick_tab_ui(&mut self, ui: &mut egui::Ui, origin: egui::Pos2) {
        self.quick_defaults();
        let Some(d) = self.active_doc() else { return };
        let names: Vec<String> = d.quick_names();
        let tab = self.analysis_tab;
        let Some(i) = page_index(tab) else { return };
        let f = &mut self.quick[i];
        let pg = Pg { o: egui::Rect::from_min_size(origin, egui::Vec2::ZERO) };
        match tab {
            AnalysisTab::Symmetric => symmetric_page(ui, &pg, f, &names, false),
            AnalysisTab::PhasePhase => symmetric_page(ui, &pg, f, &names, true),
            AnalysisTab::Zero => zero_page(ui, &pg, f, &names),
            AnalysisTab::Phase => phase_page(ui, &pg, f, &names),
            AnalysisTab::Differential => differential_page(ui, &pg, f, &names),
            AnalysisTab::Formula => {}
        }
    }
}

/// A property page's controls at its template's dialog units from the
/// page's origin (VM: (154, −1) in the sheet's client).
struct Pg {
    o: egui::Rect,
}

impl Pg {
    fn r(&self, x: i32, y: i32, w: i32, h: i32) -> egui::Rect {
        crate::dialogs::dlu(self.o, x, y, w, h)
    }
    fn label(&self, ui: &egui::Ui, (x, y, w, h): (i32, i32, i32, i32), text: &str, right: bool, enabled: bool) {
        crate::dialogs::win_label(ui, self.r(x, y, w, h), text, right, enabled);
    }
    fn group(&self, ui: &egui::Ui, (x, y, w, h): (i32, i32, i32, i32), title: &str) {
        crate::dialogs::win_group(ui, self.r(x, y, w, h), title);
    }
    /// A channel list: the placeholder at the top while it is the value
    /// (`0x411c70`), then the record's channels.
    fn combo(&self, ui: &mut egui::Ui, (x, y, w, h): (i32, i32, i32, i32), id: &str, value: &mut String, names: &[String], enabled: bool) {
        let mut items = Vec::new();
        if value == fvb_core::addformula::NO_CHANNEL || value == fvb_core::addformula::NOT_COUNTED {
            items.push(value.clone());
        }
        items.extend(names.iter().cloned());
        let cur = items.iter().position(|n| n == value).unwrap_or(0);
        if let Some(k) = crate::dialogs::win_combo_list(ui, self.r(x, y, w, h), egui::Id::new(id), &items, cur, enabled) {
            *value = items[k].clone();
        }
    }
    /// A fixed list of choices; returns the one picked.
    fn choice(&self, ui: &mut egui::Ui, (x, y, w, h): (i32, i32, i32, i32), id: &str, items: &[String], cur: usize, enabled: bool) -> Option<usize> {
        crate::dialogs::win_combo_list(ui, self.r(x, y, w, h), egui::Id::new(id), items, cur, enabled)
    }
    fn check(&self, ui: &mut egui::Ui, (x, y, w, h): (i32, i32, i32, i32), text: &str, v: &mut bool, enabled: bool, left: bool) {
        if crate::dialogs::win_check_ex(ui, self.r(x, y, w, h), text, *v, enabled, left) {
            *v = !*v;
        }
    }
    fn radio(&self, ui: &mut egui::Ui, (x, y, w, h): (i32, i32, i32, i32), text: &str, checked: bool, enabled: bool) -> bool {
        crate::dialogs::win_radio_en(ui, self.r(x, y, w, h), text, checked, enabled)
    }
    fn edit(&self, ui: &mut egui::Ui, (x, y, w, h): (i32, i32, i32, i32), id: &str, v: &mut String, enabled: bool) {
        crate::dialogs::win_edit_en(ui, self.r(x, y, w, h), egui::Id::new(id), v, false, enabled);
    }
}

/// Pages 536 «Симметричные составляющие» and (`interphase`) 538
/// «Междуфазные сопротивления» on their templates: «Токи» (Ia / Ib / Ic,
/// «Рассчитать Ib по Ia и Iс *» greying Ib `0x487d70`; 538: «Iтр, А:») and
/// «Напряжения» (Фазные / Линейные radios enabling Ua–Uc or Uab / Ubc
/// `0x487de0`); on 536 each group's «Отключить» (BS_LEFTTEXT) greys its
/// block and the other group's check (`0x48c870`, `0x48cb60`).
fn symmetric_page(ui: &mut egui::Ui, pg: &Pg, f: &mut QuickForms, names: &[String], interphase: bool) {
    let (i_off, u_off) = if interphase { (false, false) } else { (f.currents_off, f.voltages_off) };
    let line = f.line_voltages;
    if interphase {
        pg.group(ui, (7, 7, 307, 119), "Токи");
        for (k, (ly, cy)) in [(22, 18), (41, 39), (62, 60)].into_iter().enumerate() {
            pg.label(ui, (12, ly, 24, 8), ["Ia:", "Ib:", "Ic:"][k], true, true);
            let v = match k { 0 => &mut f.ia, 1 => &mut f.ib, _ => &mut f.ic };
            // The template's tab order (538): Ia–Ic, the check box, the
            // checked radio, Ua–Uc, «Iтр», Uab, Ubc.
            crate::dialogs::tab_index(ui, k as u32);
            pg.combo(ui, (42, cy, 150, 54), ["q_ia", "q_ib", "q_ic"][k], v, names, k != 1 || !f.calc_ib);
        }
        // No WS_GROUP: the check box is in the span of «Ic» (538 template).
        crate::dialogs::group_continue(ui);
        crate::dialogs::tab_index(ui, 3);
        pg.check(ui, (204, 42, 102, 10), "Рассчитать Ib по Ia и Iс *", &mut f.calc_ib, true, false);
        pg.label(ui, (12, 86, 24, 8), "Iтр, А:", true, true);
        crate::dialogs::group_continue(ui);
        crate::dialogs::tab_index(ui, 8);
        pg.edit(ui, (42, 84, 150, 14), "q_itr", &mut f.itr, true);
        pg.label(ui, (204, 88, 102, 8), "- ток точной работы", false, true);
        pg.label(ui, (24, 108, 162, 8), "* - только в сетях с изолированной нейтралью", false, true);
        pg.group(ui, (6, 138, 307, 109), "Напряжения");
        crate::dialogs::tab_index(ui, 4);
        if pg.radio(ui, (54, 150, 85, 10), "Фазные напряжения", !line, true) {
            f.line_voltages = false;
        }
        crate::dialogs::tab_index(ui, 4);
        if pg.radio(ui, (198, 150, 93, 10), "Линейные напряжения", line, true) {
            f.line_voltages = true;
        }
        for (k, (ly, cy)) in [(170, 168), (190, 188), (210, 208)].into_iter().enumerate() {
            pg.label(ui, (12, ly, 18, 8), ["Ua:", "Ub:", "Uc:"][k], true, true);
            let v = match k { 0 => &mut f.ua, 1 => &mut f.ub, _ => &mut f.uc };
            // The voltage radios, these combo boxes and «Iтр» are one span (538 template).
            crate::dialogs::group_continue(ui);
            crate::dialogs::tab_index(ui, 5 + k as u32);
            pg.combo(ui, (36, cy, 120, 54), ["q_u0", "q_u1", "q_u2"][k], v, names, !line);
        }
        pg.label(ui, (165, 171, 22, 8), "Uab:", true, true);
        crate::dialogs::tab_index(ui, 9);
        pg.combo(ui, (189, 168, 120, 54), "q_uab", &mut f.uab, names, line);
        pg.label(ui, (162, 190, 24, 8), "Ubc:", true, true);
        crate::dialogs::tab_index(ui, 10);
        pg.combo(ui, (189, 188, 120, 54), "q_ubc", &mut f.ubc, names, line);
        return;
    }
    // The template's tab order (536; VM): Ia–Ic, the check box, the
    // checked radio, Ua–Uc, Uab, Ubc, then the two «Отключить».
    pg.group(ui, (6, 7, 306, 113), "Токи");
    crate::dialogs::tab_index(ui, 11);
    pg.check(ui, (225, 11, 86, 10), "Отключить", &mut f.currents_off, !u_off, true);
    for (k, (ly, cy)) in [(34, 30), (54, 52), (74, 73)].into_iter().enumerate() {
        pg.label(ui, (12, ly, 24, 8), ["Ia:", "Ib:", "Ic:"][k], true, !i_off);
        let v = match k { 0 => &mut f.ia, 1 => &mut f.ib, _ => &mut f.ic };
        crate::dialogs::tab_index(ui, k as u32);
        pg.combo(ui, (42, cy, 150, 54), ["q_ia", "q_ib", "q_ic"][k], v, names, !i_off && (k != 1 || !f.calc_ib));
    }
    // No WS_GROUP: the check box is in the span of «Ic» (536 template).
    crate::dialogs::group_continue(ui);
    crate::dialogs::tab_index(ui, 3);
    pg.check(ui, (204, 54, 102, 10), "Рассчитать Ib по Ia и Iс *", &mut f.calc_ib, !i_off, false);
    pg.label(ui, (27, 102, 162, 8), "* - только в сетях с изолированной нейтралью", false, !i_off);
    pg.group(ui, (6, 126, 307, 108), "Напряжения");
    crate::dialogs::tab_index(ui, 12);
    pg.check(ui, (200, 131, 112, 8), "Отключить", &mut f.voltages_off, !i_off, true);
    crate::dialogs::tab_index(ui, 4);
    if pg.radio(ui, (47, 150, 85, 10), "Фазные напряжения", !line, !u_off) {
        f.line_voltages = false;
    }
    crate::dialogs::tab_index(ui, 4);
    if pg.radio(ui, (191, 150, 93, 10), "Линейные напряжения", line, !u_off) {
        f.line_voltages = true;
    }
    for (k, (ly, cy)) in [(170, 169), (190, 187), (210, 209)].into_iter().enumerate() {
        pg.label(ui, (11, ly, 18, 8), ["Ua:", "Ub:", "Uc:"][k], true, !u_off);
        let v = match k { 0 => &mut f.ua, 1 => &mut f.ub, _ => &mut f.uc };
        crate::dialogs::tab_index(ui, 5 + k as u32);
        pg.combo(ui, (35, cy, 114, 54), ["q_u0", "q_u1", "q_u2"][k], v, names, !u_off && !line);
    }
    pg.label(ui, (161, 171, 22, 8), "Uab:", true, !u_off);
    crate::dialogs::tab_index(ui, 8);
    pg.combo(ui, (185, 169, 120, 54), "q_uab", &mut f.uab, names, !u_off && line);
    pg.label(ui, (158, 190, 24, 8), "Ubc:", true, !u_off);
    crate::dialogs::tab_index(ui, 9);
    pg.combo(ui, (185, 187, 120, 54), "q_ubc", &mut f.ubc, names, !u_off && line);
}

/// Page 537 «Нулевая последовательность» on its template: «Ток 3Io»
/// (Расчетный → Ia–Ic, Измеренный → «3Io:», `0x488f50`) and «Напряжение
/// 3Uo» (из Ua, Ub, Uc / из Uни, Uик / Измеренный, each enabling its own
/// combos, `0x4890b0`); the «Отключить» pair as on 536 (`0x489270`,
/// `0x489510`).
fn zero_page(ui: &mut egui::Ui, pg: &Pg, f: &mut QuickForms, names: &[String]) {
    let (i_off, u_off) = (f.zero_i_off, f.zero_u_off);
    // The template's tab order (537): Ia–Ic, the checked 3Io radio, Ua–Uc,
    // the checked 3Uo radio, 3Io, Uни, Uик, 3Uo, the two «Отключить».
    pg.group(ui, (6, 12, 319, 113), "Ток 3Io");
    // The two «Отключить» are a span of their own (a hidden WS_GROUP static
    // before them in the 537 template); a disabled one stays in it.
    crate::dialogs::group_start(ui);
    crate::dialogs::tab_index(ui, 12);
    pg.check(ui, (238, 17, 86, 10), "Отключить", &mut f.zero_i_off, !u_off, true);
    // The radios have no WS_GROUP: with the combo box of «Ic» they make one
    // span for the arrows (clone: ↑ from «Расчетный» reaches that combo).
    crate::dialogs::group_continue(ui);
    crate::dialogs::tab_index(ui, 3);
    if pg.radio(ui, (72, 36, 53, 10), "Расчетный", f.zero_i != 2, !i_off) {
        f.zero_i = 1;
    }
    crate::dialogs::tab_index(ui, 3);
    if pg.radio(ui, (228, 36, 57, 10), "Измеренный", f.zero_i == 2, !i_off) {
        f.zero_i = 2;
    }
    let calc = f.zero_i != 2;
    for (k, (ly, cy)) in [(57, 55), (75, 76), (97, 95)].into_iter().enumerate() {
        pg.label(ui, (12, ly, 18, 8), ["Ia:", "Ib:", "Ic:"][k], true, !i_off);
        let v = match k { 0 => &mut f.ia, 1 => &mut f.ib, _ => &mut f.ic };
        crate::dialogs::tab_index(ui, k as u32);
        pg.combo(ui, (36, cy, 132, 54), ["q_zia", "q_zib", "q_zic"][k], v, names, !i_off && calc);
    }
    pg.label(ui, (179, 57, 14, 8), "3Io:", true, !i_off);
    // Last in the span of «Uc» and the voltage radios (its label follows it
    // in the template).
    crate::dialogs::group_continue(ui);
    crate::dialogs::tab_index(ui, 8);
    pg.combo(ui, (198, 55, 120, 54), "q_z3io", &mut f.i3o, names, !i_off && !calc);
    pg.group(ui, (6, 132, 318, 138), "Напряжение 3Uo");
    crate::dialogs::tab_index(ui, 13);
    // «Отключить» ×2 are one span (537 template).
    crate::dialogs::group_continue(ui);
    pg.check(ui, (244, 137, 78, 8), "Отключить", &mut f.zero_u_off, !i_off, true);
    for (m, (r, text)) in [((54, 156, 107, 10), "Расчитанный из Ua, Ub, Uc"), ((198, 156, 102, 10), "Расчитанный из Uни, Uик"), ((198, 228, 72, 10), "Измеренный")].into_iter().enumerate() {
        if m == 0 {
            crate::dialogs::group_continue(ui);
        }
        crate::dialogs::tab_index(ui, 7);
        if pg.radio(ui, r, text, f.zero_u == m as u8 + 1, !u_off) {
            f.zero_u = m as u8 + 1;
        }
    }
    let m = f.zero_u;
    for (k, (ly, cy)) in [(176, 174), (196, 195), (219, 216)].into_iter().enumerate() {
        pg.label(ui, (12, ly, 18, 8), ["Ua:", "Ub:", "Uc:"][k], true, !u_off);
        let v = match k { 0 => &mut f.ua, 1 => &mut f.ub, _ => &mut f.uc };
        crate::dialogs::tab_index(ui, 4 + k as u32);
        pg.combo(ui, (36, cy, 132, 54), ["q_zua", "q_zub", "q_zuc"][k], v, names, !u_off && m == 1);
    }
    pg.label(ui, (176, 177, 16, 8), "Uни:", true, !u_off);
    crate::dialogs::tab_index(ui, 9);
    pg.combo(ui, (198, 174, 120, 54), "q_zuni", &mut f.uni, names, !u_off && m == 2);
    pg.label(ui, (176, 197, 16, 8), "Uик:", true, !u_off);
    crate::dialogs::tab_index(ui, 10);
    pg.combo(ui, (198, 195, 120, 54), "q_zuik", &mut f.uik, names, !u_off && m == 2);
    pg.label(ui, (168, 248, 24, 8), "3Uo:", true, !u_off);
    crate::dialogs::tab_index(ui, 11);
    pg.combo(ui, (198, 246, 122, 54), "q_z3uo", &mut f.u3o, names, !u_off && m == 3);
}

/// Page 543 «Фазные сопротивления» on its template: «Токи» (Ia–Ic, «Iтр,
/// А:»), «Ток 3Io*» (По удельным параметрам → X1 / R1 / X0 / R0; По
/// коэффициентам компенсации → K0 / Ф0, `0x48a910`), «Ток 3Io параллельной
/// линии» («3Ion:» with «Не учитывать» first; its edits — Xm / Rm or Km /
/// Фm by the radio — only with a channel, `0x48ab80`) and «Напряжения»
/// (Ua–Uc).
fn phase_page(ui: &mut egui::Ui, pg: &Pg, f: &mut QuickForms, names: &[String]) {
    use fvb_core::addformula::NOT_COUNTED;
    pg.group(ui, (7, 7, 315, 89), "Токи");
    for (k, (ly, cy)) in [(20, 18), (39, 39), (60, 59)].into_iter().enumerate() {
        pg.label(ui, (12, ly, 24, 8), ["Ia:", "Ib:", "Ic:"][k], true, true);
        let v = match k { 0 => &mut f.ia, 1 => &mut f.ib, _ => &mut f.ic };
        crate::dialogs::tab_index(ui, k as u32);
        pg.combo(ui, (42, cy, 126, 54), ["q_pia", "q_pib", "q_pic"][k], v, names, true);
    }
    // The template's tab order (543): Ia–Ic, the checked radio, R0, R1,
    // X0, X1, K0, Ф0, 3Ion, Rm, Xm, Km, Фm, Ua–Uc, then «Iтр».
    pg.label(ui, (12, 80, 24, 8), "Iтр, А:", true, true);
    crate::dialogs::tab_index(ui, 18);
    pg.edit(ui, (42, 78, 126, 14), "q_pitr", &mut f.itr, true);
    pg.label(ui, (174, 80, 102, 8), "- ток точной работы", false, true);
    pg.group(ui, (7, 99, 315, 127), "Ток 3Io*");
    pg.label(ui, (42, 111, 152, 8), "Расчет коэффициента компенсации тока 3Io", false, true);
    // No WS_GROUP: the radios join the span of «Ic»'s combo box.
    crate::dialogs::group_continue(ui);
    crate::dialogs::tab_index(ui, 3);
    if pg.radio(ui, (24, 123, 103, 10), "По удельным параметрам", f.per_unit, true) {
        f.per_unit = true;
    }
    crate::dialogs::tab_index(ui, 3);
    if pg.radio(ui, (192, 123, 126, 10), "По коэффициентам компенсации", !f.per_unit, true) {
        f.per_unit = false;
    }
    let pu = f.per_unit;
    for (((ly, ey, label), (id, v)), place) in [(143, 141, "X1, Ом/км:"), (159, 159, "R1, Ом/км:"), (177, 177, "X0, Ом/км:"), (195, 195, "R0, Ом/км:")].into_iter().zip([("q_x1", &mut f.x1), ("q_r1", &mut f.r1), ("q_x0", &mut f.x0), ("q_r0", &mut f.r0)]).zip([7, 5, 6, 4]) {
        pg.label(ui, (12, ly, 42, 8), label, true, pu);
        crate::dialogs::tab_index(ui, place);
        pg.edit(ui, (60, ey, 96, 12), id, v, pu);
    }
    pg.label(ui, (168, 143, 33, 8), "K0:", true, !pu);
    crate::dialogs::tab_index(ui, 8);
    pg.edit(ui, (204, 141, 108, 12), "q_k0", &mut f.k0, !pu);
    pg.label(ui, (168, 161, 33, 8), "Ф0, град:", true, !pu);
    crate::dialogs::tab_index(ui, 9);
    pg.edit(ui, (204, 159, 108, 12), "q_f0", &mut f.f0, !pu);
    pg.label(ui, (42, 213, 239, 8), "* Ток нулевой последовательности рассчитывается из фазных токов", false, true);
    pg.group(ui, (6, 229, 316, 80), "Ток 3Io параллельной линии");
    if f.i0_parallel.is_empty() {
        f.i0_parallel = NOT_COUNTED.into();
    }
    pg.label(ui, (12, 249, 18, 8), "3Ion:", true, true);
    let mut items = vec![NOT_COUNTED.to_string()];
    items.extend(names.iter().cloned());
    let cur = items.iter().position(|n| *n == f.i0_parallel).unwrap_or(0);
    crate::dialogs::tab_index(ui, 10);
    if let Some(k) = pg.choice(ui, (36, 247, 132, 30), "q_p3ion", &items, cur, true) {
        f.i0_parallel = items[k].clone();
    }
    let used = f.parallel_used();
    pg.label(ui, (12, 274, 42, 8), "Xm, Ом/км:", true, used && pu);
    crate::dialogs::tab_index(ui, 12);
    pg.edit(ui, (60, 271, 96, 14), "q_xm", &mut f.xm, used && pu);
    pg.label(ui, (13, 292, 41, 8), "Rm, Ом/км:", true, used && pu);
    crate::dialogs::tab_index(ui, 11);
    pg.edit(ui, (60, 289, 96, 14), "q_rm", &mut f.rm, used && pu);
    pg.label(ui, (168, 274, 31, 8), "Km:", true, used && !pu);
    crate::dialogs::tab_index(ui, 13);
    pg.edit(ui, (204, 271, 108, 14), "q_km", &mut f.km, used && !pu);
    pg.label(ui, (164, 291, 35, 8), "Фm, град:", true, used && !pu);
    crate::dialogs::tab_index(ui, 14);
    pg.edit(ui, (204, 289, 108, 14), "q_fm", &mut f.fm, used && !pu);
    pg.group(ui, (6, 312, 316, 76), "Напряжения");
    for (k, (ly, cy)) in [(330, 328), (350, 348), (370, 368)].into_iter().enumerate() {
        pg.label(ui, (12, ly, 18, 8), ["Ua:", "Ub:", "Uc:"][k], true, true);
        let v = match k { 0 => &mut f.ua, 1 => &mut f.ub, _ => &mut f.uc };
        crate::dialogs::tab_index(ui, 15 + k as u32);
        pg.combo(ui, (36, cy, 132, 54), ["q_pua", "q_pub", "q_puc"][k], v, names, true);
    }
}

/// Page 539 «Дифференциальные токи» on its template (its controls given
/// the template's tab places): arm and phase
/// counts; per arm (kept per arm) «Приведение плеч» (its radio enables
/// only its edit block, `0x4835b0`…) and «Учет группы соединений» (3 phases
/// only, `0x482c10`: По последовательностям → «Группа» 0…11 + «Удалить
/// I0», Цифровой треугольник → {0, 1, 11}, Не учитывать → 0…11 greyed,
/// `0x483ab0`); «<<» / «Текущее плечо:» / «>>» (greyed at the ends,
/// `0x4831b0`); the arm's group «Плечо %d» (string 198); «Расчет тока
/// торможения» — «Полуразность токов» only with 2 arms (else forced to
/// «Полусумма», `0x482b60`), «Kторм:» {1, 0.5} for Полусумма or Полуразность
/// with 2 arms (`0x483da0`).
fn differential_page(ui: &mut egui::Ui, pg: &Pg, f: &mut QuickForms, names: &[String]) {
    pg.label(ui, (12, 12, 90, 8), "Количество плеч:", true, true);
    // A changed count keeps every slot (`0x482b60` → `0x482a40`).
    let arms: Vec<String> = (2..=ARM_SLOTS).map(|k| k.to_string()).collect();
    crate::dialogs::tab_index(ui, 0);
    if let Some(k) = pg.choice(ui, (108, 10, 42, 30), "q_dif_n", &arms, f.dif_arms.saturating_sub(2), true) {
        f.dif_arms = k + 2;
    }
    pg.label(ui, (192, 13, 72, 8), "Количество фаз:", true, true);
    let phases = ["1".to_string(), "3".to_string()];
    crate::dialogs::tab_index(ui, 1);
    if let Some(k) = pg.choice(ui, (270, 10, 44, 30), "q_dif_ph", &phases, usize::from(f.dif_phases == 3), true) {
        f.dif_phases = if k == 1 { 3 } else { 1 };
    }
    let n = f.dif_arms;
    if n > 2 && f.braking == 1 {
        f.braking = 0;
    }
    f.cur_arm = f.cur_arm.min(n - 1);
    let three = f.dif_phases == 3;
    let cur = f.cur_arm;
    pg.group(ui, (6, 31, 305, 40), "Приведение плеч");
    let checked = f.dif_methods();
    for (m, r, text) in [(2u8, (18, 45, 60, 10), "По мощности"), (3, (18, 58, 93, 10), "По номинальному току"), (0, (150, 45, 119, 10), "По коэффициенту приведения"), (1, (150, 58, 141, 10), "По коэффициенту трансформации ТТ")] {
        // The radios have no WS_GROUP, nor has the box drawn before them: they join
        // the span of «Ic» (539 template).
        crate::dialogs::group_continue(ui);
        crate::dialogs::tab_index(ui, 6 + m as u32);
        if pg.radio(ui, r, text, checked.contains(&m), true) {
            f.dif_reduction = m;
            f.dif_also.clear();
        }
    }
    pg.group(ui, (6, 79, 306, 42), "Учет группы соединений");
    for (g, r, text) in [(0u8, (18, 91, 64, 10), "Не учитывать"), (1, (18, 103, 102, 10), "По последовательностям"), (2, (150, 103, 96, 10), "Цифровой треугольник")] {
        crate::dialogs::tab_index(ui, 20 + g as u32);
        if pg.radio(ui, r, text, f.dif_group == g, three) {
            f.dif_group = g;
        }
    }
    let (r, grp) = (f.dif_edit_method(), f.dif_group);
    pg.label(ui, (48, 133, 66, 8), "Текущее плечо:", true, true);
    crate::dialogs::group_continue(ui);
    crate::dialogs::tab_index(ui, 10);
    if crate::dialogs::win_button_en(ui, pg.r(120, 133, 24, 14), "<<", false, cur > 0).clicked() {
        f.cur_arm -= 1;
    }
    let curs: Vec<String> = (1..=n).map(|k| k.to_string()).collect();
    crate::dialogs::tab_index(ui, 2);
    if let Some(k) = pg.choice(ui, (150, 133, 42, 30), "q_dif_cur", &curs, cur, true) {
        f.cur_arm = k;
    }
    crate::dialogs::tab_index(ui, 11);
    if crate::dialogs::win_button_en(ui, pg.r(198, 133, 24, 14), ">>", false, cur + 1 < n).clicked() {
        f.cur_arm += 1;
    }
    let arm = &mut f.arms[cur];
    pg.group(ui, (6, 151, 306, 192), &format!("Плечо {}", cur + 1));
    {
        let [ia, ib, ic] = &mut arm.currents;
        for (k, (ly, cy), v) in [(0, (167, 165), ia), (1, (188, 186), ib), (2, (209, 207), ic)] {
            pg.label(ui, (15, ly, 24, 8), ["Ia:", "Ib:", "Ic:"][k], true, k == 0 || three);
            crate::dialogs::tab_index(ui, 3 + k as u32);
            pg.combo(ui, (45, cy, 150, 54), ["q_dia", "q_dib", "q_dic"][k], v, names, k == 0 || three);
        }
    }
    pg.label(ui, (155, 229, 107, 8), "По коэффициенту приведения:", false, r == 0);
    pg.label(ui, (173, 242, 30, 8), "Kприв:", false, r == 0);
    crate::dialogs::tab_index(ui, 12);
    pg.edit(ui, (203, 241, 102, 12), "q_kpriv", &mut arm.kpriv, r == 0);
    pg.label(ui, (155, 258, 129, 8), "По коэффициенту трансформации ТТ:", false, r == 1);
    pg.label(ui, (173, 271, 16, 8), "Kтт:", false, r == 1);
    crate::dialogs::tab_index(ui, 13);
    pg.edit(ui, (203, 271, 102, 12), "q_tt_ktt", &mut arm.tt_ktt, r == 1);
    pg.label(ui, (173, 289, 27, 8), "Kттбаз:", false, r == 1);
    crate::dialogs::tab_index(ui, 14);
    pg.edit(ui, (203, 289, 102, 12), "q_tt_base", &mut arm.tt_base, r == 1);
    pg.label(ui, (19, 229, 48, 8), "По мощности:", false, r == 2);
    pg.label(ui, (25, 241, 24, 8), "Kтт:", true, r == 2);
    crate::dialogs::tab_index(ui, 15);
    pg.edit(ui, (49, 241, 90, 12), "q_pw_ktt", &mut arm.pw_ktt, r == 2);
    pg.label(ui, (7, 259, 42, 8), "Sном, кВА:", true, r == 2);
    crate::dialogs::tab_index(ui, 16);
    pg.edit(ui, (49, 259, 90, 12), "q_snom", &mut arm.snom, r == 2);
    pg.label(ui, (7, 277, 42, 8), "Uном, кВ:", true, r == 2);
    crate::dialogs::tab_index(ui, 17);
    pg.edit(ui, (49, 277, 90, 12), "q_unom", &mut arm.unom, r == 2);
    pg.label(ui, (19, 295, 80, 8), "По номинальному току:", false, r == 3);
    pg.label(ui, (25, 306, 24, 8), "Kтт:", true, r == 3);
    crate::dialogs::tab_index(ui, 18);
    pg.edit(ui, (49, 306, 90, 12), "q_amp_ktt", &mut arm.amp_ktt, r == 3);
    pg.label(ui, (13, 327, 36, 8), "Iном, А:", true, r == 3);
    crate::dialogs::tab_index(ui, 19);
    pg.edit(ui, (49, 325, 90, 12), "q_inom", &mut arm.inom, r == 3);
    // Two «Группа:» combos at one place (2127 / 2129), one per method; with
    // one phase the sequence combo shows, whatever the radio (`0x483ab0`).
    let (groups, value): (&[&str], &mut String) = if three && grp == 2 {
        (&["0", "1", "11"], &mut arm.tri_group)
    } else {
        (&["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11"], &mut arm.seq_group)
    };
    let gl: Vec<String> = groups.iter().map(|g| g.to_string()).collect();
    let genabled = three && grp != 0;
    pg.label(ui, (173, 328, 28, 8), "Группа:", false, genabled);
    let gcur = gl.iter().position(|g| g == value).unwrap_or(0);
    crate::dialogs::tab_index(ui, 27);
    if let Some(k) = pg.choice(ui, (203, 325, 29, 30), &format!("q_dif_grp{grp}"), &gl, gcur, genabled) {
        *value = gl[k].clone();
    }
    crate::dialogs::tab_index(ui, 28);
    pg.check(ui, (237, 328, 50, 10), "Удалить I0", &mut arm.remove_i0, three && grp == 1, false);
    pg.group(ui, (6, 346, 305, 53), "Расчет тока торможения");
    for (b, r, text, en) in [(0u8, (17, 357, 105, 10), "Полусумма модулей токов", true), (1, (17, 370, 85, 10), "Полуразность токов", n == 2), (2, (17, 383, 97, 10), "Максимальный из токов", true)] {
        crate::dialogs::tab_index(ui, 24 + b as u32);
        if pg.radio(ui, r, text, f.braking == b, en) {
            f.braking = b;
        }
    }
    let kt = f.braking == 0 || (f.braking == 1 && n == 2);
    pg.label(ui, (149, 363, 24, 8), "Kторм:", true, kt);
    let kts = ["1".to_string(), "0.5".to_string()];
    let kcur = kts.iter().position(|k| *k == f.k_torm).unwrap_or(0);
    crate::dialogs::tab_index(ui, 23);
    if let Some(k) = pg.choice(ui, (185, 362, 42, 30), "q_dif_kt", &kts, kcur, kt) {
        f.k_torm = kts[k].clone();
    }
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

/// An Outlook-bar page title: the bar's own font, Segoe UI 9 pt (VM; ours
/// Tahoma at its 12 px, which wraps the titles where Segoe UI does —
/// «Дифференциальные токи» on two lines, «Фазные сопротивления» on one),
/// centred, wrapped 4 px in from each side.
fn bar_title(fonts: &egui::epaint::Fonts, title: &str, ink: egui::Color32, width: f32) -> egui::text::LayoutJob {
    // DrawText breaks between words only: a word wider than the bar (ours
    // at > 100 %, «последовательность») keeps its line, centred.
    let font = egui::FontId::proportional(crate::dpi::font(12.0));
    let words = title.split(' ').map(|w| fonts.layout_no_wrap(w.to_string(), font.clone(), ink).size().x).fold(0.0, f32::max);
    let mut job = egui::text::LayoutJob::simple(title.to_string(), font, ink, (width - 8.0).max(words.ceil()));
    job.halign = egui::Align::Center;
    job
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Outlook bar's titles break where the original's do (VM, a
    /// clone run: «Дифференциальные токи» on two lines).
    #[test]
    fn outlook_titles_wrap_as_the_original() {
        let ctx = egui::Context::default();
        ctx.set_fonts(crate::app_fonts());
        let _ = ctx.run(Default::default(), |_| {});
        let rows: Vec<usize> = PAGES.iter().map(|(_, _, t)| ctx.fonts(|f| f.layout_job(bar_title(f, t, egui::Color32::BLACK, 146.0))).rows.len()).collect();
        assert_eq!(rows, [2, 2, 2, 1, 2, 1], "{:?}", PAGES.map(|p| p.2));
    }

    /// VM: closing the sheet opened on the formula editor writes
    /// «SymmetricItemsPage» too (the sheet's first page is created).
    #[test]
    fn closing_the_sheet_creates_its_first_page() {
        let mut app = App::new();
        app.dispatch(crate::app::Command::Demo);
        app.dispatch(crate::app::Command::QuickAnalysis);
        let d = app.active_doc().unwrap();
        assert!(d.set.tree.section(PAGE_KEYS[0]).is_none());
        app.quick_save_pages();
        let sec = app.active_doc().unwrap().set.tree.section(PAGE_KEYS[0]).expect("SymmetricItemsPage");
        assert!(sec.child("IA").is_some() && sec.attrs.contains_key("phaseVoltState"));
    }

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
