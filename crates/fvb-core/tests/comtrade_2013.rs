//! A COMTRADE 2013 export as a modern fault recorder writes it (issue #8):
//! a UTF-8 `.cfg` with a BOM and Cyrillic names (one cut by the exporter
//! mid-character, a U+FFFD left in it), the 2013 lines `timemult`,
//! `time_code,local_code` and `tmq_code,leapsec`, an ASCII `.dat` with
//! samples spread over the whole float32 range and scale factors `a` near
//! 1e‑37, and a UTF-8 `.hdr`. A small synthetic record, no real data.

use fvb_core::comtrade::{self, Encoding};

/// The `.cfg` text (without BOM) with the given revision token.
fn cfg(rev: &str) -> String {
    [
        &format!("Підстанція Тест,Реєстратор 1,{rev}"),
        "4,2A,2D",
        "1,Струм фази А,A,,A,2.5e-37,5.0e-03,0,-3.4028235E38,3.4028235E38,10000,5,S",
        "2,Гр.\u{FFFD}.:Напруга шин,,,kV,2.0e-37,1.25e-02,0,-3.4028235E38,3.4028235E38,10000,5,S",
        "1,Пуск захисту,,,0",
        "2,Вимкнення,,,0",
        "50",
        "1",
        "1000.000000,4",
        "14/03/2025,09:15:30.250000",
        "14/03/2025,09:15:30.450000",
        "ASCII",
        "1.000000",
        "+2h0,+2h0",
        "1,0",
        "",
    ]
    .join("\r\n")
}

const DAT: &[u8] = b"0000000001,0000000000000,-3.0e+38,9.0e+37,0,1\r\n\
0000000002,0000000001000,2.4e+38,-3.2e+38,1,1\r\n\
0000000003,0000000002000,3.4028235e+38,1.0e-3,1,0\r\n\
0000000004,0000000003000,-3.4028235e+38,0,0,0\r\n";

const HDR: &str = "Коментар: синтетичний запис для тесту\r\nПримітка: «Тест»\r\n";

/// The record's three files in a fresh folder; returns the `.cfg` path.
fn record(tag: &str, cfg: &[u8]) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("fv_2013_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("rec.cfg"), cfg).unwrap();
    std::fs::write(dir.join("rec.dat"), DAT).unwrap();
    std::fs::write(dir.join("rec.hdr"), HDR).unwrap();
    dir.join("rec.cfg")
}

fn with_bom(text: &str) -> Vec<u8> {
    [b"\xEF\xBB\xBF".as_slice(), text.as_bytes()].concat()
}

/// A BOM fixes the `.cfg` as UTF-8 whatever the program encoding (the
/// original widens through `Display Encoding`, 1251 by default, and shows
/// mojibake with «п»ї» before the station); the BOM is not part of the
/// station; the exporter's U+FFFD stays in its name.
#[test]
fn a_bom_cfg_reads_as_utf8_whatever_the_program_encoding() {
    let path = record("bom", &with_bom(&cfg("2013")));
    for enc in [Encoding::Cp1251, Encoding::Cp866, Encoding::Koi8r, Encoding::Utf8] {
        let rec = comtrade::load_with(&path, enc).unwrap();
        assert_eq!((rec.station.as_str(), rec.device.as_str()), ("Підстанція Тест", "Реєстратор 1"), "{enc:?}");
        assert_eq!(rec.analog[0].id, "Струм фази А");
        assert_eq!(rec.analog[1].id, "Гр.\u{FFFD}.:Напруга шин");
        assert_eq!(rec.digital[1].id, "Вимкнення");
        assert_eq!(rec.text_enc, Some(Encoding::Utf8));
        assert_eq!((rec.rev_year, rec.rev_text.as_str()), (2013, "2013"));
    }
    // A BOM in front of a revision-1999 file counts as well.
    let rec = comtrade::load_with(record("bom1999", &with_bom(&cfg("1999"))), Encoding::Cp1251).unwrap();
    assert_eq!((rec.station.as_str(), rec.text_enc), ("Підстанція Тест", Some(Encoding::Utf8)));
}

/// Without a BOM a revision-2013 `.cfg` that is valid UTF-8 (with a
/// multi-byte character) is UTF-8 too; an older revision, or bytes that are
/// not UTF-8 (a 1251 file), stay with the program encoding as before.
#[test]
fn a_bomless_2013_cfg_in_utf8_is_utf8_others_keep_the_program_encoding() {
    let rec = comtrade::load_with(record("plain2013", cfg("2013").as_bytes()), Encoding::Cp1251).unwrap();
    assert_eq!((rec.station.as_str(), rec.text_enc), ("Підстанція Тест", Some(Encoding::Utf8)));
    let rec = comtrade::load_with(record("plain1999", cfg("1999").as_bytes()), Encoding::Cp1251).unwrap();
    assert_eq!(rec.text_enc, None);
    assert_eq!(rec.station, comtrade::decode_bytes("Підстанція Тест".as_bytes(), Encoding::Cp1251), "1251 as the original reads it");
    let text = cfg("2013").replace('\u{FFFD}', "?");
    let rec = comtrade::load_with(record("cp1251", &comtrade::encode_text(&text, Encoding::Cp1251)), Encoding::Cp1251).unwrap();
    assert_eq!((rec.analog[0].id.as_str(), rec.text_enc), ("Струм фази А", None));
    // Plain ASCII fixes nothing either.
    assert_eq!(comtrade::cfg_utf8(b"S,D,2013\r\n", false), None);
}

