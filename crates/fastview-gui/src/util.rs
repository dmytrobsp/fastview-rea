//! Dependency-free encoders: CRC-32, a minimal (stored) PNG writer for image
//! export, and a minimal (stored) ZIP writer for archiving records.

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in bytes {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

fn adler32(bytes: &[u8]) -> u32 {
    let (mut a, mut b): (u32, u32) = (1, 0);
    for &byte in bytes {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

/// Encode RGBA8 pixels as a PNG using uncompressed (stored) DEFLATE blocks.
pub fn png_rgba(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    // Raw image data with a filter byte (0) per scanline.
    let mut raw = Vec::with_capacity((w * h * 4 + h) as usize);
    for y in 0..h {
        raw.push(0u8); // filter: none
        let row = (y * w * 4) as usize;
        raw.extend_from_slice(&rgba[row..row + (w * 4) as usize]);
    }
    // zlib stream: header + stored DEFLATE blocks + Adler32.
    let mut zlib = vec![0x78, 0x01];
    let mut off = 0usize;
    while off < raw.len() {
        let chunk = (raw.len() - off).min(0xFFFF);
        let last = off + chunk >= raw.len();
        zlib.push(if last { 1 } else { 0 });
        zlib.extend_from_slice(&(chunk as u16).to_le_bytes());
        zlib.extend_from_slice(&(!(chunk as u16)).to_le_bytes());
        zlib.extend_from_slice(&raw[off..off + chunk]);
        off += chunk;
    }
    zlib.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let chunk = |out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]| {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        let mut crc_in = Vec::with_capacity(4 + data.len());
        crc_in.extend_from_slice(kind);
        crc_in.extend_from_slice(data);
        out.extend_from_slice(&crc32(&crc_in).to_be_bytes());
    };
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit, RGBA, deflate, no filter, no interlace
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &zlib);
    chunk(&mut out, b"IEND", &[]);
    out
}

/// Build a ZIP archive with the given `(name, bytes)` entries, stored (no
/// compression). Good enough to bundle a COMTRADE record for sending.
pub fn zip_store(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    let mut offsets: Vec<u32> = Vec::new();
    for (name, data) in entries {
        offsets.push(out.len() as u32);
        let crc = crc32(data);
        let nb = name.as_bytes();
        // local file header
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes()); // version
        out.extend_from_slice(&0u16.to_le_bytes()); // flags
        out.extend_from_slice(&0u16.to_le_bytes()); // method: store
        out.extend_from_slice(&0u16.to_le_bytes()); // time
        out.extend_from_slice(&0u16.to_le_bytes()); // date
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(nb.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // extra len
        out.extend_from_slice(nb);
        out.extend_from_slice(data);
    }
    let cd_start = out.len() as u32;
    for (i, (name, data)) in entries.iter().enumerate() {
        let crc = crc32(data);
        let nb = name.as_bytes();
        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes()); // version made by
        central.extend_from_slice(&20u16.to_le_bytes()); // version needed
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(data.len() as u32).to_le_bytes());
        central.extend_from_slice(&(nb.len() as u16).to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes()); // extra
        central.extend_from_slice(&0u16.to_le_bytes()); // comment
        central.extend_from_slice(&0u16.to_le_bytes()); // disk
        central.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
        central.extend_from_slice(&0u32.to_le_bytes()); // external attrs
        central.extend_from_slice(&offsets[i].to_le_bytes());
        central.extend_from_slice(nb);
    }
    let cd_len = central.len() as u32;
    out.extend_from_slice(&central);
    // end of central directory
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&cd_len.to_le_bytes());
    out.extend_from_slice(&cd_start.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_header_is_valid() {
        let px = vec![255u8; 2 * 2 * 4];
        let png = png_rgba(2, 2, &px);
        assert_eq!(&png[0..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        // contains IHDR / IDAT / IEND
        assert!(png.windows(4).any(|w| w == b"IHDR"));
        assert!(png.windows(4).any(|w| w == b"IDAT"));
        assert!(png.windows(4).any(|w| w == b"IEND"));
    }

    #[test]
    fn zip_has_signatures() {
        let z = zip_store(&[("a.txt".into(), b"hello".to_vec())]);
        assert_eq!(&z[0..4], &0x0403_4b50u32.to_le_bytes());
        assert!(z.windows(4).any(|w| w == 0x0605_4b50u32.to_le_bytes()));
    }
}
