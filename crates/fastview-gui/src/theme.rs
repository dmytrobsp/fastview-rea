//! Office-2007-style skins for the FastView reconstruction.
//!
//! The original is an MFC ribbon app whose *Стиль* menu (MENU 200) offers five
//! classic visual managers (Windows 2000 … VS.NET 2005) and Office 2007 in
//! four colour variants (Голубой/Чёрный/Серебристый/Зеленовато-голубой). We can't reproduce the native GDI ribbon
//! byte-for-byte on a cross-platform stack, but we approximate its palette and
//! chrome closely in egui. `Office2007Blue` matches the help screenshots and is
//! the default.

use eframe::egui::{self, Color32};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skin {
    Win2000,
    OfficeXp,
    WinXp,
    Office2003,
    Vs2005,
    Blue,
    Black,
    Silver,
    Aqua,
}

impl Skin {
    /// In `ApplicationLook` order: 210 Windows 2000 … 214 VS.NET 2005, then
    /// the Office 2007 styles 215 … 218 (`0x475250`).
    pub const ALL: [Skin; 9] =
        [Skin::Win2000, Skin::OfficeXp, Skin::WinXp, Skin::Office2003, Skin::Vs2005, Skin::Blue, Skin::Black, Skin::Silver, Skin::Aqua];

    /// The `ApplicationLook` / menu-200 command id.
    pub fn code(self) -> u32 {
        210 + Skin::ALL.iter().position(|&k| k == self).unwrap_or(5) as u32
    }

    pub fn from_code(code: u32) -> Option<Skin> {
        Skin::ALL.get(code.checked_sub(210)? as usize).copied()
    }

