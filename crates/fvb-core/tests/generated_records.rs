//! Records made by `tools/records/synth_record.py` from four hand-written sources
//! (`tools/records/make_fixtures.py`): every name, number and sample in them is invented, and
//! the structure of each source is kept. They pin that our readers take the features a real
//! recorder's files have: a UTF-8 BOM with a name cut by U+FFFD, the float32 range with a tiny
//! factor `a`, int16 and int32 data with two rate segments and twenty digital channels, a name cut
//! mid-character (invalid UTF-8) at a byte limit, a revision-1991 layout.

use fvb_core::comtrade::{self, Encoding};
use std::path::PathBuf;

fn fixture(dir: &str, file: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/generated").join(dir).join(file)
}

/// 2013, BOM, ASCII, samples over the whole float32 range, a tiny `a`, a name cut by U+FFFD.
#[test]
fn utf8_bom_ascii_over_the_float32_range() {
    let raw = std::fs::read(fixture("a", "a.cfg")).unwrap();
    assert_eq!(&raw[..3], b"\xEF\xBB\xBF");
    for enc in [Encoding::Cp1251, Encoding::Utf8] {
        let rec = comtrade::load_with(fixture("a", "a.cfg"), enc).unwrap();
        assert_eq!((rec.text_enc, rec.text_bom, rec.rev_year), (Some(Encoding::Utf8), true, 2013));
        assert_eq!((rec.analog.len(), rec.digital.len(), rec.sample_count()), (2, 2, 96));
        assert!(rec.analog[1].id.ends_with('\u{FFFD}'), "the cut name keeps its replacement character");
        assert!(rec.analog.iter().all(|c| c.a > 0.0 && c.a < 1e-35), "a tiny scale factor");
        let top = rec.analog[0].samples.iter().fold(0f64, |m, v| m.max(v.abs()));
        assert!(top > 1e-6 && top < 1e6, "samples over the float32 range scaled by a ~1e-37 stay finite and non-zero: {top:e}");
        assert!(rec.analog[0].primary > 0.0 && rec.analog[0].secondary > 0.0);
    }
    assert!(std::fs::read(fixture("a", "a.hdr")).unwrap().len() > 20, "the .hdr is there");
}

/// 1999, 1251, BINARY int16, two rate segments, twenty digital channels (two data words), no `.hdr`.
#[test]
fn cp1251_binary_with_two_rate_segments_and_twenty_digitals() {
    let rec = comtrade::load_with(fixture("b", "b.CFG"), Encoding::Cp1251).unwrap();
    assert_eq!((rec.text_enc, rec.rev_year, rec.format.as_str()), (None, 1999, "BINARY"));
    assert_eq!((rec.analog.len(), rec.digital.len()), (6, 20));
    assert_eq!(rec.rate_segs.len(), 2);
    assert_eq!(rec.rate_segs[1].1, rec.sample_count());
    assert!(rec.rate_segs[0].1 > 0 && rec.rate_segs[0].1 < rec.sample_count());
    // The second segment runs at about half the first one's rate (the source's ratio is kept).
    let (r0, r1) = (rec.rate_segs[0].0, rec.rate_segs[1].0);
    assert!(r0 > 1.8 * r1 && r0 < 2.2 * r1, "{r0} {r1}");
    assert!(rec.analog.iter().all(|c| c.samples.len() == rec.sample_count()));
    assert!(rec.digital.iter().all(|c| c.samples.len() == rec.sample_count()));
    assert!(!fixture("b", "b.HDR").exists() && !fixture("b", "b.hdr").exists());
}

/// 2013 without a BOM, binary32 (int32 samples), a name cut mid-character at a byte limit.
#[test]
fn bomless_utf8_with_a_name_cut_mid_character() {
    let raw = std::fs::read(fixture("c", "c.cfg")).unwrap();
    assert_ne!(&raw[..3], b"\xEF\xBB\xBF");
    assert!(std::str::from_utf8(&raw).is_err(), "the cut leaves invalid UTF-8");
    // Invalid UTF-8 keeps the program encoding in charge (as for any non-UTF-8 file): read as 1251 the cut name is
    // 20 characters (one per byte, nothing replaced); read as UTF-8 it is 11 whole characters and a replacement
    // character for the half one.
    let as_1251 = comtrade::load_with(fixture("c", "c.cfg"), Encoding::Cp1251).unwrap();
    assert_eq!((as_1251.text_enc, as_1251.analog[0].id.chars().count()), (None, 20));
    assert!(!as_1251.analog[0].id.contains('\u{FFFD}'));
    let as_utf8 = comtrade::load_with(fixture("c", "c.cfg"), Encoding::Utf8).unwrap();
    assert_eq!(as_utf8.analog[0].id.chars().count(), 12);
    assert!(as_utf8.analog[0].id.ends_with('\u{FFFD}'), "the half character reads as a replacement character");
    let rec = as_1251;
    assert_eq!((rec.analog.len(), rec.digital.len(), rec.format.as_str()), (3, 2, "binary32"));
    assert_eq!(rec.sample_count(), 96);
    assert!(rec.analog[0].samples.iter().any(|v| *v != 0.0));
    let tc = rec.stamps.time_code.as_str();
    assert!(tc.starts_with('-') && tc.contains('h'), "the time code keeps its shape: {tc}");
    assert_eq!(rec.rev_year, 2013);
}

/// A 1991 layout: no revision token, short analog lines, three-field digital lines, no multiplier.
#[test]
fn revision_1991_layout() {
    let rec = comtrade::load_with(fixture("d", "d.cfg"), Encoding::Cp1251).unwrap();
    assert_eq!((rec.analog.len(), rec.digital.len(), rec.rev_year), (3, 3, 1991));
    assert_eq!(rec.sample_count(), 30);
    assert_eq!(rec.time_mult, 1.0);
}

/// The reader's own writer reproduces the structure it was given (names are invented, kept).
#[test]
fn a_generated_record_survives_a_write_and_read() {
    for (dir, file) in [("a", "a.cfg"), ("b", "b.CFG"), ("c", "c.cfg")] {
        let rec = comtrade::load_with(fixture(dir, file), Encoding::Cp1251).unwrap();
        let (cfg, dat) = comtrade::write(&rec, Encoding::Cp1251);
        let text = comtrade::decode_bytes(&cfg, Encoding::Cp1251);
        let again = comtrade::parse(&text, &dat).unwrap();
        assert_eq!((again.analog.len(), again.digital.len(), again.sample_count()), (rec.analog.len(), rec.digital.len(), rec.sample_count()), "{file}");
    }
}
