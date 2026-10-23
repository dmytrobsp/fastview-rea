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

/// A printer's paper: its form name, its portrait size in mm and the
/// printer's hardware margins on it (portrait left, top, right, bottom mm;
/// GDI's PHYSICALOFFSET / the PPD's ImageableArea).
#[derive(Debug, Clone, PartialEq)]
pub struct Paper {
    pub name: String,
    pub mm: (f32, f32),
    pub margins: [f32; 4],
}

impl Paper {
    /// A sheet with no hardware margins.
    pub fn whole(name: &str, mm: (f32, f32)) -> Paper {
        Paper { name: name.into(), mm, margins: [0.0; 4] }
    }
}

/// The paper of `printer`. The original's page setup and preview work on
/// the application's printer (`0x448a90` takes the default DEVMODE and
/// hands the result to the application's printer; the greyed «Размер»
/// shows its form).
pub fn paper(printer: &str) -> Option<Paper> {
    imp::paper(printer)
}

/// The device to render for `printer` with the page setup's orientation.
pub fn device(printer: &str, setup: &PageSetup) -> Device {
    imp::device(printer, setup).unwrap_or_else(|| Device::sheet(setup, PDF_DPI))
}

/// The print dialog's «Состояние», «Тип», «Место», «Комментарий» (the
/// common print dialog fills them from the spooler, L9.113): CUPS's state,
/// make and model, location and description; Windows' PRINTER_INFO_2.
pub fn details(printer: &str) -> [String; 4] {
    imp::details(printer)
}

/// The fields from `lpstat -p <printer>` and `lpoptions -p <printer>`.
#[cfg(any(not(windows), test))]
pub fn cups_details(lpstat: &str, lpoptions: &str) -> [String; 4] {
    let state = if lpstat.contains("disabled") {
        "Приостановлен"
    } else if lpstat.contains("now printing") {
        "Печать"
    } else if lpstat.contains("idle") {
        "Готов"
    } else {
        ""
    };
    let opt = |key: &str| cups_option(lpoptions, key).unwrap_or_default();
    [state.into(), opt("printer-make-and-model"), opt("printer-location"), opt("printer-info")]
}

/// One `key=value` of an `lpoptions` line (values may be quoted with ' or ").
#[cfg(any(not(windows), test))]
fn cups_option(line: &str, key: &str) -> Option<String> {
    let mut rest = line;
    while let Some(i) = rest.find(&format!("{key}=")) {
        if i == 0 || rest.as_bytes()[i - 1] == b' ' {
            let v = &rest[i + key.len() + 1..];
            return Some(match v.chars().next() {
                Some(q @ ('\'' | '"')) => v[1..].split(q).next().unwrap_or("").to_string(),
                _ => v.split(' ').next().unwrap_or("").to_string(),
            });
        }
        rest = &rest[i + key.len()..];
    }
    None
}

/// Print rendered pages on `printer` as a job named `title`.
pub fn print(printer: &str, title: &str, pages: &[Canvas], dev: &Device, setup: &PageSetup) -> Result<(), String> {
    imp::print(printer, title, pages, dev, setup)
}

/// The paper of a CUPS printer: the starred choice of the `PageSize` line of
/// `lpoptions -l` («PageSize/Media Size: Letter *A4 …»), in mm portrait.
/// The original lays pages out on the printer DC's own paper (GDI caps,
/// L9.22); unknown names give `None` (A4).
#[cfg(any(not(windows), test))]
pub fn cups_paper(lpoptions: &str) -> Option<Paper> {
    let line = lpoptions.lines().find(|l| l.starts_with("PageSize/") || l.starts_with("PageSize:"))?;
    let choice = line.split_once(':')?.1.split_whitespace().find_map(|c| c.strip_prefix('*'))?;
    let name = choice.split('.').next().unwrap_or(choice);
    let mm = match name.to_ascii_lowercase().as_str() {
        "a3" => (297.0, 420.0),
        "a4" => (210.0, 297.0),
        "a5" => (148.0, 210.0),
        "b5" => (176.0, 250.0),
        "letter" => (215.9, 279.4),
        "legal" => (215.9, 355.6),
        _ => return None,
    };
    Some(Paper::whole(name, mm))
}

