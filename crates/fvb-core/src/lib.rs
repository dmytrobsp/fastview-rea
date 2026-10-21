//! `fvb-core` — a clean-room reconstruction of the FastView `.fvb`
//! relay-protection characteristic engine.
//!
//! Pipeline: decode bytes -> [`parse::parse_library`] -> [`model::Library`];
//! then for a chosen [`model::Characteristic`] build a [`expr::Context`] from
//! settings and evaluate it into drawable geometry with [`geom::evaluate`].
//!
//! The crate is pure Rust with no platform dependencies, so it builds and runs
//! identically on Linux, Windows and macOS.

pub mod addformula;
pub mod analysis;
pub mod channels;
pub mod clipdata;
pub mod comtrade;
pub mod expr;
pub mod geom;
pub mod merge;
pub mod model;
pub mod omp;
pub mod omp_params;
pub mod parse;
pub mod quick;
pub mod recset;
pub mod settree;
pub mod signal;
pub mod views;

pub use comtrade::{AnalogChannel, DigitalChannel, Oscillogram};
pub use expr::{Context, Expr, ExprError, FormulaError};
pub use geom::{evaluate, evaluate_block, CharGeometry, FigureValues, ModeGeometry, Shape};
pub use model::{
    Block, Characteristic, Combine, Dimension, Library, LineKind, Mode, Parameter, Primitive, Settings, Var,
};
pub use parse::{parse_library, ParseError};

/// Decode a `.fvb` byte stream (UTF-16 LE/BE with BOM, or UTF-8) to a String.
pub fn decode_fvb(bytes: &[u8]) -> String {
    if bytes.len() >= 2 {
        match (bytes[0], bytes[1]) {
            (0xFF, 0xFE) => return decode_utf16(&bytes[2..], false),
            (0xFE, 0xFF) => return decode_utf16(&bytes[2..], true),
            _ => {}
        }
    }
    // Strip a UTF-8 BOM if present.
    let s = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &bytes[3..]
    } else {
        bytes
    };
    String::from_utf8_lossy(s).into_owned()
}

// `chunks_exact(2)` is portable; the suggested `as_chunks` would raise the MSRV.
#[allow(clippy::chunks_exact_to_as_chunks)]
fn decode_utf16(bytes: &[u8], big_endian: bool) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| {
            if big_endian {
                u16::from_be_bytes([c[0], c[1]])
            } else {
                u16::from_le_bytes([c[0], c[1]])
            }
        })
        .collect();
    String::from_utf16_lossy(&units)
}

/// Decode and parse a `.fvb` byte stream into a [`Library`].
pub fn load_fvb_bytes(bytes: &[u8]) -> Result<Library, ParseError> {
    parse_library(&decode_fvb(bytes))
}
