//! Wine's Tahoma Bold (`assets/fonts/tahomabd.ttf`) carries the regular
//! face's outlines for the Cyrillic letters with no Latin look-alike (и, Б,
//! Л …: glyph records byte for byte those of `tahoma.ttf`, in the bold
//! advances), so a bold Cyrillic word came out in two weights («Н**а**и
//! м**е**н**о**вание»). At load those outlines are thickened as a bold
//! stroke — every contour offset outward by the two faces' stem difference
//! (FreeType's embolden: each point along the miter of its edges' normals)
//! — and centred in their advance; the glyph table is rebuilt with long
//! offsets. Everything else is kept as it is.

fn rd16(b: &[u8], o: usize) -> u16 {
    u16::from_be_bytes([b[o], b[o + 1]])
}

fn rd32(b: &[u8], o: usize) -> u32 {
    u32::from_be_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

/// A table's offset and length.
fn table(b: &[u8], tag: &[u8; 4]) -> Option<(usize, usize)> {
    (0..rd16(b, 4) as usize).map(|i| 12 + 16 * i).find(|&r| &b[r..r + 4] == tag).map(|r| (rd32(b, r + 8) as usize, rd32(b, r + 12) as usize))
}

/// The glyph of `ch` by the (3, 1) format 4 cmap; 0 when none.
fn glyph_id(b: &[u8], ch: u32) -> u16 {
    let Some((cmap, _)) = table(b, b"cmap") else { return 0 };
    let sub = (0..rd16(b, cmap + 2) as usize).map(|i| cmap + 4 + 8 * i).find(|&r| rd16(b, r) == 3 && rd16(b, r + 2) == 1).map(|r| cmap + rd32(b, r + 4) as usize);
    let Some(p) = sub.filter(|&p| rd16(b, p) == 4) else { return 0 };
    let seg = rd16(b, p + 6) as usize / 2;
    let (ends, starts, deltas, ranges) = (p + 14, p + 16 + 2 * seg, p + 16 + 4 * seg, p + 16 + 6 * seg);
    for k in 0..seg {
        let (s, e) = (u32::from(rd16(b, starts + 2 * k)), u32::from(rd16(b, ends + 2 * k)));
        if (s..=e).contains(&ch) {
            let delta = rd16(b, deltas + 2 * k);
            let ro = rd16(b, ranges + 2 * k) as usize;
            if ro == 0 {
                return (ch as u16).wrapping_add(delta);
            }
            let g = rd16(b, ranges + 2 * k + ro + 2 * (ch - s) as usize);
            return if g == 0 { 0 } else { g.wrapping_add(delta) };
        }
    }
    0
}

/// Every glyph's byte range in `glyf`.
fn glyph_ranges(b: &[u8]) -> Option<Vec<(usize, usize)>> {
    let (head, _) = table(b, b"head")?;
    let (maxp, _) = table(b, b"maxp")?;
    let (loca, _) = table(b, b"loca")?;
    let long = rd16(b, head + 50) == 1;
    let n = rd16(b, maxp + 4) as usize;
    let at = |g: usize| if long { rd32(b, loca + 4 * g) as usize } else { 2 * rd16(b, loca + 2 * g) as usize };
    Some((0..n).map(|g| (at(g), at(g + 1))).collect())
}

fn advance(b: &[u8], g: u16) -> Option<f32> {
    let (hhea, _) = table(b, b"hhea")?;
    let (hmtx, _) = table(b, b"hmtx")?;
    let g = usize::from(g).min(rd16(b, hhea + 34) as usize - 1);
    Some(f32::from(rd16(b, hmtx + 4 * g)))
}

type Contour = Vec<(f32, f32, bool)>;

/// A simple glyph's contours (points absolute, on-curve flags).
fn decode(g: &[u8]) -> Option<Vec<Contour>> {
    let nc = i16::from_be_bytes([g[0], g[1]]);
    if nc <= 0 {
        return None;
    }
    let nc = nc as usize;
    let ends: Vec<usize> = (0..nc).map(|i| rd16(g, 10 + 2 * i) as usize).collect();
    let n = ends.last()? + 1;
    let mut p = 10 + 2 * nc;
    p += 2 + rd16(g, p) as usize;
    let mut flags = Vec::with_capacity(n);
    while flags.len() < n {
        let f = g[p];
        p += 1;
        flags.push(f);
        if f & 8 != 0 {
            let r = g[p];
            p += 1;
            flags.extend(std::iter::repeat_n(f, r as usize));
        }
    }
    flags.truncate(n);
    let mut coords = |short: u8, same: u8| {
        let mut v = 0i32;
        let mut out = Vec::with_capacity(n);
        for &f in &flags {
            if f & short != 0 {
                let d = i32::from(g[p]);
                p += 1;
                v += if f & same != 0 { d } else { -d };
            } else if f & same == 0 {
                v += i32::from(i16::from_be_bytes([g[p], g[p + 1]]));
                p += 2;
            }
            out.push(v as f32);
        }
        out
    };
    let xs = coords(2, 16);
    let ys = coords(4, 32);
    let mut start = 0;
    Some(
        ends.iter()
            .map(|&e| {
                let c = (start..=e).map(|i| (xs[i], ys[i], flags[i] & 1 != 0)).collect();
                start = e + 1;
                c
            })
            .collect(),
    )
}

/// The contours as a simple glyph (no instructions; 16-bit coordinates).
fn encode(contours: &[Contour]) -> Vec<u8> {
    let pts: Vec<(i16, i16, bool)> = contours.iter().flatten().map(|&(x, y, on)| (x.round() as i16, y.round() as i16, on)).collect();
    let (x0, x1) = pts.iter().fold((i16::MAX, i16::MIN), |(a, b), p| (a.min(p.0), b.max(p.0)));
    let (y0, y1) = pts.iter().fold((i16::MAX, i16::MIN), |(a, b), p| (a.min(p.1), b.max(p.1)));
    let mut out = Vec::new();
    for v in [contours.len() as i16, x0, y0, x1, y1] {
        out.extend(v.to_be_bytes());
    }
    let mut end = 0usize;
    for c in contours {
        end += c.len();
        out.extend(((end - 1) as u16).to_be_bytes());
    }
    out.extend(0u16.to_be_bytes());
    out.extend(pts.iter().map(|p| u8::from(p.2)));
    let (mut px, mut py) = (0i16, 0i16);
    for p in &pts {
        out.extend(p.0.wrapping_sub(px).to_be_bytes());
        px = p.0;
    }
    for p in &pts {
        out.extend(p.1.wrapping_sub(py).to_be_bytes());
        py = p.1;
    }
    out
}

/// A contour's signed area (positive: counter-clockwise, y up).
fn area(c: &Contour) -> f32 {
    let n = c.len();
    (0..n).map(|i| c[i].0 * c[(i + 1) % n].1 - c[(i + 1) % n].0 * c[i].1).sum::<f32>() / 2.0
}

/// Each contour offset outward by (dx, dy), then moved by `shift`. Outer
/// contours run clockwise in TrueType (outward is left of the travel); a
/// glyph drawn the other way round (its largest contour counter-clockwise
/// — some of Wine's) has every normal turned.
fn embolden(contours: &[Contour], (dx, dy): (f32, f32), shift: (f32, f32)) -> Vec<Contour> {
    let outer = contours.iter().map(area).fold(0.0f32, |m, a| if a.abs() > m.abs() { a } else { m });
    let (dx, dy) = if outer > 0.0 { (-dx, -dy) } else { (dx, dy) };
    let unit = |x: f32, y: f32| {
        let l = (x * x + y * y).sqrt();
        if l > 0.0 { (x / l, y / l) } else { (0.0, 0.0) }
    };
    contours
        .iter()
        .map(|c| {
            let n = c.len();
            (0..n)
                .map(|i| {
                    let (p, a, b) = (c[i], c[(i + n - 1) % n], c[(i + 1) % n]);
                    let din = unit(p.0 - a.0, p.1 - a.1);
                    let dout = unit(b.0 - p.0, b.1 - p.1);
                    let (n1, n2) = ((-din.1, din.0), (-dout.1, dout.0));
                    let (sx, sy) = (n1.0 + n2.0, n1.1 + n2.1);
                    let d = 1.0 + n1.0 * n2.0 + n1.1 * n2.1;
                    // The miter, limited at sharp turns.
                    let (mx, my) = if d > 0.25 { (sx / d, sy / d) } else { unit(sx, sy) };
                    (p.0 + mx * dx + shift.0, p.1 + my * dy + shift.1, p.2)
                })
                .collect()
        })
        .collect()
}

/// `bold` with its copied regular Cyrillic outlines made bold (see the
/// module); `bold` unchanged when anything is unexpected.
pub fn embolden_copied_cyrillic(bold: &[u8], regular: &[u8]) -> Vec<u8> {
    patch(bold, regular).unwrap_or_else(|| bold.to_vec())
}

fn patch(bold: &[u8], regular: &[u8]) -> Option<Vec<u8>> {
    let (glyf_b, _) = table(bold, b"glyf")?;
    let (glyf_r, _) = table(regular, b"glyf")?;
    let (rb, rr) = (glyph_ranges(bold)?, glyph_ranges(regular)?);
    // The stem difference of the two faces' «l», per side; the horizontal
    // strokes take less (Tahoma Bold's are thinner than its stems).
    let stem = |b: &[u8], r: &[(usize, usize)], glyf: usize| {
        let (s, e) = r[usize::from(glyph_id(b, u32::from('l')))];
        (e > s).then(|| f32::from(i16::from_be_bytes([b[glyf + s + 6], b[glyf + s + 7]])) - f32::from(i16::from_be_bytes([b[glyf + s + 2], b[glyf + s + 3]])))
    };
    let dx = ((stem(bold, &rb, glyf_b)? - stem(regular, &rr, glyf_r)?) / 2.0).clamp(0.0, 200.0);
    let dy = 0.35 * dx;
    let mut new: Vec<Option<Vec<u8>>> = vec![None; rb.len()];
    for ch in 0x400..0x500u32 {
        let (gb, gr) = (glyph_id(bold, ch), glyph_id(regular, ch));
        if gb == 0 || gr == 0 || new[usize::from(gb)].is_some() {
            continue;
        }
        let ((s, e), (s2, e2)) = (rb[usize::from(gb)], rr[usize::from(gr)]);
        let (gbytes, rbytes) = (&bold[glyf_b + s..glyf_b + e], &regular[glyf_r + s2..glyf_r + e2]);
        if e <= s || gbytes != rbytes {
            continue;
        }
        let Some(c) = decode(gbytes) else { continue };
        let centre = (advance(bold, gb)? - advance(regular, gr)?) / 2.0;
        new[usize::from(gb)] = Some(encode(&embolden(&c, (dx, dy), (centre, dy))));
    }
    if new.iter().all(Option::is_none) {
        return None;
    }
    // The glyph table again, long offsets.
    let mut glyf = Vec::new();
    let mut loca = Vec::with_capacity(4 * (rb.len() + 1));
    for (g, &(s, e)) in rb.iter().enumerate() {
        loca.extend((glyf.len() as u32).to_be_bytes());
        match &new[g] {
            Some(bytes) => glyf.extend(bytes),
            None => glyf.extend(&bold[glyf_b + s..glyf_b + e.max(s)]),
        }
        while !glyf.len().is_multiple_of(4) {
            glyf.push(0);
        }
    }
    loca.extend((glyf.len() as u32).to_be_bytes());
    let (head, head_len) = table(bold, b"head")?;
    let mut head_t = bold[head..head + head_len].to_vec();
    head_t[50..52].copy_from_slice(&1u16.to_be_bytes());
    // The file again: the same tables in the same order.
    let n = rd16(bold, 4) as usize;
    let mut out = bold[..12 + 16 * n].to_vec();
    for i in 0..n {
        let r = 12 + 16 * i;
        let tag: [u8; 4] = bold[r..r + 4].try_into().ok()?;
        let data: Vec<u8> = match &tag {
            b"glyf" => glyf.clone(),
            b"loca" => loca.clone(),
            b"head" => head_t.clone(),
            _ => {
                let (o, l) = (rd32(bold, r + 8) as usize, rd32(bold, r + 12) as usize);
                bold[o..o + l].to_vec()
            }
        };
        let at = out.len() as u32;
        out[r + 8..r + 12].copy_from_slice(&at.to_be_bytes());
        out[r + 12..r + 16].copy_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend(&data);
        while !out.len().is_multiple_of(4) {
            out.push(0);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const REG: &[u8] = include_bytes!("../../../assets/fonts/tahoma.ttf");
    const BOLD: &[u8] = include_bytes!("../../../assets/fonts/tahomabd.ttf");

    fn bbox(b: &[u8], ch: char) -> (i16, i16) {
        let (glyf, _) = table(b, b"glyf").unwrap();
        let (s, _) = glyph_ranges(b).unwrap()[usize::from(glyph_id(b, u32::from(ch)))];
        let g = &b[glyf + s..];
        (i16::from_be_bytes([g[2], g[3]]), i16::from_be_bytes([g[6], g[7]]))
    }

    /// «и» copied from the regular face gets the bold stems; «Н» (a real
    /// bold composite) and the Latin letters are untouched.
    #[test]
    fn the_copied_cyrillic_outlines_get_bold() {
        let out = embolden_copied_cyrillic(BOLD, REG);
        let (x0, x1) = bbox(BOLD, 'и');
        let (y0, y1) = bbox(&out, 'и');
        assert!((y1 - y0) - (x1 - x0) > 150, "wider by about two stem steps: {:?} → {:?}", (x0, x1), (y0, y1));
        for ch in ['l', 'H', 'a', 'Н'] {
            assert_eq!(bbox(&out, ch), bbox(BOLD, ch), "{ch}");
        }
        // Every glyph decodes the same as before but the patched ones.
        assert_eq!(glyph_ranges(&out).unwrap().len(), glyph_ranges(BOLD).unwrap().len());
        assert_eq!(advance(&out, glyph_id(&out, u32::from('и'))), advance(BOLD, glyph_id(BOLD, u32::from('и'))));
    }
}
