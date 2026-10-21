//! Office-2007-style skins for the FastView reconstruction.
//!
//! The original is an MFC ribbon app whose *Стиль* menu offers Office 2007 in
//! four colour variants (Голубой/Чёрный/Серебристый/Зеленовато-голубой — see
//! `notes/ui/menus.txt`, MENU 200). We can't reproduce the native GDI ribbon
//! byte-for-byte on a cross-platform stack, but we approximate its palette and
//! chrome closely in egui. `Office2007Blue` matches the help screenshots and is
//! the default.

use eframe::egui::{self, Color32};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skin {
    Blue,
    Black,
    Silver,
    Aqua,
}

impl Skin {
    pub const ALL: [Skin; 4] = [Skin::Blue, Skin::Black, Skin::Silver, Skin::Aqua];


    pub fn palette(self) -> Palette {
        match self {
            Skin::Blue => Palette {
                ribbon_top: Color32::from_rgb(0xC6, 0xD9, 0xF1),
                ribbon_bottom: Color32::from_rgb(0xA6, 0xC2, 0xE3),
                frame: Color32::from_rgb(0x3A, 0x5D, 0x8F),
                accent: Color32::from_rgb(0x2E, 0x5B, 0x9E),
                accent_soft: Color32::from_rgb(0xFF, 0xE4, 0x9B),
                accent_border: Color32::from_rgb(0xE0, 0xA8, 0x1E),
                group_bg: Color32::from_rgb(0xDF, 0xE9, 0xF6),
                group_border: Color32::from_rgb(0xB4, 0xC9, 0xE3),
                tab_active: Color32::from_rgb(0xF4, 0xF8, 0xFD),
                work_bg: Color32::from_rgb(0x9C, 0xB6, 0xD6),
                text: Color32::from_rgb(0x21, 0x37, 0x55),
            },
            Skin::Black => Palette {
                ribbon_top: Color32::from_rgb(0x74, 0x78, 0x7D),
                ribbon_bottom: Color32::from_rgb(0x50, 0x53, 0x57),
                frame: Color32::from_rgb(0x2B, 0x2C, 0x2E),
                accent: Color32::from_rgb(0x3A, 0x6E, 0xA5),
                accent_soft: Color32::from_rgb(0xF5, 0xD6, 0x7A),
                accent_border: Color32::from_rgb(0xC8, 0x95, 0x1A),
                group_bg: Color32::from_rgb(0x6A, 0x6D, 0x72),
                group_border: Color32::from_rgb(0x44, 0x46, 0x49),
                tab_active: Color32::from_rgb(0x83, 0x87, 0x8C),
                work_bg: Color32::from_rgb(0x58, 0x5B, 0x5F),
                text: Color32::from_rgb(0xF0, 0xF0, 0xF0),
            },
            Skin::Silver => Palette {
                ribbon_top: Color32::from_rgb(0xEE, 0xEF, 0xF2),
                ribbon_bottom: Color32::from_rgb(0xCE, 0xD1, 0xD8),
                frame: Color32::from_rgb(0x8A, 0x8D, 0x94),
                accent: Color32::from_rgb(0x55, 0x6A, 0x86),
                accent_soft: Color32::from_rgb(0xFF, 0xE4, 0x9B),
                accent_border: Color32::from_rgb(0xD9, 0xA8, 0x3A),
                group_bg: Color32::from_rgb(0xE6, 0xE8, 0xEC),
                group_border: Color32::from_rgb(0xC2, 0xC5, 0xCC),
                tab_active: Color32::from_rgb(0xFA, 0xFB, 0xFC),
                work_bg: Color32::from_rgb(0xBD, 0xC0, 0xC7),
                text: Color32::from_rgb(0x2C, 0x30, 0x36),
            },
            Skin::Aqua => Palette {
                ribbon_top: Color32::from_rgb(0xC4, 0xE4, 0xE4),
                ribbon_bottom: Color32::from_rgb(0x9C, 0xCB, 0xCE),
                frame: Color32::from_rgb(0x36, 0x75, 0x79),
                accent: Color32::from_rgb(0x1F, 0x79, 0x7E),
                accent_soft: Color32::from_rgb(0xFF, 0xE4, 0x9B),
                accent_border: Color32::from_rgb(0xD9, 0xA8, 0x3A),
                group_bg: Color32::from_rgb(0xD5, 0xEA, 0xEA),
                group_border: Color32::from_rgb(0xAC, 0xCE, 0xD0),
                tab_active: Color32::from_rgb(0xF0, 0xFA, 0xFA),
                work_bg: Color32::from_rgb(0x8F, 0xBF, 0xC2),
                text: Color32::from_rgb(0x14, 0x3B, 0x3D),
            },
        }
    }
}

/// Concrete colours derived from a skin.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub ribbon_top: Color32,
    pub ribbon_bottom: Color32,
    pub frame: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub accent_border: Color32,
    pub group_bg: Color32,
    pub group_border: Color32,
    pub tab_active: Color32,
    pub work_bg: Color32,
    pub text: Color32,
}

/// Apply a skin to the egui context (base widget visuals; the ribbon paints its
/// own gradients on top).
pub fn apply(ctx: &egui::Context, skin: Skin) {
    let p = skin.palette();
    let dark = matches!(skin, Skin::Black);
    let mut v = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.panel_fill = p.group_bg;
    v.window_fill = p.tab_active;
    v.extreme_bg_color = if dark {
        Color32::from_rgb(0x30, 0x32, 0x35)
    } else {
        Color32::WHITE
    };
    v.selection.bg_fill = p.accent_soft.gamma_multiply(0.9f32);
    v.selection.stroke = egui::Stroke::new(1.0f32, p.accent_border);
    v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0f32, p.text);
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0f32, p.frame);
    v.faint_bg_color = p.work_bg.gamma_multiply(0.4f32);
    v.widgets.inactive.bg_fill = p.group_bg;
    v.widgets.inactive.weak_bg_fill = p.group_bg;
    v.widgets.hovered.bg_fill = p.accent_soft;
    v.widgets.hovered.weak_bg_fill = p.accent_soft;
    v.widgets.active.bg_fill = p.accent_soft.gamma_multiply(0.85f32);
    v.hyperlink_color = p.accent;
    v.window_rounding = egui::Rounding::same(4.0f32);
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(6.0, 5.0);
        s.spacing.button_padding = egui::vec2(6.0, 3.0);
    });
}