/// The 2013 lines: the multiplier, `time_code,local_code` (the local stamp
/// adds the offset), `tmq_code,leapsec` ignored as in the original.
#[test]
fn the_2013_time_lines() {
    let rec = comtrade::load_with(record("time", &with_bom(&cfg("2013"))), Encoding::Cp1251).unwrap();
    assert_eq!(rec.time_mult, 1.0);
    assert_eq!((rec.stamps.time_code.as_str(), rec.stamps.local_code.as_str()), ("+2h0", "+2h0"));
    let st = &rec.stamps;
    assert_eq!(comtrade::format_stamp(&st.start_date, &st.start_time, rec.rev_year, &st.local_code, true), "14.03.2025, 09:15:30.250000 (UTC)");
    assert_eq!(comtrade::format_stamp(&st.start_date, &st.start_time, rec.rev_year, &st.local_code, false), "14.03.2025, 11:15:30.250000 (Местное время)");
    assert_eq!(rec.time, [0.0, 0.001, 0.002, 0.003]);
    assert_eq!(rec.sample_count(), 4);
}

/// Float samples over the whole float32 range with `a` ≈ 1e‑37: the value
/// is `m·a·x + b` in f64 (`0x451904`) — a few tens of volts here; the cfg's
/// min / max (±3.40282e+38) are not used; the ADC code (`0x422250`) gives
/// the raw sample back, `m` included, however small `a`.
#[test]
fn float32_range_samples_with_tiny_scale_factors() {
    let rec = comtrade::load_with(record("float", &with_bom(&cfg("2013"))), Encoding::Cp1251).unwrap();
    let [ia, ur] = [&rec.analog[0], &rec.analog[1]];
    let v = |c: &comtrade::AnalogChannel, x: f64| c.unit_mult * c.a * x + c.b;
    assert_eq!(ia.samples, [v(ia, -3.0e+38), v(ia, 2.4e+38), v(ia, 3.4028235e+38), v(ia, -3.4028235e+38)]);
    assert!((ia.samples[0] - -74.995).abs() < 1e-4 && (ia.samples[2] - 85.0756).abs() < 1e-4, "{:?}", ia.samples);
    assert_eq!(ur.unit_mult, 1000.0, "kV");
    assert_eq!(ur.samples[1], 1000.0 * ur.a * -3.2e+38 + ur.b, "kV: × 1000");
    assert!((ur.samples[1] - -63999.99).abs() < 0.01, "{}", ur.samples[1]);
    for (c, raw) in [(ia, [-3.0e+38, 2.4e+38, 3.4028235e+38, -3.4028235e+38]), (ur, [9.0e+37, -3.2e+38, 1.0e-3, 0.0])] {
        for (s, x) in c.samples.iter().zip(raw as [f64; 4]) {
            let code = c.adc_code(*s);
            assert!((code - x.round()).abs() <= 1e-9 * x.abs().max(1.0), "{} {code} {x}", c.id);
        }
    }
    // The original's rounding: down unless at least half-way.
    let mut c = ia.clone();
    (c.a, c.b, c.unit_mult) = (1.0, 0.0, 1.0);
    assert_eq!([c.adc_code(-2.5), c.adc_code(2.5), c.adc_code(-2.6)], [-2.0, 3.0, -3.0]);
    assert!(rec.digital[0].samples == [false, true, true, false] && rec.digital[1].samples == [true, true, false, false]);
}

/// A `.cff` holding a UTF-8 revision-2013 record is read as UTF-8 too.
#[test]
fn a_utf8_cff_reads_as_utf8() {
    let text = format!("--- file type: CFG ---\r\n{}--- file type: HDR ---\r\n{HDR}--- file type: DAT ASCII: {} ---\r\n", cfg("2013"), DAT.len());
    let file = [with_bom(&text), DAT.to_vec()].concat();
    let (rec, sections) = comtrade::parse_cff(&file, Encoding::Cp1251).unwrap();
    assert_eq!((rec.station.as_str(), rec.analog[0].id.as_str(), rec.text_enc), ("Підстанція Тест", "Струм фази А", Some(Encoding::Utf8)));
    assert_eq!(sections.hdr.as_deref(), Some(HDR));
    assert_eq!(rec.analog[0].samples.len(), 4);
    let (rec, _) = comtrade::parse_cff(&file[3..], Encoding::Cp1251).unwrap();
    assert_eq!(rec.text_enc, Some(Encoding::Utf8), "no BOM: valid UTF-8, revision 2013");
}
