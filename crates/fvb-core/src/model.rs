//! Data model for a FastView `.fvb` characteristic library and evaluation of a
//! characteristic (settings -> concrete geometry) against the expression engine.

use crate::expr::{Context, Expr, FormulaError};
use std::collections::HashMap;

/// The plane a characteristic is drawn in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dimension {
    /// Impedance plane: R on the x-axis, jX on the y-axis (secondary ohms).
    Ohm,
    /// Differential plane: braking current on x, differential current on y.
    Dif,
}

impl Dimension {
    /// `Compare(dimension, L"DIF")` (case-sensitive, `0x4c3c28`): anything but
    /// exactly `DIF` — including `dif` — is the impedance plane.
    pub fn parse(s: &str) -> Dimension {
        if s == "DIF" {
            Dimension::Dif
        } else {
            Dimension::Ohm
        }
    }
    pub fn x_label(self) -> &'static str {
        match self {
            Dimension::Ohm => "R, Ω",
            Dimension::Dif => "I_торм",
        }
    }
    pub fn y_label(self) -> &'static str {
        match self {
            Dimension::Ohm => "jX, Ω",
            Dimension::Dif => "I_диф",
        }
    }
}

/// How a primitive combines into the characteristic region: the CombineRgn
/// mode applied when it is folded into the region built so far (`and`=1,
/// `or`=2, `xor`=3, `not`=4). The first figure of each mode starts the region
/// and its `type` is ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combine {
    /// No `type` attribute. The original leaves the mode uninitialised here
    /// (undefined for a non-first figure; never occurs in the shipped
    /// library); we union.
    Base,
    /// `type="or"`: unioned with the region.
    Or,
    /// `type="and"`: intersected with the region.
    And,
    /// `type="not"`: subtracted from the region (e.g. load-encroachment cutout).
    Not,
    /// `type="xor"`: symmetric difference (accepted by the original's parser).
    Xor,
}

impl Combine {
    /// Exact, case-sensitive compares (`CString::Compare`) in `FUN_0040c7e0`.
    pub fn parse(s: Option<&str>) -> Combine {
        match s {
            Some("and") => Combine::And,
            Some("or") => Combine::Or,
            Some("xor") => Combine::Xor,
            Some("not") => Combine::Not,
            _ => Combine::Base,
        }
    }
}

/// Operate (`on`) vs reset/return (`off`) boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    On,
    Off,
}

impl Mode {
    pub fn parse(s: Option<&str>) -> Mode {
        // FUN_0040c7e0: mode is 1 only for exactly "on" (case-sensitive);
        // anything else, including a missing attribute, is 0 (reset).
        if s == Some("on") {
            Mode::On
        } else {
            Mode::Off
        }
    }
}

/// A bounding line inside a `<polygon>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// `type="1"`: y = k*x + b.
    Slope,
    /// `type="2"`: x = k*y + b (used for near-vertical boundaries).
    InverseSlope,
    /// Any other `_wtoi(type)` value: the original places every vertex that
    /// touches such a line at the origin.
    Invalid,
}

impl LineKind {
    /// `_wtoi(type)`: 1 and 2 are the two line forms, anything else (a
    /// missing attribute is 0) is invalid.
    pub fn parse(s: Option<&str>) -> LineKind {
        match wtoi(s.unwrap_or("")) {
            1 => LineKind::Slope,
            2 => LineKind::InverseSlope,
            _ => LineKind::Invalid,
        }
    }
}

/// CRT `_wtoi`: leading whitespace, optional sign, decimal digits; 0 if none.
fn wtoi(s: &str) -> i64 {
    let t = s.trim_start();
    let (neg, digits) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    let n = digits
        .bytes()
        .take_while(u8::is_ascii_digit)
        .fold(0i64, |acc, d| acc.saturating_mul(10).saturating_add(i64::from(d - b'0')));
    if neg {
        -n
    } else {
        n
    }
}

#[derive(Debug, Clone)]
pub struct PolyLine {
    pub k: Expr,
    pub b: Expr,
    pub kind: LineKind,
}

