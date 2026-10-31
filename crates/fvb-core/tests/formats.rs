//! Oscillogram file formats pinned to the original (ledger L3,
//! `docs/original/specs/formats.md`).

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

/// Our `.cff` writer's sections read back through the original's reader
/// rules. The samples do not: the data is int32 rows (revision 2013)
/// whatever the format line says, which the reader takes as that line's
/// kind — the original's own `.cff` of an «ASCII» record opens as zeros
/// (VM).
#[test]
fn cff_round_trip() {
    let mut osc = demo();
    osc.rev_year = 2013;
    osc.rev_text = "2013".into();
    let sections = CffSections { hdr: Some("заголовок\r\n".into()), ..Default::default() };
    let bytes = write_cff(&osc, &sections, comtrade::Encoding::Cp1251).unwrap();
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.starts_with("--- file type: CFG ---\r\n"));
    assert!(text.contains("--- file type: DAT BINARY: "));
    let (back, secs) = parse_cff(&bytes, Encoding::Cp1251).unwrap();
    assert_eq!(back.analog.len(), osc.analog.len());
    assert_eq!(secs.hdr.as_deref(), Some("заголовок\r\n"));
}

/// The `.cff` data is binary for an «ASCII» record too — int32 number,
/// int32 time, int32 values (revision 2013), the digital words — and the
/// format line stays «ASCII» (VM, the original's file byte for byte).
#[test]
fn cff_data_is_binary_for_an_ascii_record() {
    let mut osc = demo();
    osc.rev_year = 2013;
    osc.rev_text = "2013".into();
    assert_eq!(osc.format, "ASCII");
    let bytes = write_cff(&osc, &CffSections::default(), comtrade::Encoding::Cp1251).unwrap();
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("\r\nASCII\r\n"));
    let n = osc.sample_count();
    let row = 4 + 4 + 4 * osc.analog.len() + 2 * osc.digital.len().div_ceil(16);
    assert!(text.contains(&format!("--- file type: DAT BINARY: {}---\r\n", n * row)));
    let at = bytes.windows(3).rposition(|w| w == b"---").unwrap() + 5;
    assert_eq!(&bytes[at..at + 8], &[1, 0, 0, 0, 0, 0, 0, 0]);
}

/// The ASCII data writes integers: a value rounding to −0 is «0» (VM).
#[test]
fn ascii_data_writes_minus_zero_as_zero() {
    let osc = demo();
    let (_, dat) = comtrade::write(&osc, comtrade::Encoding::Cp1251);
    let text = String::from_utf8(dat).unwrap();
    assert!(!text.contains(", -0,") && !text.contains(", -0\r"));
    assert!(text.starts_with("1,0, 0, -71, 71, 0, -1, 1, 0\r\n"));
}

/// Only COMTRADE'13+ records can be written as `.cff` (error 187).
#[test]
fn cff_needs_revision_2013() {
    let osc = demo();
    assert!(osc.rev_year < 2013);
    assert!(write_cff(&osc, &CffSections::default(), comtrade::Encoding::Cp1251).is_err());
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
    // Its 32 bytes are one ASCII field, so no row: the loader's
    // zero-sample check (`0x450bc1`) answers string 330.
    assert!(matches!(r, Err(ComtradeError::FormatAt(_))), "{:?}", r.map(|r| r.0.analog.len()));
}

/// Build an OSC2 file per the verified layout: block 0, one calibration
/// block, the text blocks (one per tag), the info block, then data blocks.
fn osc_file(texts: &[&str], blocks: &[(i16, Vec<[i16; 2]>)]) -> Vec<u8> {
    let mut b0 = [0u8; 128];
    b0[0] = 2;
    b0[1] = 6;
    b0[2] = 2 + texts.len() as u8; // H = block 0 + 1 calibration + the texts
    let mut cal = [0u8; 128];
    cal[0..4].copy_from_slice(&(2.0f32 * 3276.8).to_be_bytes()); // F1[0]
    cal[0x20..0x22].copy_from_slice(&1i16.to_be_bytes()); // S1[0]
    cal[0x30..0x34].copy_from_slice(&1.0f32.to_be_bytes()); // F2[0]
    cal[0x50..0x52].copy_from_slice(&1i16.to_be_bytes()); // S2[0]
    let txt: Vec<[u8; 128]> = texts
        .iter()
        .map(|t| {
            let mut b = [0u8; 128];
            b[..t.len()].copy_from_slice(t.as_bytes());
            b
        })
        .collect();
    let mut info = [0u8; 128];
    info[2..4].copy_from_slice(&(blocks.len() as i16).to_be_bytes());
    info[0x40..0x44].copy_from_slice(&100.0f32.to_be_bytes());
    let mut out = [vec![b0, cal], txt, vec![info]].concat().concat();
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
    let text = ["nnA,02\r\n1,Ia,A,A\r\n2,,B,V\r\n", "nnF,4\r\n"];
    // M = 2 (units A and V): code = 1e6 / (Fs·0.0625).
    let bytes = osc_file(&text, &[(16000, vec![[10, 1]; 4]), (8000, vec![[20, 2]; 4])]);
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
    // No nnF line → error 330; a tag is its three characters exactly.
    let bad = osc_file(&["nnA,01\r\n1,Ia,A,A\r\n"], &[(16000, vec![[1, 1]; 4])]);
    assert!(matches!(comtrade::parse_osc(&bad, Encoding::Cp1251), Err(ComtradeError::Format)));
    let bad = osc_file(&["01A,01\r\n1,Ia,A,A\r\n", "nnF,4\r\n"], &[(16000, vec![[1, 1]; 4])]);
    assert!(matches!(comtrade::parse_osc(&bad, Encoding::Cp1251), Err(ComtradeError::Format)));
}

