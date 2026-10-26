//! The ribbon's tooltips as MFC's CMFCToolTipCtrl shows them in the Office
//! 2007 looks (VM): after half a second over an element — a disabled one
//! too — a box with a (118, 118, 118) rounded edge round a (253, 253, 254)
//! → (203, 218, 239) gradient and a soft shadow; the element's image, its
//! bold title (the command prompt's part after «\n», with the accelerator)
//! and the description (the part before) wrapped near 190 px. A panel
//! element's box opens at its left, 3 px under the ribbon; a quick-access
//! button's at the pointer's x, under the caption row.

use crate::app::{Command, Pane, ValueMode};
use crate::chansel::ChanFlag;
use crate::icons::Icon;
use crate::ribbon::Mini;
use eframe::egui::{self, Color32, Pos2, Rect, Vec2};

/// The image a tooltip shows: an element's large (32 px) or small (16 px)
/// one, or none (a plain one-line tip).
#[derive(Clone, Copy)]
pub enum TipImage {
    None,
    Large(Icon),
    Small(Icon),
    Mini(Mini),
    Tool(crate::diagram_ui::Tool),
    OmpTool(crate::omp_ui::ToolIcon),
    /// The help button's round «?».
    Help,
    /// A title and description with no image (VM: «Стиль»).
    Blank,
}

/// A command's tooltip title and description (its prompt string,
/// «description\ntitle»; the accelerator added to the title as MFC does).
/// `None`: no tooltip (VM: «Каналы», whose command has no prompt).
pub fn cmd_tip(cmd: &Command) -> Option<(&'static str, &'static str)> {
    use Command as C;
    Some(match cmd {
        C::Paste => ("Вставить (Ctrl+V)", "Вставить каналы для объединения"),
        C::Copy => ("Копировать (Ctrl+C)", "Копировать выделенные каналы для объединения"),
        // MFC's own 57643 / 57644 (mfc120rus).
        C::Undo => ("Отменить (Ctrl+Z)", "Отмена последнего действия"),
        C::Redo => ("Вернуть (Ctrl+Y)", "Повторное выполнение последнего отмененного действия"),
        C::Open => ("Открыть (Ctrl+O)", "Открытие существующего документа"),
        C::Save => ("Сохранить", "Сохранение активного документа"),
        C::Print => ("Печать (Ctrl+P)", "Печать активного документа"),
        C::TogglePane(Pane::Frequency) => ("Частотная диаграмма", "Показать частотную диаграмму"),
        C::TogglePane(Pane::Vector) => ("Векторная диаграмма", "Показать векторную диаграмму"),
        C::TogglePane(Pane::Circle) => ("Круговая диаграмма", "Показать круговую диаграмму"),
        C::TogglePane(Pane::Differential) => ("Дифференциальная диаграмма", "Показать дифференциальную диаграмму"),
        C::Discrete(0) => ("Показать дискретные каналы", "Показать дискретные каналы"),
        C::Discrete(1) => ("Скрыть дискретные каналы", "Скрыть дискретные каналы"),
        C::Discrete(_) => ("Скрыть неизменяющиеся каналы", "Скрыть дискретные каналы, не меняющие свое значение"),
        C::ZoomX(true) => ("Увеличить масштаб по горизонтали", "Увеличить масштаб по горизонтали (CTRL + колесико мыши)"),
        C::ZoomX(false) => ("Уменьшить масштаб по горизонтали", "Уменьшить масштаб по горизонтали (CTRL + колесико мыши)"),
        C::RectZoom => ("Приблизить по вертикали выделенную область сигнала", "Приблизить по вертикали"),
        C::ZoomY(true) => ("Увеличить масштаб по вертикали", "Увеличить масштаб по вертикали (ALT + колесико мыши)"),
        C::ZoomY(false) => ("Уменьшить масштаб по вертикали", "Уменьшить масштаб по вертикали (ALT + колесико мыши)"),
        C::ResetZoom => ("Вернуть сигнал к исходному размеру", "Вернуть к исходному размеру"),
        C::ToggleVizir(0) => ("Вертикальный визир 1", "Показать или скрыть вертикальный визир 1 (левая кнопка мыши по осциллограмме)"),
        C::ToggleVizir(1) => ("Вертикальный визир 2", "Показать или скрыть вертикальный визир 2 (правая кнопка мыши по осциллограмме)"),
        C::ToggleVizir(2) => ("Вертикальный визир 3", "Показать или скрыть вертикальный визир 3"),
        C::ToggleVizir(3) => ("Горизонтальный визир 1", "Показать или скрыть горизонтальный визир 1 (SHIFT + левая кнопка мыши по осциллограмме)"),
        C::ToggleVizir(_) => ("Горизонтальный визир 2", "Показать или скрыть горизонтальный визир 2 (SHIFT + правая кнопка мыши по осциллограмме)"),
        C::CommentMode(1) => ("Момент времени", "Комментирование момента времени"),
        C::CommentMode(3) => ("Область", "Выделение и комментирование области на осциллограмме"),
        C::CommentMode(_) => ("Линия уровня", "Комментирование уровня сигнала"),
        C::StatsRow(0) => ("Минимальное и максимальное значение", "Минимальное и максимальное значение"),
        C::StatsRow(1) => ("Среднеквадратическое значение", "Среднеквадратическое значение"),
        C::StatsRow(_) => ("Оценка первой гармоники", "Оценка первой гармоники по Фурье"),
        C::SetValueMode(ValueMode::Instant) => ("Мгновенные значения", "Отображать на графике осциллограммы мгновенные значения"),
        C::SetValueMode(ValueMode::Rms) => ("Среднеквадратичные значения", "Отображать на графике осциллограммы среднеквадратичные значения"),
        C::SetValueMode(_) => ("Значения первой гармоники", "Отображать на графике осциллограммы значения первой гармоники"),
        C::ToggleFlag(ChanFlag::Invert) => ("Инвертировать", "Инвертировать осциллограмму"),
        C::ToggleFlag(_) => ("Удалить постоянную", "Удалить постоянную составляющую осциллограммы"),
        C::Coefficients => ("Установить коэффициенты", "Установить коэффициенты для каналов"),
        C::SetPrimaryValues(true) => ("Первичные значения", "Отображать первичные значения величин"),
        C::SetPrimaryValues(false) => ("Вторичные значения", "Отображать вторичные значения величин"),
        // 137 is a bare string: the text is the description too.
        C::QuickAnalysis => ("Быстрый анализ", "Быстрый анализ"),
        C::ToggleParamBar => ("Параметры осциллограммы", "Показать или скрыть панель параметров осциллограммы"),
        C::OmpParams => ("Параметры ОМП", "Параметры определения места повреждения"),
        C::OmpAnalysis => ("Анализ ОМП", "Произвести определение места повреждения"),
        _ => return None,
    })
}

