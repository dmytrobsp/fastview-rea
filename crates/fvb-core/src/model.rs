//! Data model for a FastView `.fvb` characteristic library and evaluation of a
//! characteristic (settings -> concrete geometry) against the expression engine.

use crate::expr::{Context, Expr, ExprError};
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
    pub fn parse(s: &str) -> Dimension {
        match s.trim().to_ascii_uppercase().as_str() {
            "DIF" => Dimension::Dif,
            _ => Dimension::Ohm,
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

/// How a primitive combines into the characteristic region (GDI CombineRgn).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combine {
    /// No `type` attribute: part of the base region (unioned together).
    Base,
    /// `type="or"`: unioned with the region.
    Or,
    /// `type="and"`: intersected with the region.
    And,
    /// `type="not"`: subtracted from the region (e.g. load-encroachment cutout).
    Not,
}

impl Combine {
    pub fn parse(s: Option<&str>) -> Combine {
        match s.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
            Some("or") => Combine::Or,
            Some("and") => Combine::And,
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
        match s.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
            Some("off") => Mode::Off,
            _ => Mode::On,
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
}

impl LineKind {
    pub fn parse(s: Option<&str>) -> LineKind {
        match s.map(|v| v.trim()) {
            Some("2") => LineKind::InverseSlope,
            _ => LineKind::Slope,
        }
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

impl Characteristic {
    /// Build the evaluation context: parameter defaults (optionally overridden
    /// by `settings`) followed by vars in document order. Later assignments to
    /// the same name win, matching the original name-keyed map.
    pub fn context(&self, settings: &HashMap<String, f64>) -> Result<Context, ExprError> {
        // Keys are stored lowercased for case-insensitive resolution (see the
        // note in `Expr::eval`).
        let mut ctx = Context::new();
        for p in &self.parameters {
            let v = match settings.get(&p.name) {
                Some(v) => *v,
                None => p.default.eval(&ctx)?,
            };
            ctx.insert(p.name.to_lowercase(), v);
        }
        for v in &self.vars {
            let val = v.expr.eval(&ctx)?;
            ctx.insert(v.name.to_lowercase(), val);
        }
        Ok(ctx)
    }

    /// Default settings = each parameter's default value.
    pub fn default_settings(&self) -> HashMap<String, f64> {
        let mut m = HashMap::new();
        let mut ctx = Context::new();
        for p in &self.parameters {
            if let Ok(v) = p.default.eval(&ctx) {
                ctx.insert(p.name.clone(), v);
                m.insert(p.name.clone(), v);
            }
        }
        m
    }
}

impl Library {
    pub fn block(&self, name: &str) -> Option<&Block> {
        self.blocks.iter().find(|b| b.name == name)
    }
}
