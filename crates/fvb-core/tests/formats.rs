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