/// Where the ribbon's body ends (its tooltips open 3 px under it); set
/// by the ribbon each frame.
pub fn set_ribbon_bottom(ctx: &egui::Context, y: f32) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("fv_ribbon_bottom"), y));
}

/// A panel element's tooltip: `cmd`'s texts, under the ribbon at the
/// element's left.
pub fn ribbon_tip(ui: &egui::Ui, r: &egui::Response, img: TipImage, cmd: &Command) {
    let Some((title, desc)) = cmd_tip(cmd) else { return };
    let bottom = ui.ctx().data(|d| d.get_temp::<f32>(egui::Id::new("fv_ribbon_bottom"))).unwrap_or(r.rect.bottom());
    tip(ui, r, Pos2::new(r.rect.left(), bottom + 3.0), img, title, desc, true, 2.0);
}

/// A quick-access button's tooltip: at the pointer's x, under the caption
/// row (VM: y 31).
pub fn qat_tip(ui: &egui::Ui, r: &egui::Response, img: TipImage, title: &str, desc: &str) {
    let x = ui.ctx().pointer_hover_pos().map_or(r.rect.left(), |p| p.x);
    tip(ui, r, Pos2::new(x, ui.max_rect().top() + 31.0), img, title, desc, true, 2.0);
}

/// A toolbar button's tooltip: 18 px under the pointer, kept on the
/// screen; its title bold or not (VM: the panes' vertical toolbars write
/// it in the regular weight, «Параметры ОМП»'s toolbar in bold).
pub fn pointer_tip(ui: &egui::Ui, r: &egui::Response, img: TipImage, title: &str, desc: &str, bold: bool) {
    pointer_tip_inset(ui, r, img, title, desc, bold, 2.0);
}