/// The starred `PageSize` choice of `lpoptions -l` as the PPD names it.
#[cfg(any(not(windows), test))]
pub fn cups_page_choice(lpoptions: &str) -> Option<&str> {
    let line = lpoptions.lines().find(|l| l.starts_with("PageSize/") || l.starts_with("PageSize:"))?;
    line.split_once(':')?.1.split_whitespace().find_map(|c| c.strip_prefix('*'))
}

/// The printer's hardware margins in mm (portrait l, t, r, b) for `page`
/// of `paper_mm` from a PPD's `*ImageableArea <page>/…: "llx lly urx ury"`
/// (points from the sheet's lower left) — the CUPS side of GDI's
/// printable area (L9.110).
#[cfg(any(not(windows), test))]
pub fn ppd_margins(ppd: &str, page: &str, paper_mm: (f32, f32)) -> Option<[f32; 4]> {
    let line = ppd.lines().find(|l| l.strip_prefix("*ImageableArea ").is_some_and(|r| r.split(['/', ':']).next() == Some(page)))?;
    let quoted = line.split('"').nth(1)?;
    let n: Vec<f32> = quoted.split_whitespace().map(str::parse).collect::<Result<_, _>>().ok()?;
    let [llx, lly, urx, ury] = n[..] else { return None };
    let mm = |pt: f32| (pt / 72.0 * 25.4).max(0.0);
    let (w, h) = (paper_mm.0 / 25.4 * 72.0, paper_mm.1 / 25.4 * 72.0);
    (urx > llx && ury > lly).then(|| [mm(llx), mm(h - ury), mm(w - urx), mm(lly)])
}

