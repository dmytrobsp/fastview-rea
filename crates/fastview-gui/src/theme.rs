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
                tab_text: Color32::BLACK,
                status_bg: Color32::from_rgb(0xD4, 0xD0, 0xC8),
                status_text: Color32::BLACK,
                body: None,
                caption_band: None,
                caption_text: Color32::BLACK.gamma_multiply(0.8),
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
                tab_text: Color32::BLACK,
                status_bg: Color32::from_rgb(0xEC, 0xEA, 0xE5),
                status_text: Color32::BLACK,
                body: None,
                caption_band: None,
                caption_text: Color32::BLACK.gamma_multiply(0.8),
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
                tab_text: Color32::BLACK,
                status_bg: Color32::from_rgb(0xEC, 0xE9, 0xD8),
                status_text: Color32::BLACK,
                body: None,
                caption_band: None,
                caption_text: Color32::BLACK.gamma_multiply(0.8),
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
                tab_text: Color32::BLACK,
                status_bg: Color32::from_rgb(0xC3, 0xDA, 0xF9),
                status_text: Color32::BLACK,
                body: None,
                caption_band: None,
                caption_text: Color32::BLACK.gamma_multiply(0.8),
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
                tab_text: Color32::BLACK,
                status_bg: Color32::from_rgb(0xEE, 0xED, 0xE5),
                status_text: Color32::BLACK,
                body: None,
                caption_band: None,
                caption_text: Color32::BLACK.gamma_multiply(0.8),
            },
                        Skin::Blue => Palette {
                // The Office 2007 style data (L9.117): BARS/DEFAULT for the
                // band, the category body's top, RIBBON/PANEL/TextNormal, the
                // tab and status-bar texts, MAINWND/MainClientArea.
                ribbon_top: Color32::from_rgb(191, 219, 255),
                ribbon_bottom: Color32::from_rgb(191, 219, 255),
                frame: Color32::from_rgb(58, 93, 143),
                accent: Color32::from_rgb(46, 91, 158),
                accent_soft: Color32::from_rgb(255, 228, 150),
                accent_border: Color32::from_rgb(219, 206, 153),
                group_bg: Color32::from_rgb(223, 233, 246),
                group_border: Color32::from_rgb(174, 197, 214),
                tab_active: Color32::from_rgb(229, 237, 248),
                work_bg: Color32::from_rgb(164, 195, 235),
                text: Color32::from_rgb(21, 66, 139),
                disabled: Some(Color32::from_rgb(141, 141, 141)),
                tab_text: Color32::from_rgb(21, 66, 139),
                status_bg: Color32::from_rgb(186, 212, 247),
                status_text: Color32::from_rgb(21, 66, 139),
                body: Some([Color32::from_rgb(229, 237, 248), Color32::from_rgb(199, 216, 237), Color32::from_rgb(231, 242, 255)]),
                caption_band: Some([Color32::from_rgba_unmultiplied(152, 188, 232, 92), Color32::from_rgba_unmultiplied(133, 169, 216, 92)]),
                caption_text: Color32::from_rgb(62, 106, 170),
            },
                        Skin::Black => Palette {
                // The Office 2007 style data (L9.117): BARS/DEFAULT for the
                // band, the category body's top, RIBBON/PANEL/TextNormal, the
                // tab and status-bar texts, MAINWND/MainClientArea.
                ribbon_top: Color32::from_rgb(83, 83, 83),
                ribbon_bottom: Color32::from_rgb(83, 83, 83),
                frame: Color32::from_rgb(43, 44, 46),
                accent: Color32::from_rgb(58, 110, 165),
                accent_soft: Color32::from_rgb(255, 228, 150),
                accent_border: Color32::from_rgb(219, 206, 153),
                group_bg: Color32::from_rgb(225, 227, 231),
                group_border: Color32::from_rgb(146, 148, 152),
                tab_active: Color32::from_rgb(215, 219, 224),
                work_bg: Color32::from_rgb(70, 70, 70),
                text: Color32::from_rgb(0, 0, 0),
                disabled: Some(Color32::from_rgb(141, 141, 141)),
                tab_text: Color32::from_rgb(255, 255, 255),
                status_bg: Color32::from_rgb(47, 48, 48),
                status_text: Color32::from_rgb(255, 255, 255),
                body: Some([Color32::from_rgb(215, 219, 224), Color32::from_rgb(181, 188, 198), Color32::from_rgb(211, 211, 211)]),
                caption_band: Some([Color32::from_rgba_unmultiplied(159, 159, 159, 186), Color32::from_rgba_unmultiplied(137, 137, 137, 186)]),
                caption_text: Color32::from_rgb(255, 255, 255),
            },
                        Skin::Silver => Palette {
                // The Office 2007 style data (L9.117): BARS/DEFAULT for the
                // band, the category body's top, RIBBON/PANEL/TextNormal, the
                // tab and status-bar texts, MAINWND/MainClientArea.
                ribbon_top: Color32::from_rgb(208, 212, 221),
                ribbon_bottom: Color32::from_rgb(208, 212, 221),
                frame: Color32::from_rgb(138, 141, 148),
                accent: Color32::from_rgb(85, 106, 134),
                accent_soft: Color32::from_rgb(255, 228, 150),
                accent_border: Color32::from_rgb(219, 206, 153),
                group_bg: Color32::from_rgb(230, 232, 236),
                group_border: Color32::from_rgb(152, 154, 157),
                tab_active: Color32::from_rgb(242, 244, 248),
                work_bg: Color32::from_rgb(186, 193, 202),
                text: Color32::from_rgb(76, 83, 92),
                disabled: Some(Color32::from_rgb(141, 141, 141)),
                tab_text: Color32::from_rgb(76, 83, 92),
                status_bg: Color32::from_rgb(199, 201, 206),
                status_text: Color32::from_rgb(35, 38, 42),
                body: Some([Color32::from_rgb(242, 244, 248), Color32::from_rgb(213, 220, 231), Color32::from_rgb(249, 249, 249)]),
                caption_band: Some([Color32::from_rgba_unmultiplied(222, 226, 238, 186), Color32::from_rgba_unmultiplied(196, 201, 210, 186)]),
                caption_text: Color32::from_rgb(83, 84, 89),
            },
                        Skin::Aqua => Palette {
                // The Office 2007 style data (L9.117): BARS/DEFAULT for the
                // band, the category body's top, RIBBON/PANEL/TextNormal, the
                // tab and status-bar texts, MAINWND/MainClientArea.
                ribbon_top: Color32::from_rgb(196, 202, 217),
                ribbon_bottom: Color32::from_rgb(196, 202, 217),
                frame: Color32::from_rgb(96, 106, 128),
                accent: Color32::from_rgb(46, 74, 138),
                accent_soft: Color32::from_rgb(190, 201, 222),
                accent_border: Color32::from_rgb(143, 153, 181),
                group_bg: Color32::from_rgb(210, 215, 227),
                group_border: Color32::from_rgb(176, 184, 202),
                tab_active: Color32::from_rgb(195, 202, 217),
                work_bg: Color32::from_rgb(181, 189, 211),
                text: Color32::from_rgb(0, 21, 110),
                disabled: Some(Color32::from_rgb(159, 141, 154)),
                tab_text: Color32::from_rgb(0, 21, 110),
                status_bg: Color32::from_rgb(209, 215, 229),
                status_text: Color32::from_rgb(0, 21, 110),
                body: None,
                caption_band: Some([Color32::from_rgb(106, 112, 128), Color32::from_rgb(163, 174, 199)]),
                caption_text: Color32::from_rgb(255, 255, 255),
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
    /// Inactive category-tab labels (RIBBON/CATEGORY/TAB/TextNormal).
    pub tab_text: Color32,
    /// The status bar's fill and text.
    pub status_bg: Color32,
    pub status_text: Color32,
    /// The category body (RIBBON_CATEGORY_BACK): top, the dip 18 px down,
    /// bottom; `None` a flat `tab_active`.
    pub body: Option<[Color32; 3]>,
    /// The panels' caption band (RIBBON_PANEL_BACK_B, 15 px, translucent)
    /// and caption text (RIBBON/PANEL/CAPTION/TextNormal).
    pub caption_band: Option<[Color32; 2]>,
    pub caption_text: Color32,
}

/// Apply a skin to the egui context (base widget visuals; the ribbon paints its
/// own gradients on top).
pub fn apply(ctx: &egui::Context, skin: Skin) {
    let p = skin.palette();
    // Office 2007 Black has a light ribbon and dialogs inside a dark frame.
    let dark = false;
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
    // egui draws selected text in the selection stroke's colour: the
    // label colour on the light highlight (the style's own hot border is
    // too light for text).
    v.selection.stroke = egui::Stroke::new(1.0f32, p.text);
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