/// [`pointer_tip`] kept `inset` px inside the screen's right edge: a
/// tooltip the system places (the ОМП bar's button, VM) stops 8 px short,
/// where CMFCToolBar's own placement leaves 2.
pub fn pointer_tip_inset(ui: &egui::Ui, r: &egui::Response, img: TipImage, title: &str, desc: &str, bold: bool, inset: f32) {
    let at = ui.ctx().pointer_hover_pos().unwrap_or(r.rect.left_bottom()) + Vec2::new(0.0, 18.0);
    tip(ui, r, at, img, title, desc, bold, inset);
}

/// The hover timing: shown after 0.5 s over `r` (restarted on entering it
/// again), not after a press until the pointer leaves.
#[allow(clippy::too_many_arguments)]
fn tip(ui: &egui::Ui, r: &egui::Response, at: Pos2, img: TipImage, title: &str, desc: &str, bold_title: bool, inset: f32) {
    if !r.contains_pointer() {
        return;
    }
    let ctx = ui.ctx();
    let key = egui::Id::new("fv_rtip");
    let (now, pass) = (ctx.input(|i| i.time), ctx.cumulative_pass_nr());
    let pressed = ctx.input(|i| i.pointer.any_down() || i.pointer.any_pressed());
    let (start, quiet) = ctx.data_mut(|d| {
        let s: &mut (egui::Id, f64, bool, u64) = d.get_temp_mut_or_insert_with(key, || (r.id, now, false, pass));
        if s.0 != r.id || s.3 + 1 < pass {
            *s = (r.id, now, false, pass);
        }
        s.3 = pass;
        s.2 |= pressed;
        (s.1, s.2)
    });
    if quiet {
        return;
    }
    let wait = 0.5 - (now - start);
    if wait > 0.0 {
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(wait));
        return;
    }
    paint(ctx, at, img, title, desc, bold_title, inset);
}

/// Words of `text` in lines no wider than `max` (a single longer word on
/// a line of its own).
fn wrap(ctx: &egui::Context, text: &str, font: &egui::FontId, max: f32) -> Vec<String> {
    let w = |t: &str| ctx.fonts(|f| f.layout_no_wrap(t.to_owned(), font.clone(), Color32::BLACK).size().x);
    let mut lines: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        match lines.last_mut() {
            Some(l) if w(&format!("{l} {word}")) <= max => {
                l.push(' ');
                l.push_str(word);
            }
            _ => lines.push(word.to_owned()),
        }
    }
    lines
}