    pub fn palette(self) -> Palette {
        match self {
            // The pre-2007 visual managers draw with the system colours of
            // their era; these palettes are those defaults (inferred, not
            // read from mfc120u.dll).
            Skin::Win2000 => Palette {
                ribbon_top: Color32::from_rgb(0xD4, 0xD0, 0xC8),
                ribbon_bottom: Color32::from_rgb(0xD4, 0xD0, 0xC8),
                frame: Color32::from_rgb(0x80, 0x80, 0x80),
                accent: Color32::from_rgb(0x0A, 0x24, 0x6A),
                accent_soft: Color32::from_rgb(0xB6, 0xBD, 0xD2),
                accent_border: Color32::from_rgb(0x0A, 0x24, 0x6A),
                group_bg: Color32::from_rgb(0xD4, 0xD0, 0xC8),
                group_border: Color32::from_rgb(0x80, 0x80, 0x80),
                tab_active: Color32::from_rgb(0xE4, 0xE1, 0xDC),
                work_bg: Color32::from_rgb(0x80, 0x80, 0x80),
                text: Color32::BLACK,
                disabled: None,
            },
            Skin::OfficeXp => Palette {
                ribbon_top: Color32::from_rgb(0xF7, 0xF6, 0xF3),
                ribbon_bottom: Color32::from_rgb(0xDB, 0xD8, 0xD1),
                frame: Color32::from_rgb(0x84, 0x82, 0x7B),
                accent: Color32::from_rgb(0x0A, 0x24, 0x6A),
                accent_soft: Color32::from_rgb(0xB6, 0xBD, 0xD2),
                accent_border: Color32::from_rgb(0x0A, 0x24, 0x6A),
                group_bg: Color32::from_rgb(0xEC, 0xEA, 0xE5),
                group_border: Color32::from_rgb(0xA6, 0xA6, 0xA6),
                tab_active: Color32::from_rgb(0xFB, 0xFA, 0xF8),
                work_bg: Color32::from_rgb(0x80, 0x80, 0x80),
                text: Color32::BLACK,
                disabled: None,
            },
            Skin::WinXp => Palette {
                ribbon_top: Color32::from_rgb(0xF6, 0xF5, 0xEE),
                ribbon_bottom: Color32::from_rgb(0xE3, 0xE0, 0xD0),
                frame: Color32::from_rgb(0x00, 0x3C, 0xA6),
                accent: Color32::from_rgb(0x31, 0x6A, 0xC5),
                accent_soft: Color32::from_rgb(0xC1, 0xD2, 0xEE),
                accent_border: Color32::from_rgb(0x31, 0x6A, 0xC5),
                group_bg: Color32::from_rgb(0xEC, 0xE9, 0xD8),
                group_border: Color32::from_rgb(0xAC, 0xA8, 0x99),
                tab_active: Color32::from_rgb(0xFC, 0xFC, 0xFE),
                work_bg: Color32::from_rgb(0x80, 0x80, 0x80),
                text: Color32::BLACK,
                disabled: None,
            },
            Skin::Office2003 => Palette {
                ribbon_top: Color32::from_rgb(0xE3, 0xEF, 0xFF),
                ribbon_bottom: Color32::from_rgb(0x9E, 0xBE, 0xF5),
                frame: Color32::from_rgb(0x00, 0x2D, 0x96),
                accent: Color32::from_rgb(0x00, 0x00, 0x80),
                accent_soft: Color32::from_rgb(0xFF, 0xEE, 0xC2),
                accent_border: Color32::from_rgb(0x00, 0x00, 0x80),
                group_bg: Color32::from_rgb(0xC3, 0xDA, 0xF9),
                group_border: Color32::from_rgb(0x7B, 0xA4, 0xE0),
                tab_active: Color32::from_rgb(0xF6, 0xFA, 0xFF),
                work_bg: Color32::from_rgb(0x90, 0x99, 0xAE),
                text: Color32::BLACK,
                disabled: None,
            },
            Skin::Vs2005 => Palette {
                ribbon_top: Color32::from_rgb(0xFA, 0xF9, 0xF5),
                ribbon_bottom: Color32::from_rgb(0xE5, 0xE2, 0xD8),
                frame: Color32::from_rgb(0x80, 0x80, 0x80),
                accent: Color32::from_rgb(0x31, 0x6A, 0xC5),
                accent_soft: Color32::from_rgb(0xC1, 0xD2, 0xEE),
                accent_border: Color32::from_rgb(0x31, 0x6A, 0xC5),
                group_bg: Color32::from_rgb(0xEE, 0xED, 0xE5),
                group_border: Color32::from_rgb(0xB5, 0xB2, 0xA6),
                tab_active: Color32::from_rgb(0xFF, 0xFF, 0xFF),
                work_bg: Color32::from_rgb(0x80, 0x80, 0x80),
                text: Color32::BLACK,
                disabled: None,
            },
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
                disabled: Some(Color32::from_rgb(141, 141, 141)),
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
                // The read (141, 141, 141) needs the style's light panel body;
                // ours is dark (the Black palette not yet read).
                disabled: None,
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
                disabled: Some(Color32::from_rgb(141, 141, 141)),
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
                disabled: Some(Color32::from_rgb(159, 141, 154)),
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
    /// Disabled labels: the Office 2007 styles' fixed BARS/TOOLBAR
    /// TextDisabled (`0x10186ca0`); the older looks use the text at half
    /// strength.
    pub disabled: Option<Color32>,
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
    // Idle controls must stand out from the panel they sit on: a white
    // check / radio / edit interior, a lighter button face and a frame.
    v.widgets.inactive.bg_fill = v.extreme_bg_color;
    v.widgets.inactive.weak_bg_fill = p.tab_active;
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0f32, p.frame);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Check boxes, radio buttons, combo boxes and buttons are visible on
    /// the panels in every look (they used to be panel-coloured, unframed).
    #[test]
    fn idle_controls_stand_out_from_the_panel() {
        for skin in Skin::ALL {
            let ctx = egui::Context::default();
            apply(&ctx, skin);
            let v = ctx.style().visuals.clone();
            let w = &v.widgets.inactive;
            assert_ne!(w.bg_fill, v.panel_fill, "{skin:?}");
            assert!(w.bg_stroke.width > 0.0 && w.bg_stroke.color != v.panel_fill, "{skin:?}");
        }
    }
}
