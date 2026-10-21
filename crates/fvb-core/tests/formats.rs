//! Oscillogram file formats pinned to the original (ledger L3,
//! `notes/FORMATS_SPEC.md`).

use fvb_core::comtrade::{self, parse_cff, write_cff, CffSections, ComtradeError, Encoding};

const CFG: &str = include_str!("../../../assets/demo/demo_fault.cfg");
const DAT: &[u8] = include_bytes!("../../../assets/demo/demo_fault.dat");

fn demo() -> comtrade::Oscillogram {
    comtrade::parse(CFG, DAT).unwrap()
}

/// `.cfg` data-file types: exactly ASCII, BINARY or binary32 (0x45158a).
#[test]
fn cfg_file_type_spellings() {
    for bad in ["FLOAT32", "BINARY32", "ascii"] {
        let cfg = CFG.replacen("\nASCII", &format!("\n{bad}"), 1);
        assert!(matches!(comtrade::parse(&cfg, DAT), Err(ComtradeError::Format)), "{bad}");
    }
    assert_eq!(ComtradeError::Format.to_string(), "Неправильный формат файла");
}

/// Our `.cff` writer output reads back through the original's reader rules.
#[test]
fn cff_round_trip() {
    let mut osc = demo();
    osc.rev_year = 2013;
    let sections = CffSections { hdr: Some("заголовок\r\n".into()), ..Default::default() };
    let bytes = write_cff(&osc, &sections).unwrap();
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.starts_with("--- file type: CFG ---\r\n"));
    assert!(text.contains("--- file type: DAT BINARY: "));
    let (back, secs) = parse_cff(&bytes, Encoding::Cp1251).unwrap();
    assert_eq!(back.analog.len(), osc.analog.len());
    assert_eq!(back.analog[0].samples.len(), osc.analog[0].samples.len());
    // Same values as a plain .cfg/.dat round trip (the writer quantises to
    // integer sample codes).
    let (c, d) = comtrade::write(&osc);
    let plain = comtrade::parse(&c, d.as_bytes()).unwrap();
    assert_eq!(back.analog[0].samples, plain.analog[0].samples);
    assert_eq!(secs.hdr.as_deref(), Some("заголовок\r\n"));
}

/// Only COMTRADE'13+ records can be written as `.cff` (error 187).
#[test]
fn cff_needs_revision_2013() {
    let osc = demo();
    assert!(osc.rev_year < 2013);
    assert!(write_cff(&osc, &CffSections::default()).is_err());
}

/// No CFG or DAT section → error 330.
#[test]
fn cff_missing_sections() {
    assert!(matches!(parse_cff(b"--- file type: DAT ASCII: 4 ---\r\n1234", Encoding::Auto), Err(ComtradeError::Format)));
    let no_dat = format!("--- file type: CFG ---\r\n{CFG}");
    assert!(matches!(parse_cff(no_dat.as_bytes(), Encoding::Auto), Err(ComtradeError::Format)));
}

/// The DAT byte count is the first digit run on the marker line
/// (`BINARY32: 100` → 32), and the payload starts one byte after the line's
/// terminator — the LF of a CRLF pair.
#[test]
fn cff_dat_line_quirks() {
    // One analog channel, binary int16: rows of sample#, timestamp, value.
    let cfg = "S,D,2013\r\n1,1A,0D\r\n1,X,,,V,1,0,0,-32768,32767,1,1,S\r\n50\r\n1\r\n1000,2\r\n\
               01/01/2024,00:00:00.000000\r\n01/01/2024,00:00:00.000000\r\nBINARY\r\n1\r\n";
    let row = |n: u32, v: i16| [n.to_le_bytes().to_vec(), 0u32.to_le_bytes().to_vec(), v.to_le_bytes().to_vec()].concat();
    let payload = [row(1, 100), row(2, 200)].concat();
    let mut file = format!("--- file type: CFG ---\r\n{cfg}--- file type: DAT BINARY: {}---\r\n", payload.len()).into_bytes();
    file.extend(&payload);
    let (rec, _) = parse_cff(&file, Encoding::Auto).unwrap();
    // Shifted by one byte: the LF is read as the first data byte, so both
    // rows are misaligned.
    assert_eq!(rec.analog[0].samples.len(), 2);
    assert_ne!(rec.analog[0].samples[0], 100.0);
    assert_ne!(rec.analog[0].samples[1], 200.0);
    // `BINARY32: 100` gives a byte count of 32.
    let file2 = format!("--- file type: CFG ---\r\n{}--- file type: DAT BINARY32: 100---\r\n{}", CFG.replace('\n', "\r\n"), "x".repeat(200));
    let r = parse_cff(file2.as_bytes(), Encoding::Auto);
    assert!(r.is_ok() || matches!(r, Err(ComtradeError::Dat(_))));
}