/// OSC2 channel lines (`0x424800`): analog 4 or 5 fields untrimmed, else
/// «Резерв.» with nothing; a block short of lines gives «Резерв.»
/// channels; digital: the name up to the comma searched from two
/// characters on, the normal state `_wtoi` after it, no phase.
#[test]
fn osc2_channel_lines() {
    let text = ["nnA,04\r\n1, Ia,A,cc,kA\r\n2,Ib,B,A,x,y\r\n\r\n3,,C,V\r\n", "nnD,3\r\n1,K1,1\r\n2,,0\r\n3,K3\r\n", "nnF,4\r\n"];
    let rec = comtrade::parse_osc(&osc_file(&text, &[(16000, vec![[10, 1]; 4])]), Encoding::Cp1251).unwrap();
    let a: Vec<(&str, &str, &str, &str)> = rec.analog.iter().map(|c| (c.id.as_str(), c.phase.as_str(), c.ccbm.as_str(), c.raw_unit.as_str())).collect();
    assert_eq!(a, [(" Ia", "A", "cc", "kA"), ("Резерв.", "", "", ""), ("Резерв.", "C", "", "V"), ("Резерв.", "", "", "")]);
    let d: Vec<(&str, &str, i32)> = rec.digital.iter().map(|c| (c.id.as_str(), c.phase.as_str(), c.normal)).collect();
    assert_eq!(d, [("K1", "", 1), (",0", "", 2), ("K3", "", 3)]);
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
    // The cursor after the last row: past its last value's delimiter, so
    // short of a CR LF's LF (a checkout with CR LF line ends).
    let end = s - u32::from(DAT.ends_with(b"\r\n"));
    assert_eq!(p.pos.load(Relaxed), end + a * n * (t / (n * c)), "the cursor after the parse, then the steps");
    assert_eq!(p.percent(), ((end + a * n * (t / (n * c))) * 100 / t).min(100));
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
    // Cancelled before the first line: nothing is read, and the loader's
    // zero-sample check (`0x450bc1`) gives string 330 with the record's
    // path less its extension.
    let p = Progress::default();
    p.cancel.store(true, Relaxed);
    let path = temp_record("cancel", CFG, DAT);
    let base = path.with_extension("").to_string_lossy().into_owned();
    match comtrade::load_progress(&path, Encoding::Auto, Some(&p)) {
        Err(comtrade::ComtradeError::FormatAt(d)) => assert_eq!(d, base),
        r => panic!("{:?}", r.map(|r| r.analog.len())),
    }
    // So does an empty data file, cancelled or not.
    assert!(matches!(comtrade::load_progress(temp_record("empty", CFG, b""), Encoding::Auto, None), Err(comtrade::ComtradeError::FormatAt(_))));
    // A `.cff` cancelled during its split: the unpacked copy's path.
    let mut osc = demo();
    osc.rev_year = 2013;
    osc.rev_text = "2013".into();
    let bytes = write_cff(&osc, &CffSections::default(), Encoding::Cp1251).unwrap();
    let p = Progress::default();
    p.cancel.store(true, Relaxed);
    match comtrade::parse_cff_progress(&bytes, Encoding::Cp1251, Some(&p)) {
        Err(comtrade::ComtradeError::FormatAt(d)) => assert!(d.contains("Fast_View_") && d.ends_with("tmp_cfg"), "{d}"),
        r => panic!("{:?}", r.map(|r| r.0.analog.len())),
    }
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
    let text = ["nnA,02\r\n1,Ia,A,A\r\n2,,B,V\r\n", "nnF,4\r\n"];
    let bytes = osc_file(&text, &[(16000, vec![[10, 1]; 4]), (8000, vec![[20, 2]; 4])]);
    let p = comtrade::Progress::default();
    let rec = comtrade::parse_osc_progress(&bytes, Encoding::Cp1251, Some(&p)).unwrap();
    let t = 5 * (bytes.len() as u32 - 5 * 128);
    let (n, c) = (rec.analog[0].samples.len() as u32, rec.analog.len() as u32 + rec.digital.len() as u32);
    assert_eq!(p.total.load(Relaxed), t);
    assert_eq!(p.pos.load(Relaxed), t + rec.analog.len() as u32 * n * 5 * (t / (n * c)));
}

