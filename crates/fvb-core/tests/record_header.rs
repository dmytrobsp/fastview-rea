//! What the record keeps for the parameters bar and the dialogs: time
//! stamps, the COMTRADE 2013 local code, the time-stamp formatting of
//! `0x456e90` and the code-page tools (ledger L9, UI spec §7.3/§7.9).

use fvb_core::comtrade::{self, detect_codepage, encode_text, format_stamp, raw_stamp, recode, Encoding};

const CFG: &str = include_str!("../../../assets/demo/demo_fault.cfg");
const DAT: &[u8] = include_bytes!("../../../assets/demo/demo_fault.dat");

#[test]
fn loader_keeps_station_device_and_stamps() {
    let rec = comtrade::parse(CFG, DAT).unwrap();
    assert_eq!(rec.device, "BMRZ-demo");
    assert!(rec.station.starts_with("НТЦ"));
    assert_eq!(rec.stamps.start_date, "01/01/2024");
    assert_eq!(rec.stamps.start_time, "00:00:00.000000");
    assert_eq!(rec.stamps.trigger_time, "00:00:00.030000");
    // Only revision 2013 has the time-code line.
    assert_eq!(rec.stamps.local_code, "");
}

/// Primary / secondary / PS are fields 11–13 of an analog line (the
/// coefficient dialog edits them); the writer's lines read back the same.
#[test]
fn analog_line_primary_secondary_ps() {
    let cfg = CFG.replacen("1,Ua,A,,V,1.0,0.0,0.0,-1000000,1000000,1.0,1.0,S", "1,Ua,A,,V,1.0,0.0,0.0,-1000000,1000000,110000,100,P", 1);
    let rec = comtrade::parse(&cfg, DAT).unwrap();
    let ua = &rec.analog[0];
    assert_eq!((ua.primary, ua.secondary, ua.is_primary), (110000.0, 100.0, true));
    assert_eq!((rec.analog[1].primary, rec.analog[1].secondary, rec.analog[1].is_primary), (1.0, 1.0, false));
    let (c, d) = comtrade::write(&rec, comtrade::Encoding::Cp1251);
    let c = comtrade::decode_bytes(&c, comtrade::Encoding::Cp1251);
    let back = comtrade::parse(&c, &d).unwrap();
    assert_eq!((back.analog[0].primary, back.analog[0].secondary, back.analog[0].is_primary), (110000.0, 100.0, true));
    // The tokens go back as read (`0x425530`).
    assert!(c.contains(",110000,100,P\r\n"));
}

#[test]
fn rev_2013_reads_time_code_line_and_writer_keeps_stamps() {
    let cfg = CFG.replace("\r\n", "\n").replacen(",1999", ",2013", 1).replacen("ASCII\n1\n", "ASCII\n1\n+3h30,+3h30\n0,0\n", 1);
    let rec = comtrade::parse(&cfg, DAT).unwrap();
    assert_eq!(rec.stamps.time_code, "+3h30");
    assert_eq!(rec.stamps.local_code, "+3h30");
    let (c, d) = comtrade::write(&rec, comtrade::Encoding::Cp1251);
    let c = comtrade::decode_bytes(&c, comtrade::Encoding::Cp1251);
    assert!(c.contains("01/01/2024,00:00:00.030000\r\n"));
    // The writer keeps the revision as read but never writes the time-code
    // line (`0x44dfa0`).
    assert!(c.lines().next().unwrap().ends_with(",2013"));
    assert!(!c.contains("+3h30"));
    let back = comtrade::parse(&c, &d).unwrap();
    assert_eq!((back.stamps.start_time.as_str(), back.stamps.time_code.as_str()), (rec.stamps.start_time.as_str(), ""));
}

#[test]
fn raw_form_without_local_code() {
    // Pre-2013: first field = month; 1999 "01/02/2024" is 2 January.
    assert_eq!(format_stamp("01/02/2024", "10:20:30.5", 1999, "", true), "02/01/2024, 10:20:30.5");
    // A "month" over 12 swaps; a two-digit year gets +2000.
    assert_eq!(raw_stamp("25/12/24", "t", 1999), "25/12/2024, t");
    // Unparsable date → empty date part.
    assert_eq!(raw_stamp("x", "t", 1999), ", t");
}

#[test]
fn formatted_2013_stamp_utc_and_local() {
    // 2013: first field = day.
    let s = format_stamp("31/12/2023", "23:30:00.012500", 2013, "+3h30", true);
    // _itow of the fraction drops its leading zero, as the original does.
    assert_eq!(s, "31.12.2023, 23:30:00.12500 (UTC)");
    let l = format_stamp("31/12/2023", "23:30:00.012500", 2013, "+3h30", false);
    assert_eq!(l, "01.01.2024, 03:00:00.12500 (Местное время)");
    let m = format_stamp("01/01/2024", "01:00:00.0", 2013, "-5", false);
    assert_eq!(m, "31.12.2023, 20:00:00.0 (Местное время)");
    // "0" or an unsigned code: no offset, still the local suffix.
    assert_eq!(format_stamp("01/01/2024", "01:00:00.0", 2013, "0", false), "01.01.2024, 01:00:00.0 (Местное время)");
    assert_eq!(format_stamp("01/01/2024", "01:00:00.0", 2013, "3", false), "01.01.2024, 01:00:00.0 (Местное время)");
    // Time without a fraction → raw form.
    assert_eq!(format_stamp("01/01/2024", "01:00:00", 2013, "+3", true), "01/01/2024, 01:00:00");
}

#[test]
fn code_page_tables_round_trip_and_recode() {
    for enc in [Encoding::Cp1251, Encoding::Cp866] {
        let all: Vec<u8> = (0u8..=255).collect();
        let text = comtrade::decode_bytes(&all, enc);
        assert_eq!(encode_text(&text, enc), all, "{enc:?}");
    }
    // An 866 name read as 1251 is restored by switching to 866.
    let raw = encode_text("Ток фазы А", Encoding::Cp866);
    let wrong = comtrade::decode_bytes(&raw, Encoding::Cp1251);
    assert_ne!(wrong, "Ток фазы А");
    assert_eq!(recode(&wrong, Encoding::Cp1251, Encoding::Cp866), "Ток фазы А");
}

#[test]
fn code_page_guess_counts_letters() {
    assert_eq!(detect_codepage(&encode_text("Напряжение Ua", Encoding::Cp1251)), Encoding::Cp1251);
    assert_eq!(detect_codepage(&encode_text("Напряжение Ua", Encoding::Cp866)), Encoding::Cp866);
    // A tie (ASCII only) gives 866.
    assert_eq!(detect_codepage(b"Ua Ub"), Encoding::Cp866);
}

#[test]
fn retime_rebuilds_a_uniform_grid() {
    let mut rec = comtrade::parse(CFG, DAT).unwrap();
    rec.retime(4000.0);
    assert_eq!(rec.sample_rate, 4000.0);
    assert!((rec.time[4] - rec.time[0] - 0.001).abs() < 1e-12);
    assert!((rec.duration() - (rec.sample_count() - 1) as f64 / 4000.0).abs() < 1e-9);
}