fn paint(ctx: &egui::Context, at: Pos2, img: TipImage, title: &str, desc: &str, bold_title: bool, inset: f32) {
    const INK: Color32 = Color32::from_rgb(76, 76, 76);
    const PITCH: f32 = 13.0;
    let font = egui::FontId::proportional(12.0);
    let bold = if bold_title { ctx.fonts(|f| crate::lanes::bold_font(f, 12.0)).unwrap_or(font.clone()) } else { font.clone() };
    let width = |t: &str, f: &egui::FontId| ctx.fonts(|fs| fs.layout_no_wrap(t.to_owned(), f.clone(), INK).size().x);
    // Segoe UI wraps near 190 px; Tahoma's letters are a little wider.
    let lines = wrap(ctx, desc, &font, 195.0);
    let plain = matches!(img, TipImage::None);
    let text_x = match img {
        TipImage::Large(_) => 45.0,
        TipImage::Small(_) | TipImage::Mini(_) | TipImage::Tool(_) | TipImage::OmpTool(_) | TipImage::Help => 29.0,
        TipImage::None | TipImage::Blank => 6.0,
    };
    let text_w = lines.iter().map(|l| width(l, &font)).chain(std::iter::once(if plain { 0.0 } else { width(title, &bold) })).fold(0.0, f32::max);
    let size = if plain { Vec2::new((text_x + text_w + 7.0).round(), 21.0) } else { Vec2::new((text_x + text_w + 10.0).round(), 24.0 + PITCH * lines.len() as f32 + 5.0) };
    let screen = ctx.screen_rect();
    // Kept `inset` px inside the screen's right edge (VM).
    let at = Pos2::new(at.x.min(screen.right() - inset - size.x).max(screen.left()), at.y).round();
    egui::Area::new(egui::Id::new("fv_rtip_area")).order(egui::Order::Tooltip).fixed_pos(at).interactable(false).show(ctx, |ui| {
        let (r, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        let p = ui.painter();
        // The shadow: a few pixels right and below, fading.
        for k in 1..=4 {
            let a = 34 - 7 * k as u8;
            p.rect_filled(r.translate(Vec2::splat(k as f32)), 3.0, Color32::from_black_alpha(a));
        }
        p.rect_filled(r, 3.0, Color32::from_rgb(118, 118, 118));
        let face = r.shrink(1.0);
        let top = Color32::from_rgb(253, 253, 254);
        let bottom = Color32::from_rgb(203, 218, 239);
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(face.left_top(), top);
        mesh.colored_vertex(face.right_top(), top);
        mesh.colored_vertex(face.left_bottom(), bottom);
        mesh.colored_vertex(face.right_bottom(), bottom);
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(1, 2, 3);
        p.add(mesh);
        let accent = Color32::from_rgb(21, 66, 139);
        match img {
            TipImage::Large(icon) => crate::icons::paint(p, Rect::from_min_size(r.min + Vec2::new(12.0, 5.0), Vec2::splat(32.0)).shrink(2.0), icon, accent),
            TipImage::Small(icon) => crate::icons::paint(p, Rect::from_min_size(r.min + Vec2::new(9.0, 5.0), Vec2::splat(16.0)), icon, accent),
            TipImage::Mini(m) => crate::ribbon::paint_mini(p, Rect::from_min_size(r.min + Vec2::new(9.0, 5.0), Vec2::splat(16.0)), m, accent),
            // (the toolbar's 22 px cell, the glyph 4 px in)
            TipImage::Tool(t) => crate::diagram_ui::paint_tool(p, Rect::from_min_size(r.min + Vec2::new(5.0, 2.0), Vec2::splat(22.0)), t, accent),
            TipImage::OmpTool(t) => crate::omp_ui::paint_tool_icon(p, Rect::from_min_size(r.min + Vec2::new(9.0, 5.0), Vec2::splat(16.0)), t, true),
            TipImage::Help => crate::ribbon::help_image(p, r.min + Vec2::new(17.0, 13.0)),
            TipImage::None | TipImage::Blank => {}
        }
        if plain {
            p.text(r.min + Vec2::new(text_x, 4.0), egui::Align2::LEFT_TOP, desc, font.clone(), INK);
            return;
        }
        if bold_title {
            crate::lanes::bold_text(p, r.min + Vec2::new(text_x, 5.0), title, font.clone(), INK);
        } else {
            p.text(r.min + Vec2::new(text_x, 5.0), egui::Align2::LEFT_TOP, title, font.clone(), INK);
        }
        for (k, l) in lines.iter().enumerate() {
            // (the glyphs from + 26, as the original's)
            p.text(r.min + Vec2::new(text_x, 24.0 + PITCH * k as f32), egui::Align2::LEFT_TOP, l, font.clone(), INK);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The texts as the original's tooltips show them (VM); «Каналы» has none.
    #[test]
    fn commands_carry_their_prompt_texts() {
        assert_eq!(cmd_tip(&Command::TogglePane(Pane::Frequency)), Some(("Частотная диаграмма", "Показать частотную диаграмму")));
        assert_eq!(cmd_tip(&Command::Paste).map(|t| t.0), Some("Вставить (Ctrl+V)"));
        assert_eq!(cmd_tip(&Command::ChannelsDialog), None);
    }

    #[test]
    fn descriptions_wrap_by_words() {
        let ctx = egui::Context::default();
        ctx.set_fonts(crate::app_fonts());
        let _ = ctx.run(Default::default(), |_| {});
        let font = egui::FontId::proportional(12.0);
        let lines = wrap(&ctx, "Удалить постоянную составляющую осциллограммы", &font, 195.0);
        assert_eq!(lines, vec!["Удалить постоянную", "составляющую осциллограммы"]);
        assert_eq!(wrap(&ctx, "Комментирование уровня сигнала", &font, 195.0).len(), 1);
    }
}