/// The `.cfg` / `.dat` writer as the original's (`0x44dfa0`, `0x425530`,
/// `0x44e680`; checked byte for byte against the original's output on
/// three records, L9.172): CR LF, `%g` coefficients, min / max from the
/// data, primary / secondary tokens as read, the discrete line with its
/// normal state, ", " in the data rows, no time-code line.
#[test]
fn cfg_dat_writer_as_the_original() {
    let cfg = "St,Dev,1999\r\n2,1A,1D\r\n1,Ia,A,,кА,0.031846127,0.000000000,0.000000,-2047,2047,2380.000000,1.000000,P\r\n\
               1,D1,,,1\r\n50.000000\r\n1\r\n1000.000000,3\r\n01/01/2024,00:00:00.000000\r\n01/01/2024,00:00:00.001000\r\nASCII\r\n1.000000\r\n";
    let dat = "1,0,10,1\r\n2,1000,-20,0\r\n3,2000,5,1\r\n";
    let rec = comtrade::parse(cfg, dat.as_bytes()).unwrap();
    let (c, d) = comtrade::write(&rec, Encoding::Cp1251);
    let c = comtrade::decode_bytes(&c, Encoding::Cp1251);
    assert_eq!(
        c,
        "St,Dev,1999\r\n2,1A,1D\r\n1,Ia,A,,кА,0.0318461,0,0,-20000,10000,2380.000000,1.000000,P\r\n1,D1,,,1\r\n50\r\n1\r\n1000,3\r\n\
         01/01/2024,00:00:00.000000\r\n01/01/2024,00:00:00.001000\r\nASCII\r\n1\r\n"
    );
    assert_eq!(String::from_utf8(d).unwrap(), "1,0, 10, 1\r\n2,1000, -20, 0\r\n3,2000, 5, 1\r\n");
}

/// binary32 samples carry the unit multiplier as BINARY does: the store
/// `0x449200` (called from `0x4522c2`) computes unit · a · raw + b.
#[test]
fn binary32_applies_the_unit_multiplier() {
    let cfg_of = |unit: &str, ty: &str| {
        format!(
            "S,D,1999\r\n1,1A,0D\r\n1,X,,,{unit},2,0.5,0,-2147483648,2147483647,1,1,S\r\n50\r\n1\r\n1000,2\r\n\
             01/01/2024,00:00:00.000000\r\n01/01/2024,00:00:00.000000\r\n{ty}\r\n1\r\n"
        )
    };
    let row32 = |n: u32, v: i32| [n.to_le_bytes().to_vec(), 0u32.to_le_bytes().to_vec(), v.to_le_bytes().to_vec()].concat();
    let row16 = |n: u32, v: i16| [n.to_le_bytes().to_vec(), 0u32.to_le_bytes().to_vec(), v.to_le_bytes().to_vec()].concat();
    let b32 = comtrade::parse(&cfg_of("кВ", "binary32"), &[row32(1, 10), row32(2, 20)].concat()).unwrap();
    let b16 = comtrade::parse(&cfg_of("кВ", "BINARY"), &[row16(1, 10), row16(2, 20)].concat()).unwrap();
    assert_eq!(b32.analog[0].samples, b16.analog[0].samples);
    let m = b32.analog[0].unit_mult;
    assert_eq!(b32.analog[0].samples, vec![m * 2.0 * 10.0 + 0.5, m * 2.0 * 20.0 + 0.5]);
}

/// The tokenizer's integer (`0x477130`), used for the DAT timestamps and
/// digital columns and the `.cfg` rates: a wrapping i32, every '-' flips
/// the sign, '+' or '.' ends it.
#[test]
fn the_tokenizer_integer() {
    use comtrade::read_int;
    assert_eq!(read_int(" 123"), 123);
    assert_eq!(read_int("12-3"), -123);
    assert_eq!(read_int("--5"), 5);
    assert_eq!(read_int("+7"), 0);
    assert_eq!(read_int("1.0"), 1);
    assert_eq!(read_int("2147483648"), i32::MIN);
}

/// The ASCII data reader scans bytes, not lines (`0x451800`): a row is
/// 2 + nA + nD fields wherever the line ends fall; a value is C's `atof`
/// of its field (the leading number: «1.5x» is 1.5, a NUL ends it).
#[test]
fn ascii_rows_are_fields_not_lines() {
    let cfg = "S,D,1999\r\n2,2A,0D\r\n1,X,,,V,1,0,0,-9,9\r\n2,Y,,,V,1,0,0,-9,9\r\n50\r\n1\r\n1000,3\r\n\
               01/01/2024,00:00:00.000000\r\n01/01/2024,00:00:00.000000\r\nASCII\r\n1\r\n";
    let dat = b"1,0,1.5x,2\r\n2,1000,\r\n7,8\r\n3,2000,-4\0,1e3\r\n";
    let rec = comtrade::parse(cfg, dat).unwrap();
    assert_eq!(rec.analog[0].samples, [1.5, 7.0, -4.0]);
    assert_eq!(rec.analog[1].samples, [2.0, 8.0, 1000.0]);
    assert_eq!(rec.time, [0.0, 0.001, 0.002]);
}
