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
    /// The splitter strip's brush (`0x438110`, by the frame's look id):
    /// Black (83, 83, 83), Silver (208, 212, 221), Aqua (196, 202, 217),
    /// every other look (191, 219, 255).
    pub fn splitter(self) -> Color32 {
        match self {
            Skin::Black => Color32::from_rgb(83, 83, 83),
            Skin::Silver => Color32::from_rgb(208, 212, 221),
            Skin::Aqua => Color32::from_rgb(196, 202, 217),
            _ => Color32::from_rgb(191, 219, 255),
        }
    }

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
            // The pre-2007 visual managers (210–214: CMFCVisualManager,
            // …OfficeXP, …Windows, …Office2003, …VS2005 — `0x475250`) draw
            // with the system colours: these are Windows 11's as measured
            // on the VM (W2.2). They keep the round application button,
            // drawn flat; the parameters strip is INFOBK; the panels'
            // caption strips INACTIVECAPTION; the status bar flat BTNFACE.
            // Windows 2000: BTNFACE everywhere, captions INACTIVECAPTION.
            Skin::Win2000 => Palette {
                ribbon_top: Color32::from_rgb(240, 240, 240),
                ribbon_bottom: Color32::from_rgb(240, 240, 240),
                frame: Color32::from_rgb(160, 160, 160),
                accent: Color32::from_rgb(0, 120, 212),
                accent_soft: Color32::from_rgb(247, 247, 247),
                accent_border: Color32::from_rgb(160, 160, 160),
                group_bg: Color32::from_rgb(240, 240, 240),
                group_border: Color32::from_rgb(160, 160, 160),
                tab_active: Color32::from_rgb(240, 240, 240),
                work_bg: Color32::from_rgb(171, 171, 171),
                text: Color32::BLACK,
                disabled: None,
                tab_text: Color32::BLACK,
                status_bg: Color32::from_gray(240),
                status_text: Color32::BLACK,
                body: None,
                caption_band: Some([Color32::from_rgb(191, 205, 219); 2]),
                caption_text: Color32::BLACK,
                app_orb: Some([Color32::from_rgb(240, 240, 240); 3]),
                orb_flat: Some(Color32::from_rgb(160, 160, 160)),
                info_bar: Some(Color32::from_rgb(255, 255, 225)),
                pane_cap: Some([Color32::from_rgb(191, 205, 219), Color32::from_rgb(191, 205, 219), Color32::from_rgb(0, 0, 0)]),
                status_flat: Some(None),
                mdi_tabs: Some([Color32::from_gray(240); 2]),
            },
            // Office XP: the checked buttons HIGHLIGHT-tinted, captions BTNFACE.
            Skin::OfficeXp => Palette {
                ribbon_top: Color32::from_rgb(240, 240, 240),
                ribbon_bottom: Color32::from_rgb(240, 240, 240),
                frame: Color32::from_rgb(160, 160, 160),
                accent: Color32::from_rgb(0, 120, 212),
                accent_soft: Color32::from_rgb(210, 230, 245),
                accent_border: Color32::from_rgb(0, 120, 212),
                group_bg: Color32::from_rgb(240, 240, 240),
                group_border: Color32::from_rgb(160, 160, 160),
                tab_active: Color32::from_rgb(240, 240, 240),
                work_bg: Color32::from_rgb(171, 171, 171),
                text: Color32::BLACK,
                disabled: None,
                tab_text: Color32::BLACK,
                status_bg: Color32::from_gray(240),
                status_text: Color32::BLACK,
                body: None,
                caption_band: Some([Color32::from_rgb(191, 205, 219); 2]),
                caption_text: Color32::BLACK,
                app_orb: Some([Color32::from_rgb(240, 240, 240); 3]),
                orb_flat: Some(Color32::from_rgb(160, 160, 160)),
                info_bar: Some(Color32::from_rgb(255, 255, 225)),
                pane_cap: Some([Color32::from_rgb(240, 240, 240), Color32::from_rgb(240, 240, 240), Color32::from_rgb(0, 0, 0)]),
                status_flat: Some(None),
                mdi_tabs: Some([Color32::from_gray(247); 2]),
            },
            // Windows XP: a white tab row, captions INACTIVECAPTION → GRADIENTINACTIVECAPTION.
            Skin::WinXp => Palette {
                ribbon_top: Color32::from_rgb(255, 255, 255),
                ribbon_bottom: Color32::from_rgb(255, 255, 255),
                frame: Color32::from_rgb(160, 160, 160),
                accent: Color32::from_rgb(0, 120, 212),
                accent_soft: Color32::from_rgb(210, 230, 245),
                accent_border: Color32::from_rgb(0, 120, 212),
                group_bg: Color32::from_rgb(240, 240, 240),
                group_border: Color32::from_rgb(180, 180, 180),
                tab_active: Color32::from_rgb(240, 240, 240),
                work_bg: Color32::from_rgb(171, 171, 171),
                text: Color32::BLACK,
                disabled: None,
                tab_text: Color32::BLACK,
                status_bg: Color32::from_gray(240),
                status_text: Color32::BLACK,
                body: None,
                caption_band: Some([Color32::from_rgb(191, 205, 219); 2]),
                caption_text: Color32::BLACK,
                app_orb: Some([Color32::from_rgb(240, 240, 240); 3]),
                orb_flat: Some(Color32::from_rgb(160, 160, 160)),
                info_bar: Some(Color32::from_rgb(255, 255, 225)),
                pane_cap: Some([Color32::from_rgb(191, 205, 219), Color32::from_rgb(213, 226, 240), Color32::from_rgb(0, 0, 0)]),
                status_flat: Some(Some(Color32::from_rgb(215, 215, 215))),
                mdi_tabs: Some([Color32::from_gray(247); 2]),
            },
            // Office 2003: HIGHLIGHT-derived blues, the checked buttons orange.
            Skin::Office2003 => Palette {
                ribbon_top: Color32::from_rgb(167, 208, 240),
                ribbon_bottom: Color32::from_rgb(167, 208, 240),
                frame: Color32::from_rgb(160, 160, 160),
                accent: Color32::from_rgb(0, 120, 212),
                accent_soft: Color32::from_rgb(252, 178, 91),
                accent_border: Color32::from_rgb(44, 72, 92),
                group_bg: Color32::from_rgb(197, 230, 255),
                group_border: Color32::from_rgb(41, 176, 255),
                tab_active: Color32::from_rgb(197, 230, 255),
                work_bg: Color32::from_rgb(171, 171, 171),
                text: Color32::BLACK,
                disabled: None,
                tab_text: Color32::BLACK,
                status_bg: Color32::from_gray(240),
                status_text: Color32::BLACK,
                body: None,
                caption_band: Some([Color32::from_rgb(191, 205, 219); 2]),
                caption_text: Color32::BLACK,
                app_orb: Some([Color32::from_rgb(197, 230, 255); 3]),
                orb_flat: Some(Color32::from_rgb(41, 176, 255)),
                info_bar: Some(Color32::from_rgb(255, 255, 225)),
                pane_cap: Some([Color32::from_rgb(208, 234, 254), Color32::from_rgb(118, 186, 236), Color32::from_rgb(0, 0, 0)]),
                status_flat: Some(Some(Color32::from_rgb(215, 215, 215))),
                mdi_tabs: Some([Color32::from_rgb(120, 187, 237), Color32::from_rgb(205, 232, 254)]),
            },
            // Visual Studio .NET 2005: grey captions with white text.
            Skin::Vs2005 => Palette {
                ribbon_top: Color32::from_rgb(229, 229, 229),
                ribbon_bottom: Color32::from_rgb(229, 229, 229),
                frame: Color32::from_rgb(160, 160, 160),
                accent: Color32::from_rgb(0, 120, 212),
                accent_soft: Color32::from_rgb(210, 230, 245),
                accent_border: Color32::from_rgb(0, 120, 212),
                group_bg: Color32::from_rgb(240, 240, 240),
                group_border: Color32::from_rgb(160, 160, 160),
                tab_active: Color32::from_rgb(240, 240, 240),
                work_bg: Color32::from_rgb(171, 171, 171),
                text: Color32::BLACK,
                disabled: None,
                tab_text: Color32::BLACK,
                status_bg: Color32::from_gray(240),
                status_text: Color32::BLACK,
                body: None,
                caption_band: Some([Color32::from_rgb(191, 205, 219); 2]),
                caption_text: Color32::BLACK,
                app_orb: Some([Color32::from_rgb(240, 240, 240); 3]),
                orb_flat: Some(Color32::from_rgb(160, 160, 160)),
                info_bar: Some(Color32::from_rgb(255, 255, 225)),
                pane_cap: Some([Color32::from_rgb(188, 188, 188), Color32::from_rgb(188, 188, 188), Color32::from_rgb(255, 255, 255)]),
                status_flat: Some(Some(Color32::from_rgb(215, 215, 215))),
                mdi_tabs: Some([Color32::from_gray(240); 2]),
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
                app_orb: Some([Color32::from_rgb(215, 221, 229), Color32::from_rgb(247, 222, 89), Color32::from_rgb(205, 126, 14)]),
                orb_flat: None,
                info_bar: None,
                pane_cap: None,
                status_flat: None,
                mdi_tabs: None,
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
                app_orb: Some([Color32::from_rgb(223, 223, 223), Color32::from_rgb(247, 222, 89), Color32::from_rgb(205, 126, 14)]),
                orb_flat: None,
                info_bar: None,
                pane_cap: None,
                status_flat: None,
                mdi_tabs: None,
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
                app_orb: Some([Color32::from_rgb(223, 223, 223), Color32::from_rgb(247, 222, 89), Color32::from_rgb(205, 126, 14)]),
                orb_flat: None,
                info_bar: None,
                pane_cap: None,
                status_flat: None,
                mdi_tabs: None,
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
                app_orb: Some([Color32::from_rgb(209, 217, 232), Color32::from_rgb(146, 200, 248), Color32::from_rgb(215, 229, 250)]),
                orb_flat: None,
                info_bar: None,
                pane_cap: None,
                status_flat: None,
                mdi_tabs: None,
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
    /// The round application button's centre, normal / hot / pressed
    /// (RIBBON_BTN_MAIN); `None`: a text button.
    pub app_orb: Option<[Color32; 3]>,
    /// The older looks draw that button flat (VM): a 41 px circle in the
    /// body colour with this 1 px outline, no gloss.
    pub orb_flat: Option<Color32>,
    /// The parameters strip's fill when not the panels' colour: the older
    /// looks' INFOBK message bar (VM: (255, 255, 225)).
    pub info_bar: Option<Color32>,
    /// A docking pane's caption: top, bottom, text — when not the Office
    /// 2007 band (`pane_caption`).
    pub pane_cap: Option<[Color32; 3]>,
    /// The status bar flat in `status_bg`, its last pane too, with this top
    /// line (`Some(None)`: none) — the older looks (VM).
    pub status_flat: Option<Option<Color32>>,
    /// The MDI tab row's fill, top and bottom, in the older looks (VM).
    pub mdi_tabs: Option<[Color32; 2]>,
}

impl Palette {
    /// A docking pane's caption band: Office 2007 blue's (184, 207, 233)
    /// as captured on the VM; the other looks between their panel fill and
    /// border (not read).
    pub fn pane_caption(&self) -> Color32 {
        if self.group_bg == Color32::from_rgb(223, 233, 246) {
            Color32::from_rgb(184, 207, 233)
        } else {
            self.group_bg.lerp_to_gamma(self.group_border, 0.5)
        }
    }
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
