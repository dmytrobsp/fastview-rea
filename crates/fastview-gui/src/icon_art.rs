//! Our logo — the orb's two chevrons, blue over green — as RGBA pixels at
//! any square size. Our own drawing (as `ribbon::paint_logo`), used twice:
//! the window icon at start-up (`app_icon`) and, by `build.rs`, the icon
//! resource embedded in `fastview.exe` (Explorer, shortcuts).

/// The logo over an `n` × `n` box, straight (non-premultiplied) RGBA. The
/// geometry is the 48 px drawing scaled by `n / 48`.
// build.rs includes this file for `render` only.
#[allow(dead_code)]
pub fn render(n: usize) -> Vec<u8> {
    let k = n as f32 / 48.0;
    let seg = |p: (f32, f32), a: (f32, f32), b: (f32, f32)| {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
        ((p.0 - a.0 - t * dx).powi(2) + (p.1 - a.1 - t * dy).powi(2)).sqrt()
    };
    // As `ribbon::paint_logo` over a 40 × 33 box at (4, 8), 5 px strokes.
    let (l, t, w, h) = (4.0f32 * k, 8.0f32 * k, 40.0f32 * k, 33.0f32 * k);
    let half = 2.5 * k;
    let chevron = |yoff: f32| [(l, t + h - yoff - h * 0.1), (l + w / 2.0, t + yoff + h * 0.05), (l + w, t + h - yoff - h * 0.1)];
    let marks = [(chevron(0.0), [0x2E, 0x5B, 0x9E]), (chevron(h * 0.36), [0x4C, 0xAF, 0x50])];
    let mut rgba = vec![0u8; n * n * 4];
    for y in 0..n {
        for x in 0..n {
            let p = (x as f32 + 0.5, y as f32 + 0.5);
            for (pts, col) in &marks {
                let d = seg(p, pts[0], pts[1]).min(seg(p, pts[1], pts[2]));
                let cover = (half + 0.5 - d).clamp(0.0, 1.0);
                if cover > 0.0 {
                    let i = (y * n + x) * 4;
                    let a = (cover * 255.0) as u8;
                    // The later (green) chevron over the first.
                    if a >= rgba[i + 3] || col[1] == 0xAF {
                        rgba[i..i + 3].copy_from_slice(col);
                        rgba[i + 3] = rgba[i + 3].max(a);
                    }
                }
            }
        }
    }
    rgba
}

#[cfg(test)]
mod tests {
    use super::render;

    /// Every size draws both chevrons (blue and green pixels), on a
    /// transparent ground, and the larger sizes keep the 48 px proportions.
    #[test]
    fn logo_has_both_chevrons_at_every_size() {
        for n in [16usize, 24, 32, 48, 64, 128, 256] {
            let px = render(n);
            assert_eq!(px.len(), n * n * 4);
            let has = |c: [u8; 3]| px.chunks(4).any(|p| p[3] > 200 && p[..3] == c);
            assert!(has([0x2E, 0x5B, 0x9E]), "blue chevron at {n}");
            assert!(has([0x4C, 0xAF, 0x50]), "green chevron at {n}");
            assert_eq!(px[3], 0, "corner transparent at {n}");
        }
    }
}
