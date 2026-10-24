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
                bands: None,
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
                bands: None,
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
                bands: None,
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
                bands: None,
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
                bands: None,
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
                caption_band: Some([alpha(152, 188, 232, 92), alpha(133, 169, 216, 92)]),
                caption_text: Color32::from_rgb(62, 106, 170),
                app_orb: Some([Color32::from_rgb(215, 221, 229), Color32::from_rgb(247, 222, 89), Color32::from_rgb(205, 126, 14)]),
                orb_flat: None,
                info_bar: None,
                pane_cap: None,
                status_flat: None,
                mdi_tabs: Some([Color32::from_rgb(179, 211, 255), Color32::from_rgb(228, 239, 255)]),
                bands: Some(Bands { under_ribbon: [Color32::from_rgb(157, 179, 209), Color32::from_rgb(174, 199, 232), Color32::from_rgb(185, 212, 247)], bar_line: Color32::from_rgb(157, 189, 216), bar_bottom: Color32::from_rgb(213, 228, 242), mdi_border: Color32::from_rgb(185, 208, 237) , caption: Color32::from_rgb(184, 207, 233), toolbar: [Color32::from_rgb(227, 239, 255), Color32::from_rgb(177, 211, 255)] , status: [0x567DB0, 0xFFFFFF, 0xD7E6F9, 0xD3E4FA, 0xD0E2FB, 0xCCE0FB, 0xC5DBF8, 0xC7DCF8, 0xB3D0F5, 0xB3D0F5, 0xB6D1F5, 0xB6D1F5, 0xBAD3F5, 0xBAD3F5, 0xBDD6F5, 0xBDD6F5, 0xC1D8F6, 0xC1D8F6, 0xC5DAF6, 0xC5DAF6, 0xC5DAF6, 0xC9DDF6, 0xC9DDF6, 0xCDDFF6], status_pane: [0x567DB0, 0xC5DCF8, 0xC2D9F7, 0xBBD5F6, 0xB4D0F5, 0xAECDF4, 0xADCCF6, 0xA9CAF7, 0x90B6EA, 0x90B6EA, 0x8EB4E7, 0x8EB4E7, 0x8BB2E5, 0x8BB2E5, 0x89AFE1, 0x89AFE1, 0x87ACDE, 0x87ACDE, 0x84A9DA, 0x84A9DA, 0x84A9DA, 0x82A6D7, 0x82A6D7, 0x7FA3D3], pane_edge: [Color32::from_rgb(229, 238, 251), Color32::from_rgb(105, 141, 189)] }),
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
                caption_band: Some([alpha(159, 159, 159, 186), alpha(137, 137, 137, 186)]),
                caption_text: Color32::from_rgb(255, 255, 255),
                app_orb: Some([Color32::from_rgb(223, 223, 223), Color32::from_rgb(247, 222, 89), Color32::from_rgb(205, 126, 14)]),
                orb_flat: None,
                info_bar: None,
                pane_cap: None,
                status_flat: None,
                mdi_tabs: None,
                bands: Some(Bands { under_ribbon: [Color32::from_rgb(61, 61, 61), Color32::from_rgb(66, 66, 66), Color32::from_rgb(77, 77, 77)], bar_line: Color32::from_rgb(129, 129, 129), bar_bottom: Color32::from_rgb(235, 235, 235), mdi_border: Color32::from_rgb(172, 172, 172) , caption: Color32::from_rgb(158, 160, 160), toolbar: [Color32::from_rgb(205, 208, 213), Color32::from_rgb(148, 156, 166)] , status: [0x414141, 0x434752, 0x424651, 0x40434E, 0x3E414B, 0x3C404A, 0x3C404A, 0x3B3E46, 0x2F3030, 0x2F3030, 0x2F2F2F, 0x2F2F2F, 0x302F2F, 0x302F2F, 0x30302F, 0x30302F, 0x313130, 0x313130, 0x333332, 0x333332, 0x333332, 0x353534, 0x353534, 0x373736], status_pane: [0x333333, 0xD8D7D8, 0xC6C5C6, 0xC3C3C3, 0xC0BFC0, 0xBBBBBB, 0xB7B7B7, 0xB4B4B4, 0x979898, 0x979898, 0x979898, 0x979898, 0x929496, 0x929496, 0x8C9093, 0x8C9093, 0x858A8E, 0x858A8E, 0x7E848A, 0x7E848A, 0x7E848A, 0x787F86, 0x787F86, 0x717981], pane_edge: [Color32::from_rgb(150, 150, 150), Color32::from_rgb(0, 0, 0)] }),
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
                caption_band: Some([alpha(222, 226, 238, 186), alpha(196, 201, 210, 186)]),
                caption_text: Color32::from_rgb(83, 84, 89),
                app_orb: Some([Color32::from_rgb(223, 223, 223), Color32::from_rgb(247, 222, 89), Color32::from_rgb(205, 126, 14)]),
                orb_flat: None,
                info_bar: None,
                pane_cap: None,
                status_flat: None,
                mdi_tabs: None,
                bands: Some(Bands { under_ribbon: [Color32::from_rgb(152, 155, 161), Color32::from_rgb(166, 169, 176), Color32::from_rgb(194, 198, 206)], bar_line: Color32::from_rgb(124, 124, 124), bar_bottom: Color32::from_rgb(233, 238, 247), mdi_border: Color32::from_rgb(172, 175, 183) , caption: Color32::from_rgb(186, 193, 202), toolbar: [Color32::from_rgb(233, 238, 247), Color32::from_rgb(209, 214, 222)] , status: [0xACAFB7, 0xE7E8EB, 0xE3E4E8, 0xDCDEE3, 0xD4D7DD, 0xCBCFD7, 0xC3C7D1, 0xBBC0CC, 0xB5BBC7, 0xB5BBC7, 0xA9AFB6, 0xA9AFB6, 0xAAB0B7, 0xAAB0B7, 0xADB3B9, 0xADB3B9, 0xB0B5BC, 0xB0B5BC, 0xB4B9BF, 0xB4B9BF, 0xB4B9BF, 0xB8BCC2, 0xB8BCC2, 0xBCBFC5], status_pane: [0xACAFB7, 0xE7E8EB, 0xE3E4E8, 0xDCDFE3, 0xD5D8DE, 0xCDD0D9, 0xC4C9D3, 0xBDC1CD, 0xB6BBC7, 0xB6BBC7, 0xA5ABAF, 0xA5ABAF, 0xA6ABAF, 0xA6ABAF, 0xA7ABB0, 0xA7ABB0, 0xA7ACB2, 0xA7ACB2, 0xA9AEB4, 0xA9AEB4, 0xA9AEB4, 0xAAB0B5, 0xAAB0B5, 0xACB2B7], pane_edge: [Color32::from_rgb(225, 225, 225), Color32::from_rgb(152, 152, 152)] }),
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
                bands: Some(Bands { under_ribbon: [Color32::from_rgb(195, 202, 217), Color32::from_rgb(137, 152, 163), Color32::from_rgb(237, 239, 243)], bar_line: Color32::from_rgb(106, 112, 128), bar_bottom: Color32::from_rgb(206, 212, 225), mdi_border: Color32::from_rgb(177, 187, 198) , caption: Color32::from_rgb(181, 189, 211), toolbar: [Color32::from_rgb(225, 226, 236), Color32::from_rgb(150, 148, 177)] , status: [0x778299, 0xEEF0F6, 0xD8DFEB, 0xD8DEEA, 0xD7DDE9, 0xD5DBE9, 0xD4DBE7, 0xD3DAE7, 0xD1D7E5, 0xD1D7E5, 0xD0D6E4, 0xD0D6E4, 0xCED5E3, 0xCED5E3, 0xCDD3E2, 0xCDD3E2, 0xC0C8DB, 0xC0C8DB, 0xBEC6D9, 0xBEC6D9, 0xBEC6D9, 0xBBC4D7, 0xBBC4D7, 0xB9C2D6], status_pane: [0x778299, 0xDCE3F0, 0xDDE3F1, 0xDEE4F2, 0xDFE5F3, 0xE0E7F4, 0xE1E7F5, 0xE2E8F6, 0xE2E8F6, 0xE2E8F6, 0xE2E8F7, 0xE2E8F7, 0xE2E8F7, 0xE2E8F7, 0xE2E8F7, 0xE2E8F7, 0xCED5E7, 0xCED5E7, 0xCCD3E5, 0xCCD3E5, 0xCCD3E5, 0xC9D0E2, 0xC9D0E2, 0xC7CEE1], pane_edge: [Color32::from_rgb(190, 198, 217), Color32::from_rgb(119, 130, 153)] }),
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
    /// The MDI tab row's fill, top and bottom of its 20 px (VM: the older
    /// looks; Blue's rows (180, 212, 255) → (224, 237, 255) at the pixel
    /// centres of the 19 above its bottom line).
    pub mdi_tabs: Option<[Color32; 2]>,
    /// The Office 2007 frame's bands (VM): the three rows under the ribbon
    /// (its bottom line, two shades), the parameters bar's lines and its
    /// gradient's bottom (from white), the document area's border.
    pub bands: Option<Bands>,
}