/// Build an OSC2 file per the verified layout: block 0, one calibration
/// block, one text block, the info block, then data blocks.
fn osc_file(text: &str, blocks: &[(i16, Vec<[i16; 2]>)]) -> Vec<u8> {
    let mut b0 = [0u8; 128];
    b0[0] = 2;
    b0[1] = 6;
    b0[2] = 3; // H = block 0 + 1 calibration + 1 text
    let mut cal = [0u8; 128];
    cal[0..4].copy_from_slice(&(2.0f32 * 3276.8).to_be_bytes()); // F1[0]
    cal[0x20..0x22].copy_from_slice(&1i16.to_be_bytes()); // S1[0]
    cal[0x30..0x34].copy_from_slice(&1.0f32.to_be_bytes()); // F2[0]
    cal[0x50..0x52].copy_from_slice(&1i16.to_be_bytes()); // S2[0]
    let mut txt = [0u8; 128];
    txt[..text.len()].copy_from_slice(text.as_bytes());
    let mut info = [0u8; 128];
    info[2..4].copy_from_slice(&(blocks.len() as i16).to_be_bytes());
    info[0x40..0x44].copy_from_slice(&100.0f32.to_be_bytes());
    let mut out = [b0, cal, txt, info].concat();
    for (code, samples) in blocks {
        let mut blk = [0u8; 128];
        for (s, [a, b]) in samples.iter().enumerate() {
            blk[4 * s..4 * s + 2].copy_from_slice(&a.to_be_bytes());
            blk[4 * s + 2..4 * s + 4].copy_from_slice(&b.to_be_bytes());
        }
        blk[0x7C..0x7E].copy_from_slice(&code.to_be_bytes());
        out.extend(blk);
    }
    out
}

/// OSC2 reading (0x4526a0 / 0x453010): calibration a/b, channel table,
/// version gain, first-block rate snapping, per-block rates and µs time.
#[test]
fn osc2_reader() {
    let text = "01A,02\r\n1,Ia,A,A\r\n2,,B,V\r\n01F,4\r\n";
    // M = 2 (units A and V): code = 1e6 / (Fs·0.0625).
    let bytes = osc_file(text, &[(16000, vec![[10, 1]; 4]), (8000, vec![[20, 2]; 4])]);
    let rec = comtrade::parse_osc(&bytes, Encoding::Cp1251).unwrap();
    assert_eq!(rec.device, "OSC Осциллограф. Версия 2.6");
    assert_eq!(rec.harmonic1_gain, 1.00259);
    assert_eq!(rec.analog[0].id, "Ia");
    assert_eq!(rec.analog[1].id, "Резерв.");
    // Calibration floats are float32 in the file, as in the original.
    assert!((rec.analog[0].a - 2.0).abs() < 1e-6 && (rec.analog[0].b + 4.0).abs() < 1e-6);
    assert_eq!(rec.analog[0].primary, 100.0);
    let want = [16.0, 16.0, 16.0, 16.0, 36.0, 36.0, 36.0, 36.0];
    assert!(rec.analog[0].samples.iter().zip(want).all(|(g, w)| (g - w).abs() < 1e-5));
    assert_eq!(rec.analog[0].samples.len(), 8);
    assert_eq!(rec.analog[1].samples[0], 1.0);
    // First block 1000 Hz snapped to 1200 (833 µs); second block 2000 Hz (500 µs).
    assert_eq!(rec.sample_rate, 1200.0);
    assert!((rec.time[1] - 0.000833).abs() < 1e-12);
    assert!((rec.time[5] - rec.time[4] - 0.0005).abs() < 1e-12);
    assert_eq!(rec.rev_year, 0, "an OSC record cannot be saved as .cff");
    // No nnF line → error 330.
    let bad = osc_file("01A,01\r\n1,Ia,A,A\r\n", &[(16000, vec![[1, 1]; 4])]);
    assert!(matches!(comtrade::parse_osc(&bad, Encoding::Cp1251), Err(ComtradeError::Format)));
}

