//! Embeds our own icon in `fastview.exe` (a Windows resource: Explorer and
//! shortcuts show it). The artwork is our logo drawing (`src/icon_art.rs`,
//! the same one the window icon uses), rendered here at several sizes into
//! an .ico (plain 32-bit DIB entries up to 64 px, PNG for the larger ones,
//! as Microsoft advises); never the original's icon. The resource compiler
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

/// An RGBA picture as an .ico entry's DIB: a BITMAPINFOHEADER of twice the
/// height (picture and mask), the 32-bit BGRA rows bottom-up, the all-clear
/// 1-bit AND mask (the alpha channel does the shaping).
fn dib(n: usize, rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend(40u32.to_le_bytes());
    out.extend((n as u32).to_le_bytes());
    out.extend((2 * n as u32).to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(32u16.to_le_bytes());
    out.extend([0u8; 24]);
    for row in rgba.chunks(n * 4).rev() {
        for p in row.chunks(4) {
            out.extend([p[2], p[1], p[0], p[3]]);
        }
    }
    // Mask rows are padded to 32 bits.
    out.extend(vec![0u8; n.div_ceil(32) * 4 * n]);
    out
}

/// An .ico file, one entry per size.
fn ico() -> Vec<u8> {
    let pics: Vec<Vec<u8>> = SIZES.iter().map(|&n| if n <= 64 { dib(n, &icon_art::render(n)) } else { png(n, &icon_art::render(n)) }).collect();
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

/// A cross-build to windows-gnu from another system compiles the resource
/// with MinGW's windres; embed-resource skips the step without a word when
/// none runs, and the exe would silently have no icon. Say so.
fn warn_if_no_windres() {
    if cfg!(windows) || env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("gnu") {
        return;
    }
    let target = env::var("TARGET").unwrap_or_default();
    let arch = target.split('-').next().unwrap_or("x86_64");
    let named = |k: &str| env::var_os(k).is_some();
    if named("RC") || named(&format!("RC_{target}")) || named(&format!("RC_{}", target.replace('-', "_"))) {
        return;
    }
    let tool = format!("{arch}-w64-mingw32-windres");
    if std::process::Command::new(&tool).arg("--version").output().is_err() {
        println!("cargo:warning={tool} not found: fastview.exe is built without its icon resource (install MinGW's binutils or set RC)");
    }
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/icon_art.rs");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    warn_if_no_windres();
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    fs::write(out.join("fastview.ico"), ico()).expect("write fastview.ico");
    // Resource 1 is the lowest id: the icon Explorer shows.
    let rc = out.join("fastview.rc");
    fs::write(&rc, "1 ICON \"fastview.ico\"\n").expect("write fastview.rc");
    embed_resource::compile(&rc, embed_resource::NONE);
}