#[derive(Clone, Copy, Debug)]
pub struct Bands {
    pub under_ribbon: [Color32; 3],
    pub bar_line: Color32,
    pub bar_bottom: Color32,
    pub mdi_border: Color32,
    /// A docking pane's caption band (under a `bar_bottom` line) and its
    /// toolbar's horizontal gradient, left to right.
    pub caption: Color32,
    pub toolbar: [Color32; 2],
    /// The status bar's 24 rows (RGB, top line first) — its bitmap,
    /// uniform along x — for the bar and for its last pane, whose left edge
    /// is a light and a dark column.
    pub status: [u32; 24],
    pub status_pane: [u32; 24],
    pub pane_edge: [Color32; 2],
}

impl Palette {
    /// A docking pane's caption band: Office 2007 blue's (184, 207, 233)
    /// as captured on the VM; the other looks between their panel fill and
    /// border (not read).
    pub fn pane_caption(&self) -> Color32 {
        if let Some(b) = self.bands {
            b.caption
        } else {
            self.group_bg.lerp_to_gamma(self.group_border, 0.5)
        }
    }
}

/// A colour at alpha `a` as Windows blends it (GDI+ / AlphaBlend: in gamma
/// space, `dst + (src − dst)·a/255`): premultiplied in gamma space, which
/// egui's blending then reproduces. `Color32::from_rgba_unmultiplied`
/// premultiplies in linear light instead, which brightens the colour and
/// clamps where layers overlap (VM: the diagram fills, L9.224).
pub const fn alpha(r: u8, g: u8, b: u8, a: u8) -> Color32 {
    const fn m(c: u8, a: u8) -> u8 {
        ((c as u32 * a as u32 + 127) / 255) as u8
    }
    Color32::from_rgba_premultiplied(m(r, a), m(g, a), m(b, a), a)
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

#[cfg(test)]
mod alpha_tests {
    use super::*;

    /// Source-over as egui composes a premultiplied colour, in gamma space.
    fn over(dst: (f32, f32, f32), c: Color32) -> (f32, f32, f32) {
        let k = 1.0 - c.a() as f32 / 255.0;
        (c.r() as f32 + dst.0 * k, c.g() as f32 + dst.1 * k, c.b() as f32 + dst.2 * k)
    }

    /// The VM's differential plane (L9.224): the green, magenta and cyan
    /// fills at 50/255 stacked over white, last → first.
    #[test]
    fn alpha_stacks_as_gdiplus() {
        let [g, m, c] = [(34, 177, 76), (202, 1, 252), (25, 245, 253)].map(|(r, gg, b)| alpha(r, gg, b, 50));
        let near = |a: (f32, f32, f32), b: (u8, u8, u8)| (a.0 - b.0 as f32).abs() <= 1.0 && (a.1 - b.1 as f32).abs() <= 1.0 && (a.2 - b.2 as f32).abs() <= 1.0;
        let white = (255.0, 255.0, 255.0);
        assert!(near(over(white, g), (212, 240, 220)));
        assert!(near(over(over(white, g), m), (210, 193, 226)));
        assert!(near(over(over(over(white, g), m), c), (174, 203, 232)));
        assert!(near(over(over(white, g), c), (175, 241, 227)));
        assert!(near(over(white, c), (210, 253, 255)));
    }
}
