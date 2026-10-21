//! Print spooling (ledger L9.22): the printers to choose from, the device a
//! page is rendered for, and handing the rendered pages over. Windows draws
//! them on a GDI printer DC (the original prints through MFC's CDC); other
//! systems send a PDF to CUPS (`lp`).

use crate::printout::{Device, PageSetup};
use crate::raster::Canvas;

/// Resolution pages are rendered at for a spooler that takes a PDF.
pub const PDF_DPI: f32 = 300.0;

/// The system's printers and its default one.
pub fn printers() -> (Vec<String>, Option<String>) {
    imp::printers()
}

/// The device to render for `printer` with the page setup's orientation.
pub fn device(printer: &str, setup: &PageSetup) -> Device {
    imp::device(printer, setup).unwrap_or_else(|| Device::sheet(setup, PDF_DPI))
}

/// Print rendered pages on `printer` as a job named `title`.
pub fn print(printer: &str, title: &str, pages: &[Canvas], dev: &Device, setup: &PageSetup) -> Result<(), String> {
    imp::print(printer, title, pages, dev, setup)
}

/// The pages as a PDF (sheet size from the device).
pub fn pdf_bytes(pages: &[Canvas], dev: &Device) -> Vec<u8> {
    let (w, h) = (dev.sheet_mm.0 / 25.4 * 72.0, dev.sheet_mm.1 / 25.4 * 72.0);
    let sheets: Vec<crate::pdf::Sheet> = pages.iter().map(|c| crate::pdf::Sheet { image: c, w_pt: w, h_pt: h }).collect();
    crate::pdf::write(&sheets)
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    use std::process::Command;

    fn lpstat(arg: &str) -> Option<String> {
        let out = Command::new("lpstat").arg(arg).env("LC_ALL", "C").output().ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    }

    pub fn printers() -> (Vec<String>, Option<String>) {
        let list = lpstat("-e").map(|s| s.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect()).unwrap_or_default();
        let default = lpstat("-d").and_then(|s| s.split_once(':').map(|(_, n)| n.trim().to_string())).filter(|n| !n.is_empty() && !n.contains(' '));
        (list, default)
    }

    pub fn device(_printer: &str, _setup: &PageSetup) -> Option<Device> {
        None
    }

    pub fn print(printer: &str, title: &str, pages: &[Canvas], dev: &Device, _setup: &PageSetup) -> Result<(), String> {
        let path = std::env::temp_dir().join(format!("fastview-print-{}.pdf", std::process::id()));
        std::fs::write(&path, pdf_bytes(pages, dev)).map_err(|e| e.to_string())?;
        let out = Command::new("lp").args(["-d", printer, "-t", title]).arg(&path).output().map_err(|e| format!("lp: {e}"))?;
        let _ = std::fs::remove_file(&path);
        if out.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
        }
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Graphics::Gdi::{
        CreateDCW, DeleteDC, GetDeviceCaps, StretchDIBits, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DEVMODEW, DIB_RGB_COLORS, DMORIENT_LANDSCAPE,
        DMORIENT_PORTRAIT, DM_IN_BUFFER, DM_ORIENTATION, DM_OUT_BUFFER, HDC, HORZRES, LOGPIXELSX, SRCCOPY, VERTRES,
    };
    use windows_sys::Win32::Graphics::Printing::{
        ClosePrinter, DocumentPropertiesW, EnumPrintersW, GetDefaultPrinterW, OpenPrinterW, PRINTER_ENUM_CONNECTIONS, PRINTER_ENUM_LOCAL, PRINTER_INFO_4W,
    };
    use windows_sys::Win32::Storage::Xps::{EndDoc, EndPage, StartDocW, StartPage, DOCINFOW};

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    /// SAFETY: `p` is a NUL-terminated wide string or null.
    unsafe fn from_wide(p: *const u16) -> String {
        if p.is_null() {
            return String::new();
        }
        let mut n = 0;
        while *p.add(n) != 0 {
            n += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(p, n))
    }

    pub fn printers() -> (Vec<String>, Option<String>) {
        let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
        let mut list = Vec::new();
        // SAFETY: the buffer is sized by the first call and holds
        // `returned` PRINTER_INFO_4W records.
        unsafe {
            let (mut needed, mut returned) = (0u32, 0u32);
            EnumPrintersW(flags, std::ptr::null(), 4, std::ptr::null_mut(), 0, &mut needed, &mut returned);
            if needed > 0 {
                let mut buf = vec![0u64; (needed as usize).div_ceil(8)];
                if EnumPrintersW(flags, std::ptr::null(), 4, buf.as_mut_ptr() as *mut u8, needed, &mut needed, &mut returned) != 0 {
                    let infos = std::slice::from_raw_parts(buf.as_ptr() as *const PRINTER_INFO_4W, returned as usize);
                    list = infos.iter().map(|i| from_wide(i.pPrinterName)).collect();
                }
            }
        }
        let mut len = 0u32;
        // SAFETY: the buffer has the length the first call reports.
        let default = unsafe {
            GetDefaultPrinterW(std::ptr::null_mut(), &mut len);
            let mut buf = vec![0u16; len.max(1) as usize];
            (GetDefaultPrinterW(buf.as_mut_ptr(), &mut len) != 0).then(|| from_wide(buf.as_ptr()))
        };
        (list, default)
    }

    /// The printer's DEVMODE with the orientation set (the original writes
    /// the stored orientation into the default DEVMODE, `0x448c00`).
    fn devmode(printer: &str, setup: &PageSetup) -> Option<Vec<u64>> {
        let name = wide(printer);
        // SAFETY: the DEVMODE buffer is sized by DocumentPropertiesW and
        // the printer handle is closed on every path.
        unsafe {
            let mut h: HANDLE = std::ptr::null_mut();
            if OpenPrinterW(name.as_ptr(), &mut h, std::ptr::null()) == 0 {
                return None;
            }
            let size = DocumentPropertiesW(std::ptr::null_mut(), h, name.as_ptr(), std::ptr::null_mut(), std::ptr::null(), 0);
            let mut buf = vec![0u64; (size.max(0) as usize).div_ceil(8).max(1)];
            let dm = buf.as_mut_ptr() as *mut DEVMODEW;
            let ok = size > 0 && DocumentPropertiesW(std::ptr::null_mut(), h, name.as_ptr(), dm, std::ptr::null(), DM_OUT_BUFFER) >= 0;
            if ok {
                (*dm).dmFields |= DM_ORIENTATION;
                (*dm).Anonymous1.Anonymous1.dmOrientation = if setup.portrait { DMORIENT_PORTRAIT } else { DMORIENT_LANDSCAPE } as i16;
                DocumentPropertiesW(std::ptr::null_mut(), h, name.as_ptr(), dm, dm, DM_IN_BUFFER | DM_OUT_BUFFER);
            }
            ClosePrinter(h);
            ok.then_some(buf)
        }
    }

    fn dc(printer: &str, setup: &PageSetup) -> Option<HDC> {
        let dm = devmode(printer, setup);
        let name = wide(printer);
        // SAFETY: the DEVMODE (if any) outlives the call.
        let hdc = unsafe { CreateDCW(std::ptr::null(), name.as_ptr(), std::ptr::null(), dm.as_ref().map_or(std::ptr::null(), |b| b.as_ptr() as *const DEVMODEW)) };
        (!hdc.is_null()).then_some(hdc)
    }

    pub fn device(printer: &str, setup: &PageSetup) -> Option<Device> {
        let hdc = dc(printer, setup)?;
        // SAFETY: a valid DC, deleted after use.
        let (w, h, dpi) = unsafe {
            let caps = (GetDeviceCaps(hdc, HORZRES as i32), GetDeviceCaps(hdc, VERTRES as i32), GetDeviceCaps(hdc, LOGPIXELSX as i32));
            DeleteDC(hdc);
            caps
        };
        (w > 0 && h > 0 && dpi > 0).then(|| Device {
            w: w as usize,
            h: h as usize,
            dpi: dpi as f32,
            sheet_mm: (w as f32 / dpi as f32 * 25.4, h as f32 / dpi as f32 * 25.4),
            offset: (0, 0),
        })
    }

    pub fn print(printer: &str, title: &str, pages: &[Canvas], _dev: &Device, setup: &PageSetup) -> Result<(), String> {
        let hdc = dc(printer, setup).ok_or("Принтер недоступен.")?;
        let name = wide(title);
        let info = DOCINFOW { cbSize: std::mem::size_of::<DOCINFOW>() as i32, lpszDocName: name.as_ptr(), lpszOutput: std::ptr::null(), lpszDatatype: std::ptr::null(), fwType: 0 };
        // SAFETY: the DIB rows are padded to 4 bytes as BITMAPINFO says;
        // the DC is deleted at the end.
        unsafe {
            if StartDocW(hdc, &info) <= 0 {
                DeleteDC(hdc);
                return Err("Не удалось начать печать.".into());
            }
            for c in pages {
                let stride = (c.w * 3).div_ceil(4) * 4;
                let mut bits = vec![0u8; stride * c.h];
                for y in 0..c.h {
                    for x in 0..c.w {
                        let [r, g, b] = c.px[y * c.w + x];
                        bits[y * stride + x * 3..y * stride + x * 3 + 3].copy_from_slice(&[b, g, r]);
                    }
                }
                let mut bmi: BITMAPINFO = std::mem::zeroed();
                bmi.bmiHeader = BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: c.w as i32,
                    biHeight: -(c.h as i32),
                    biPlanes: 1,
                    biBitCount: 24,
                    biCompression: BI_RGB,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                };
                StartPage(hdc);
                let (w, h) = (c.w as i32, c.h as i32);
                StretchDIBits(hdc, 0, 0, w, h, 0, 0, w, h, bits.as_ptr() as *const _, &bmi, DIB_RGB_COLORS, SRCCOPY);
                EndPage(hdc);
            }
            EndDoc(hdc);
            DeleteDC(hdc);
        }
        Ok(())
    }
}