/// Writes `rec.cfg` + `rec.dat` into a fresh temp folder; the `.cfg` path.
fn temp_record(tag: &str, cfg: &str, dat: &[u8]) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("fv_progress_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("rec.cfg"), cfg).unwrap();
    std::fs::write(dir.join("rec.dat"), dat).unwrap();
    dir.join("rec.cfg")
}

/// The «Загрузка» counters (DIALOG 492, `0x471870`; ledger L9.31): total
/// 5·S; ASCII position = bytes consumed (the parse fills 20 %), BINARY
/// `T − remaining` (100 % already), binary32 no total (0 % throughout);
/// then ⌊T/(N·C)⌋ per sample of every analog channel; the bar shows
/// `((pos·100) mod 2³²)/T`, clamped to 100. A cancel stops the reader.
#[test]
fn loading_progress_follows_the_original_counters() {
    use comtrade::Progress;
    use std::sync::atomic::Ordering::Relaxed;
    let (n, a, c) = (480u32, 6u32, 7u32);
    let run = |tag: &str, cfg: &str, dat: &[u8]| {
        let p = Progress::default();
        let rec = comtrade::load_progress(temp_record(tag, cfg, dat), Encoding::Auto, Some(&p)).unwrap();
        (p, rec)
    };
    // ASCII.
    let s = DAT.len() as u32;
    let (p, rec) = run("ascii", CFG, DAT);
    assert_eq!((rec.analog.len() as u32, rec.analog[0].samples.len() as u32), (a, n));
    let t = 5 * s;
    assert_eq!(p.total.load(Relaxed), t);
    assert_eq!(p.pos.load(Relaxed), s + a * n * (t / (n * c)), "S after the parse, then the steps");
    assert_eq!(p.percent(), ((s + a * n * (t / (n * c))) * 100 / t).min(100));
    // BINARY: 8 + 6·2 + 2 bytes a record.
    let bin = vec![0u8; (n * 22) as usize];
    let (p, _) = run("binary", &CFG.replacen("\nASCII", "\nBINARY", 1), &bin);
    let t = 5 * bin.len() as u32;
    assert_eq!(p.pos.load(Relaxed), t + a * n * (t / (n * c)), "T − 0 after the last record");
    assert_eq!(p.percent(), 100);
    // binary32: the total is never set.
    let bin32 = vec![0u8; (n * 34) as usize];
    let (p, rec) = run("binary32", &CFG.replacen("\nASCII", "\nbinary32", 1), &bin32);
    assert_eq!(rec.analog[0].samples.len() as u32, n);
    assert_eq!((p.total.load(Relaxed), p.percent()), (0, 0));
    // Cancelled before the first line: nothing is read.
    let p = Progress::default();
    p.cancel.store(true, Relaxed);
    let rec = comtrade::load_progress(temp_record("cancel", CFG, DAT), Encoding::Auto, Some(&p));
    assert!(rec.map_or(true, |r| r.analog[0].samples.is_empty()));
    // The display wraps like the original's 32-bit multiply.
    let p = Progress::default();
    p.total.store(1000, Relaxed);
    p.pos.store(42_949_673, Relaxed);
    assert_eq!(p.percent(), 0, "(42 949 673·100) mod 2³² = 4");
}

/// `.osc` (`0x453080`, `0x45342d`, `0x4525d3`): total 5 × the bytes after
/// the header, `T − remaining` after each block, then a ×5 step.
#[test]
fn osc_loading_progress() {
    use std::sync::atomic::Ordering::Relaxed;
    let text = "01A,02\r\n1,Ia,A,A\r\n2,,B,V\r\n01F,4\r\n";
    let bytes = osc_file(text, &[(16000, vec![[10, 1]; 4]), (8000, vec![[20, 2]; 4])]);
    let p = comtrade::Progress::default();
    let rec = comtrade::parse_osc_progress(&bytes, Encoding::Cp1251, Some(&p)).unwrap();
    let t = 5 * (bytes.len() as u32 - 4 * 128);
    let (n, c) = (rec.analog[0].samples.len() as u32, rec.analog.len() as u32 + rec.digital.len() as u32);
    assert_eq!(p.total.load(Relaxed), t);
    assert_eq!(p.pos.load(Relaxed), t + rec.analog.len() as u32 * n * 5 * (t / (n * c)));
}
