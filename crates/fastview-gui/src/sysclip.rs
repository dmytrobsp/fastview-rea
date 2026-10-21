//! The clipboard that carries «Mechanotronica FastView Clipboard Data»
//! (ledger L9.20). On Windows it is the system clipboard under that
//! registered format (`0x404f5e`, UTF-16 text with a trailing NUL, no other
//! format); elsewhere a file in our settings folder stands in for it, so two
//! instances on one machine can still merge. Tests use the in-memory kind.

use std::cell::RefCell;
use std::path::PathBuf;

pub enum SysClip {
    Memory(RefCell<Option<String>>),
    #[cfg_attr(windows, allow(dead_code))]
    File(PathBuf),
    #[cfg(windows)]
    System,
}

impl Default for SysClip {
    fn default() -> Self {
        SysClip::Memory(RefCell::new(None))
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

    pub fn put(&self, text: &str) -> bool {
        match self {
            SysClip::Memory(m) => {
                *m.borrow_mut() = Some(text.to_string());
                true
            }
            SysClip::File(p) => p.parent().is_some_and(|d| std::fs::create_dir_all(d).is_ok()) && std::fs::write(p, text).is_ok(),
            #[cfg(windows)]
            SysClip::System => win::put(text),
        }
    }

    pub fn get(&self) -> Option<String> {
        match self {
            SysClip::Memory(m) => m.borrow().clone(),
            SysClip::File(p) => std::fs::read_to_string(p).ok(),
            #[cfg(windows)]
            SysClip::System => win::get(),
        }
    }

    /// `IsClipboardFormatAvailable` (Paste CMDUI `0x45bf60`).
    pub fn available(&self) -> bool {
        match self {
            SysClip::Memory(m) => m.borrow().is_some(),
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

    pub fn put(text: &str) -> bool {
        let wide: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        // SAFETY: the block is allocated with the text's size and only
        // written while locked; ownership passes to the clipboard on success.
        unsafe {
            if OpenClipboard(std::ptr::null_mut()) == 0 {
                return false;
            }
            EmptyClipboard();
            let h = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2);
            let mut ok = false;
            if !h.is_null() {
                let p = GlobalLock(h) as *mut u16;
                if !p.is_null() {
                    std::ptr::copy_nonoverlapping(wide.as_ptr(), p, wide.len());
                    GlobalUnlock(h);
                    ok = !SetClipboardData(format(), h).is_null();
                }
            }
            CloseClipboard();
            ok
        }
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