/// The pages as a PDF: sheets of the device's paper, each page on its
/// printable area.
pub fn pdf_bytes(pages: &[Canvas], dev: &Device) -> Vec<u8> {
    let (w, h) = (dev.sheet_mm.0 / 25.4 * 72.0, dev.sheet_mm.1 / 25.4 * 72.0);
    let pt = |px: usize| px as f32 / dev.dpi * 72.0;
    let at = [pt(dev.offset.0), pt(dev.offset.1), pt(dev.w), pt(dev.h)];
    let sheets: Vec<crate::pdf::Sheet> = pages.iter().map(|c| crate::pdf::Sheet { image: c, w_pt: w, h_pt: h, at }).collect();
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

    pub fn details(printer: &str) -> [String; 4] {
        let run = |cmd: &str, args: &[&str]| Command::new(cmd).args(args).env("LC_ALL", "C").output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
        cups_details(&run("lpstat", &["-p", printer]), &run("lpoptions", &["-p", printer]))
    }

    /// The printer's default paper from CUPS, with the PPD's printable
    /// area (the scheduler's copy, else its public `/printers/<name>.ppd`).
    pub fn paper(printer: &str) -> Option<Paper> {
        let out = Command::new("lpoptions").args(["-p", printer, "-l"]).env("LC_ALL", "C").output().ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        let mut p = cups_paper(&text)?;
        let ppd = std::fs::read_to_string(format!("/etc/cups/ppd/{printer}.ppd")).ok().or_else(|| fetch_ppd(printer));
        if let (Some(ppd), Some(page)) = (ppd, cups_page_choice(&text)) {
            p.margins = ppd_margins(&ppd, page, p.mm).unwrap_or_default();
        }
        Some(p)
    }

    /// `GET /printers/<name>.ppd` from the local scheduler (port 631).
    fn fetch_ppd(printer: &str) -> Option<String> {
        use std::io::{Read, Write};
        let ok = |c: char| c.is_ascii_alphanumeric() || "-_.".contains(c);
        if !printer.chars().all(ok) {
            return None;
        }
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 631));
        let mut s = std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(300)).ok()?;
        s.set_read_timeout(Some(std::time::Duration::from_secs(2))).ok()?;
        s.write_all(format!("GET /printers/{printer}.ppd HTTP/1.0\r\nHost: localhost\r\n\r\n").as_bytes()).ok()?;
        let mut body = Vec::new();
        s.read_to_end(&mut body).ok()?;
        let text = String::from_utf8_lossy(&body).into_owned();
        let (head, ppd) = text.split_once("\r\n\r\n")?;
        head.lines().next()?.contains(" 200 ").then(|| ppd.to_string())
    }

    /// The printer's default paper, oriented by the page setup; the job
    /// laid out on its printable area.
    pub fn device(printer: &str, setup: &PageSetup) -> Option<Device> {
        let p = paper(printer)?;
        Some(Device::printable(setup.oriented(p.mm), setup.oriented_margins(p.margins), PDF_DPI))
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
        DMORIENT_PORTRAIT, DM_IN_BUFFER, DM_ORIENTATION, DM_OUT_BUFFER, HDC, HORZRES, LOGPIXELSX, LOGPIXELSY, PHYSICALHEIGHT, PHYSICALOFFSETX, PHYSICALOFFSETY, PHYSICALWIDTH, SRCCOPY,
        VERTRES,
    };
    use windows_sys::Win32::Graphics::Printing::{
        ClosePrinter, DocumentPropertiesW, EnumPrintersW, GetDefaultPrinterW, GetPrinterW, OpenPrinterW, PRINTER_ENUM_CONNECTIONS, PRINTER_ENUM_LOCAL, PRINTER_INFO_2W,
        PRINTER_INFO_4W,
    };
    use windows_sys::Win32::Storage::Xps::{EndDoc, EndPage, StartDocW, StartPage, DOCINFOW};

    /// PRINTER_INFO_2: the status (0 «Готов»; paused, error — INFERRED
    /// wording), the driver, the port, the comment.
    pub fn details(printer: &str) -> [String; 4] {
        let name = wide(printer);
        let mut h: HANDLE = std::ptr::null_mut();
        // SAFETY: OpenPrinterW / GetPrinterW with a sized buffer, closed after.
        unsafe {
            if OpenPrinterW(name.as_ptr(), &mut h, std::ptr::null()) == 0 {
                return Default::default();
            }
            let mut need = 0u32;
            GetPrinterW(h, 2, std::ptr::null_mut(), 0, &mut need);
            let mut buf = vec![0u64; (need as usize).div_ceil(8).max(1)];
            let ok = GetPrinterW(h, 2, buf.as_mut_ptr() as *mut u8, need, &mut need) != 0;
            ClosePrinter(h);
            if !ok {
                return Default::default();
            }
            let i = &*(buf.as_ptr() as *const PRINTER_INFO_2W);
            let status = match i.Status {
                0 => "Готов",
                s if s & 1 != 0 => "Приостановлен",
                s if s & 2 != 0 => "Ошибка",
                _ => "",
            };
            [status.into(), from_wide(i.pDriverName), from_wide(i.pPortName), from_wide(i.pComment)]
        }
    }

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

    /// The DEVMODE's form name and the DC's physical page, portrait.
    pub fn paper(printer: &str) -> Option<Paper> {
        let portrait = PageSetup { portrait: true, ..PageSetup::default() };
        let dm = devmode(printer, &portrait)?;
        // SAFETY: the buffer holds the DEVMODEW DocumentPropertiesW wrote.
        let form = unsafe { (*(dm.as_ptr() as *const DEVMODEW)).dmFormName };
        let name = String::from_utf16_lossy(&form[..form.iter().position(|&c| c == 0).unwrap_or(form.len())]);
        let hdc = dc(printer, &portrait)?;
        // SAFETY: a valid DC, deleted after use.
        let (w, h, dx, dy, ox, oy, hr, vr) = unsafe {
            let caps = (
                GetDeviceCaps(hdc, PHYSICALWIDTH as i32),
                GetDeviceCaps(hdc, PHYSICALHEIGHT as i32),
                GetDeviceCaps(hdc, LOGPIXELSX as i32),
                GetDeviceCaps(hdc, LOGPIXELSY as i32),
                GetDeviceCaps(hdc, PHYSICALOFFSETX as i32),
                GetDeviceCaps(hdc, PHYSICALOFFSETY as i32),
                GetDeviceCaps(hdc, HORZRES as i32),
                GetDeviceCaps(hdc, VERTRES as i32),
            );
            DeleteDC(hdc);
            caps
        };
        let (mx, my) = (|v: i32| v as f32 / dx as f32 * 25.4, |v: i32| v as f32 / dy as f32 * 25.4);
        let margins = [mx(ox), my(oy), mx(w - ox - hr).max(0.0), my(h - oy - vr).max(0.0)];
        (w > 0 && h > 0 && dx > 0 && dy > 0).then(|| Paper { name, mm: (mx(w), my(h)), margins })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cups_paper_from_lpoptions() {
        let out = "PageSize/Media Size: Letter Legal *A4 A5 Env10\nDuplex/2-Sided Printing: *None DuplexNoTumble\n";
        assert_eq!(cups_paper(out), Some(Paper::whole("A4", (210.0, 297.0))));
        assert_eq!(cups_paper("PageSize/Media Size: *Letter A4"), Some(Paper::whole("Letter", (215.9, 279.4))));
        assert_eq!(cups_paper("PageSize/Media Size: *A5.Fullbleed"), Some(Paper::whole("A5", (148.0, 210.0))));
        assert_eq!(cups_paper("PageSize/Media Size: *Custom.100x200mm"), None);
        assert_eq!(cups_paper("Duplex/2-Sided: *None"), None);
    }

    /// The PPD's printable area as hardware margins; the PDF puts each
    /// page on it.
    #[test]
    fn cups_margins_from_the_ppd() {
        let ppd = "*PageSize A4/A4: \"<</PageSize[595 842]>>setpagedevice\"\n*ImageableArea Letter/US Letter: \"18 36 594 756\"\n*ImageableArea A4/A4: \"18 36 577 806\"\n";
        assert_eq!(cups_page_choice("PageSize/Media Size: Letter *A4"), Some("A4"));
        let m = ppd_margins(ppd, "A4", (210.0, 297.0)).unwrap();
        let near = |a: f32, b: f32| (a - b).abs() < 0.05;
        // A4 is 595.28 × 841.89 pt: 18 pt left, 35.89 top, 18.28 right, 36 bottom.
        assert!(near(m[0], 6.35) && near(m[1], 12.66) && near(m[2], 6.45) && near(m[3], 12.7), "{m:?}");
        assert_eq!(ppd_margins(ppd, "A5", (148.0, 210.0)), None);
        let setup = PageSetup::default();
        let dev = Device::printable(setup.oriented((210.0, 297.0)), setup.oriented_margins(m), 72.0);
        assert_eq!((dev.offset, dev.w, dev.h), ((18, 36), 559, 770));
        let pdf = String::from_utf8_lossy(&pdf_bytes(&[Canvas::white(dev.w, dev.h)], &dev)).into_owned();
        assert!(pdf.contains("q 559.00 0 0 770.00 18.00 35.89 cm /Im0 Do Q"), "the page on the printable area");
    }

    #[test]
    fn cups_printer_details() {
        let st = "printer Office_HP is idle.  enabled since Thu 23 Oct 2026\n";
        let op = "copies=1 device-uri=ipp://10.0.0.5/ipp printer-info='HP LaserJet 4' printer-location=\"Room 12\" printer-make-and-model='HP LaserJet 4 Postscript' printer-state=3";
        assert_eq!(cups_details(st, op), ["Готов".to_string(), "HP LaserJet 4 Postscript".into(), "Room 12".into(), "HP LaserJet 4".into()]);
        assert_eq!(cups_details("printer x disabled since …", "")[0], "Приостановлен");
    }

    /// The system printers' papers (run by hand: `--ignored`).
    #[test]
    #[ignore]
    fn system_printers_paper() {
        let (list, default) = printers();
        eprintln!("default: {default:?}");
        for p in list {
            eprintln!("{p}: {:?} {:?}", paper(&p), details(&p));
        }
    }
}
