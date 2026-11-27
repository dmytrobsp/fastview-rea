//! Embeds our own icon in `fastview.exe` (a Windows resource: Explorer and
//! shortcuts show it). The artwork is our logo drawing (`src/icon_art.rs`,
//! the same one the window icon uses), rendered here at several sizes into
//! an .ico of PNG entries; never the original's icon. The resource compiler
//! is `rc.exe` (MSVC) or MinGW's `windres` (the Windows GNU host build and
//! the Linux cross-build), found by the `embed-resource` crate. Other
//! targets build as before.

#[path = "src/icon_art.rs"]
mod icon_art;

use std::{env, fs, path::PathBuf};

const SIZES: [usize; 7] = [16, 24, 32, 48, 64, 128, 256];

/// An RGBA picture as a PNG file (8-bit, truecolour with alpha).
fn png(n: usize, rgba: &[u8]) -> Vec<u8> {
    use flate2::{write::ZlibEncoder, Compression, Crc};
    use std::io::Write;
    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        out.extend((data.len() as u32).to_be_bytes());
        out.extend(kind);
        out.extend(data);
        let mut crc = Crc::new();
        crc.update(kind);
        crc.update(data);
        out.extend(crc.sum().to_be_bytes());
    }
    let mut raw = Vec::with_capacity(n * (n * 4 + 1));
    for row in rgba.chunks(n * 4) {
        raw.push(0);
        raw.extend(row);
    }
    let mut z = ZlibEncoder::new(Vec::new(), Compression::best());
    z.write_all(&raw).unwrap();
    let mut head = Vec::new();
    head.extend((n as u32).to_be_bytes());
    head.extend((n as u32).to_be_bytes());
    head.extend([8, 6, 0, 0, 0]);
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    chunk(&mut out, b"IHDR", &head);
    chunk(&mut out, b"IDAT", &z.finish().unwrap());
    chunk(&mut out, b"IEND", &[]);
    out
}

/// An .ico file of PNG entries, one per size.
fn ico() -> Vec<u8> {
    let pics: Vec<Vec<u8>> = SIZES.iter().map(|&n| png(n, &icon_art::render(n))).collect();
    let mut out = Vec::new();
    out.extend([0, 0, 1, 0]);
    out.extend((pics.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * pics.len();
    for (&n, p) in SIZES.iter().zip(&pics) {
        // 256 is stored as 0.
        out.extend([(n % 256) as u8, (n % 256) as u8, 0, 0]);
        out.extend(1u16.to_le_bytes());
        out.extend(32u16.to_le_bytes());
        out.extend((p.len() as u32).to_le_bytes());
        out.extend((offset as u32).to_le_bytes());
        offset += p.len();
    }
    for p in pics {
        out.extend(p);
    }
    out
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/icon_art.rs");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    fs::write(out.join("fastview.ico"), ico()).expect("write fastview.ico");
    // Resource 1 is the lowest id: the icon Explorer shows.
    let rc = out.join("fastview.rc");
    fs::write(&rc, "1 ICON \"fastview.ico\"\n").expect("write fastview.rc");
    embed_resource::compile(&rc, embed_resource::NONE);
}
