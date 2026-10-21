//! The clipboard that carries «Mechanotronica FastView Clipboard Data»
//! (ledger L9.20). On Windows it is the system clipboard under that
//! registered format (`0x404f5e`, UTF-16 text with a trailing NUL, no other
//! format); elsewhere a file in our settings folder stands in for it, so two
//! instances on one machine can still merge. Tests use the in-memory kind.
//! Copy also puts a picture (`CF_ENHMETAFILE`, a raster of the view or of
//! the focused diagram) first; elsewhere the picture goes to the system
//! clipboard as an image (our extension — the original is Windows-only).

use crate::raster::Canvas;
use std::cell::RefCell;
use std::path::PathBuf;

/// The in-memory clipboard: the text, and the picture's size.
type MemoryClip = RefCell<(Option<String>, Option<(usize, usize)>)>;

pub enum SysClip {
    Memory(MemoryClip),
    #[cfg_attr(windows, allow(dead_code))]
    File(PathBuf),
    #[cfg(windows)]
    System,
}

impl Default for SysClip {
    fn default() -> Self {
        SysClip::Memory(RefCell::new((None, None)))
    }
}

impl SysClip {
    /// The real clipboard of this platform.
    pub fn system() -> SysClip {
        #[cfg(windows)]
        {
            SysClip::System
        }
        #[cfg(not(windows))]
        {
            crate::instance::default_dir().map_or_else(SysClip::default, |d| SysClip::File(d.join("clipboard.xml")))
        }
    }

    /// Empty the clipboard, then set the picture and the text, either of
    /// which may be absent (`0x45be28`: CF_ENHMETAFILE first, then the XML).
    pub fn put(&self, text: Option<&str>, picture: Option<&Canvas>) -> bool {
        match self {
            SysClip::Memory(m) => {
                *m.borrow_mut() = (text.map(str::to_string), picture.map(|c| (c.w, c.h)));
                true
            }
            SysClip::File(p) => {
                #[cfg(not(windows))]
                if let Some(c) = picture {
                    image::put(c);
                }
                match text {
                    Some(t) => p.parent().is_some_and(|d| std::fs::create_dir_all(d).is_ok()) && std::fs::write(p, t).is_ok(),
                    None => !p.exists() || std::fs::remove_file(p).is_ok(),
                }
            }
            #[cfg(windows)]
            SysClip::System => win::put(text, picture),
        }
    }

    /// The picture's size on the in-memory clipboard (tests).
    #[cfg(test)]
    pub fn picture(&self) -> Option<(usize, usize)> {
        match self {
            SysClip::Memory(m) => m.borrow().1,
            _ => None,
        }
    }

    pub fn get(&self) -> Option<String> {
        match self {
            SysClip::Memory(m) => m.borrow().0.clone(),
            SysClip::File(p) => std::fs::read_to_string(p).ok(),
            #[cfg(windows)]
            SysClip::System => win::get(),
        }
    }

    /// `IsClipboardFormatAvailable` (Paste CMDUI `0x45bf60`).
    pub fn available(&self) -> bool {
        match self {
            SysClip::Memory(m) => m.borrow().0.is_some(),
            SysClip::File(p) => p.is_file(),
            #[cfg(windows)]
            SysClip::System => win::available(),
        }
    }
}

