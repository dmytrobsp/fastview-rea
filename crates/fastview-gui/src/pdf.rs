//! A minimal PDF writer for printing (ledger L9.22): each page is one RGB
//! image covering the sheet, Flate-compressed. It is what we hand to the
//! system's print spooler (CUPS `lp`) and what «Сохранить в PDF» writes.

use crate::raster::Canvas;
use flate2::write::ZlibEncoder;
use std::io::Write;

/// One sheet: its image, its size in PDF points (1/72 inch) and where the
/// image goes on it (left, top, width, height in points).
pub struct Sheet<'a> {
    pub image: &'a Canvas,
    pub w_pt: f32,
    pub h_pt: f32,
    pub at: [f32; 4],
}

pub fn write(sheets: &[Sheet]) -> Vec<u8> {
    let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets: Vec<usize> = Vec::new();
    let obj = |out: &mut Vec<u8>, offsets: &mut Vec<usize>, body: &[u8]| {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", offsets.len()).as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    };
    // 1 catalog, 2 pages, then per sheet: page, content, image.
    let kids: Vec<String> = (0..sheets.len()).map(|i| format!("{} 0 R", 3 + 3 * i)).collect();
    obj(&mut out, &mut offsets, b"<< /Type /Catalog /Pages 2 0 R >>");
    obj(&mut out, &mut offsets, format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), sheets.len()).as_bytes());
    for (i, s) in sheets.iter().enumerate() {
        let (page, content, image) = (3 + 3 * i, 4 + 3 * i, 5 + 3 * i);
        let _ = page;
        obj(
            &mut out,
            &mut offsets,
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w:.2} {h:.2}] /Resources << /XObject << /Im0 {image} 0 R >> >> /Contents {content} 0 R >>",
                w = s.w_pt,
                h = s.h_pt
            )
            .as_bytes(),
        );
        let [x, y, w, h] = s.at;
        let ops = format!("q {w:.2} 0 0 {h:.2} {x:.2} {:.2} cm /Im0 Do Q", s.h_pt - y - h);
        obj(&mut out, &mut offsets, format!("<< /Length {} >>\nstream\n{ops}\nendstream", ops.len()).as_bytes());
        let mut z = ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        for p in &s.image.px {
            let _ = z.write_all(p);
        }
        let data = z.finish().unwrap_or_default();
        let mut body = format!(
            "<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode /Length {} >>\nstream\n",
            s.image.w,
            s.image.h,
            data.len()
        )
        .into_bytes();
        body.extend_from_slice(&data);
        body.extend_from_slice(b"\nendstream");
        obj(&mut out, &mut offsets, &body);
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1).as_bytes());
    for o in &offsets {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", offsets.len() + 1).as_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_pages_with_a_valid_xref() {
        let img = Canvas::white(20, 30);
        let pdf = write(&[Sheet { image: &img, w_pt: 595.28, h_pt: 841.89, at: [0.0, 0.0, 595.28, 841.89] }, Sheet { image: &img, w_pt: 841.89, h_pt: 595.28, at: [0.0, 0.0, 841.89, 595.28] }]);
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.starts_with("%PDF-1.4"));
        assert!(text.contains("/Count 2") && text.contains("/MediaBox [0 0 841.89 595.28]"));
        // Every xref offset points at "<n> 0 obj" (checked on the bytes).
        let tail = &text[text.rfind("startxref\n").unwrap() + 10..];
        let xref_at: usize = tail.lines().next().unwrap().parse().unwrap();
        let table = String::from_utf8_lossy(&pdf[xref_at..]).to_string();
        for (n, line) in table.lines().skip(3).take(8).enumerate() {
            let off: usize = line[..10].parse().unwrap();
            assert!(pdf[off..].starts_with(format!("{} 0 obj", n + 1).as_bytes()), "object {}", n + 1);
        }
    }
}