#[derive(Debug, Clone)]
pub enum Primitive {
    Polygon {
        combine: Combine,
        mode: Mode,
        lines: Vec<PolyLine>,
    },
    PolyPoint {
        combine: Combine,
        mode: Mode,
        closed: bool,
        points: Vec<(Expr, Expr)>,
    },
    Circle {
        combine: Combine,
        mode: Mode,
        radius: Expr,
        xcenter: Expr,
        ycenter: Expr,
    },
    Ellipse {
        combine: Combine,
        mode: Mode,
        xcenter: Expr,
        ycenter: Expr,
        width: Expr,
        height: Expr,
        angle: Expr,
    },
}

impl Primitive {
    pub fn mode(&self) -> Mode {
        match self {
            Primitive::Polygon { mode, .. }
            | Primitive::PolyPoint { mode, .. }
            | Primitive::Circle { mode, .. }
            | Primitive::Ellipse { mode, .. } => *mode,
        }
    }
    pub fn combine(&self) -> Combine {
        match self {
            Primitive::Polygon { combine, .. }
            | Primitive::PolyPoint { combine, .. }
            | Primitive::Circle { combine, .. }
            | Primitive::Ellipse { combine, .. } => *combine,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Parameter {
    pub name: String,
    pub default: Expr,
}

#[derive(Debug, Clone)]
pub struct Var {
    pub name: String,
    pub expr: Expr,
}

#[derive(Debug, Clone)]
pub struct Characteristic {
    pub name: String,
    pub dimension: Dimension,
    /// The `dimension` attribute as written (`char+0x28`): the circle
    /// diagram's scale test compares it with «Ohm» exactly (`0x4387d0`).
    pub dimension_text: String,
    pub parameters: Vec<Parameter>,
    pub vars: Vec<Var>,
    pub primitives: Vec<Primitive>,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub name: String,
    pub characteristics: Vec<Characteristic>,
}

#[derive(Debug, Clone, Default)]
pub struct Library {
    pub blocks: Vec<Block>,
}

/// Parameter values as the settings grid holds them: name -> formula text.
pub type Settings = HashMap<String, String>;

impl Characteristic {
    /// Build the variable map exactly as `FUN_00409260` does for one
    /// characteristic: each parameter's current value text (the settings grid
    /// value, initially the `default` text) is evaluated as a formula against
    /// the parameters before it, then each `<var>` against everything before
    /// it. Names are stored lowercased; a later assignment overwrites. The
    /// first failing formula aborts with its error.
    pub fn context(&self, settings: &Settings) -> Result<Context, FormulaError> {
        let mut ctx = Context::new();
        for p in &self.parameters {
            let v = match settings.get(&p.name) {
                Some(text) => Expr::lenient(text).eval_formula(&ctx)?,
                None => p.default.eval_formula(&ctx)?,
            };
            ctx.insert(p.name.to_lowercase(), v);
        }
        for v in &self.vars {
            let val = v.expr.eval_formula(&ctx)?;
            ctx.insert(v.name.to_lowercase(), val);
        }
        Ok(ctx)
    }

    /// The settings grid's initial values: each parameter's `default` text.
    pub fn default_settings(&self) -> Settings {
        self.parameters
            .iter()
            .map(|p| (p.name.clone(), p.default.source().to_string()))
            .collect()
    }
}

impl Library {
    /// The library one diagram loads: `FUN_0040c7e0` with flag 0 (circle
    /// diagram) keeps characteristics whose dimension is not `DIF`, flag 1
    /// (differential diagram) keeps only `DIF` ones; blocks left without
    /// characteristics are dropped.
    pub fn for_dimension(&self, dim: Dimension) -> Library {
        Library {
            blocks: self
                .blocks
                .iter()
                .filter_map(|b| {
                    let characteristics: Vec<_> = b
                        .characteristics
                        .iter()
                        .filter(|c| c.dimension == dim)
                        .cloned()
                        .collect();
                    (!characteristics.is_empty()).then(|| Block {
                        name: b.name.clone(),
                        characteristics,
                    })
                })
                .collect(),
        }
    }

    /// Append another library's blocks (the original loads every `*.fvb` next
    /// to the executable into one list).
    pub fn extend(&mut self, other: Library) {
        self.blocks.extend(other.blocks);
    }

    pub fn block(&self, name: &str) -> Option<&Block> {
        self.blocks.iter().find(|b| b.name == name)
    }
}