#[cfg(windows)]
mod win {
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
    };
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Graphics::Gdi::{CloseEnhMetaFile, CreateEnhMetaFileW, StretchDIBits, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, SRCCOPY};
    use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE};

    fn format() -> u32 {
        let name: Vec<u16> = "Mechanotronica FastView Clipboard Data".encode_utf16().chain(Some(0)).collect();
        // SAFETY: a NUL-terminated wide string.
        unsafe { RegisterClipboardFormatW(name.as_ptr()) }
    }

    pub fn available() -> bool {
        // SAFETY: no pointers involved.
        unsafe { IsClipboardFormatAvailable(format()) != 0 }
    }

    pub fn put(text: Option<&str>, picture: Option<&super::Canvas>) -> bool {
        // SAFETY: the clipboard is opened and closed here; the global block
        // is allocated with the text's size and only written while locked;
        // the metafile and the block pass to the clipboard on success.
        unsafe {
            if OpenClipboard(std::ptr::null_mut()) == 0 {
                return false;
            }
            EmptyClipboard();
            let mut ok = true;
            if let Some(c) = picture {
                let emf = metafile(c);
                ok &= !emf.is_null() && !SetClipboardData(14, emf).is_null();
            }
            if let Some(text) = text {
                let wide: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
                let h = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2);
                let mut set = false;
                if !h.is_null() {
                    let p = GlobalLock(h) as *mut u16;
                    if !p.is_null() {
                        std::ptr::copy_nonoverlapping(wide.as_ptr(), p, wide.len());
                        GlobalUnlock(h);
                        set = !SetClipboardData(format(), h).is_null();
                    }
                }
                ok &= set;
            }
            CloseClipboard();
            ok
        }
    }

    /// An enhanced metafile holding the picture as one bitmap record at
    /// (0, 0) — `CreateEnhMetaFileW(NULL, NULL, NULL, NULL)` (screen
    /// reference, frame from the content) and one blit, as the original.
    ///
    /// # Safety
    /// Calls GDI; the returned handle is owned by the caller.
    unsafe fn metafile(c: &super::Canvas) -> HANDLE {
        let dc = CreateEnhMetaFileW(std::ptr::null_mut(), std::ptr::null(), std::ptr::null(), std::ptr::null());
        if dc.is_null() {
            return std::ptr::null_mut();
        }
        let bgra: Vec<u8> = c.px.iter().flat_map(|p| [p[2], p[1], p[0], 0]).collect();
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = c.w as i32;
        info.bmiHeader.biHeight = -(c.h as i32);
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;
        StretchDIBits(dc, 0, 0, c.w as i32, c.h as i32, 0, 0, c.w as i32, c.h as i32, bgra.as_ptr().cast(), &info, DIB_RGB_COLORS, SRCCOPY);
        CloseEnhMetaFile(dc) as HANDLE
    }

    pub fn get() -> Option<String> {
        // SAFETY: the handle belongs to the clipboard; it is read within
        // its locked size and up to the first NUL.
        unsafe {
            if OpenClipboard(std::ptr::null_mut()) == 0 {
                return None;
            }
            let h = GetClipboardData(format());
            let mut out = None;
            if !h.is_null() {
                let p = GlobalLock(h) as *const u16;
                if !p.is_null() {
                    let n = GlobalSize(h) / 2;
                    let s = std::slice::from_raw_parts(p, n);
                    let end = s.iter().position(|&c| c == 0).unwrap_or(n);
                    out = Some(String::from_utf16_lossy(&s[..end]));
                    GlobalUnlock(h);
                }
            }
            CloseClipboard();
            out
        }
    }
}

/// The picture on the desktop clipboard (X11 / Wayland / macOS). The handle
/// lives for the process: X11 and Wayland clipboards only serve data while
/// their owner exists.
#[cfg(not(windows))]
mod image {
    use std::cell::RefCell;

    thread_local! {
        static CLIP: RefCell<Option<arboard::Clipboard>> = const { RefCell::new(None) };
    }

    pub fn put(c: &super::Canvas) {
        let bytes: Vec<u8> = c.px.iter().flat_map(|p| [p[0], p[1], p[2], 255]).collect();
        CLIP.with(|cell| {
            let mut slot = cell.borrow_mut();
            if slot.is_none() {
                *slot = arboard::Clipboard::new().ok();
            }
            if let Some(cb) = slot.as_mut() {
                let _ = cb.set_image(arboard::ImageData { width: c.w, height: c.h, bytes: bytes.into() });
            }
        });
    }
}
