//! Parse a `.fvb` XML document into the [`Library`] model.
//!
//! The on-disk file is UTF-16 XML; callers should decode to UTF-8 first (the
//! GUI does this via [`crate::load_fvb_bytes`]). This parser accepts UTF-8.

use crate::expr::{Expr, ExprError};
use crate::model::*;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("xml error: {0}")]
    Xml(#[from] quick_xml::Error),
    #[error("xml attr error: {0}")]
    Attr(#[from] quick_xml::events::attributes::AttrError),
    #[error("expression error in attribute {attr:?} ({src:?}): {err}")]
    Expr {
        attr: &'static str,
        src: String,
        err: ExprError,
    },
    #[error("utf8 error: {0}")]
    Utf8(#[from] std::str::Utf8Error),
}

/// Helper to read attributes of an element into a small vector of (name, value).
fn attrs(e: &BytesStart) -> Result<Vec<(String, String)>, ParseError> {
    let mut out = Vec::new();
    for a in e.attributes() {
        let a = a?;
        let key = std::str::from_utf8(a.key.as_ref())?.to_string();
        let val = a.unescape_value()?.into_owned();
        out.push((key, val));
    }
    Ok(out)
}

fn get<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

fn expr_attr(attrs: &[(String, String)], name: &'static str) -> Result<Expr, ParseError> {
    let src = get(attrs, name).unwrap_or("0");
    Expr::parse(src).map_err(|err| ParseError::Expr {
        attr: name,
        src: src.to_string(),
        err,
    })
}

/// Parse a UTF-8 `.fvb` document.
pub fn parse_library(xml: &str) -> Result<Library, ParseError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut lib = Library::default();
    let mut cur_block: Option<Block> = None;
    let mut cur_char: Option<Characteristic> = None;
    // polygon / polypoint being filled
    let mut cur_poly_lines: Vec<PolyLine> = Vec::new();
    let mut cur_points: Vec<(Expr, Expr)> = Vec::new();
    let mut cur_combine = Combine::Base;
    let mut cur_mode = Mode::On;
    let mut cur_closed = false;
    let mut in_polygon = false;
    let mut in_polypoint = false;

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Eof => break,
            Event::Start(e) | Event::Empty(e) => {
                // Leaf tags (parameter, var, line, circle, ellipse) arrive as
                // Empty; container tags (polygon, polypoint, characteristic,
                // block) arrive as Start and are finalised on their End event.
                let tag = e.name();
                let tag = std::str::from_utf8(tag.as_ref())?.to_string();
                let a = attrs(&e)?;
                match tag.as_str() {
                    "blocks" => {}
                    "block" => {
                        cur_block = Some(Block {
                            name: get(&a, "name").unwrap_or("").to_string(),
                            characteristics: Vec::new(),
                        });
                    }
                    "characteristic" => {
                        cur_char = Some(Characteristic {
                            name: get(&a, "name").unwrap_or("").to_string(),
                            dimension: Dimension::parse(get(&a, "dimension").unwrap_or("OHM")),
                            parameters: Vec::new(),
                            vars: Vec::new(),
                            primitives: Vec::new(),
                        });
                    }
                    "parameter" => {
                        if let Some(c) = cur_char.as_mut() {
                            c.parameters.push(Parameter {
                                name: get(&a, "name").unwrap_or("").to_string(),
                                default: expr_attr(&a, "default")?,
                            });
                        }
                    }
                    "var" => {
                        if let Some(c) = cur_char.as_mut() {
                            c.vars.push(Var {
                                name: get(&a, "name").unwrap_or("").to_string(),
                                expr: expr_attr(&a, "expression")?,
                            });
                        }
                    }
                    "polygon" => {
                        in_polygon = true;
                        cur_poly_lines.clear();
                        cur_combine = Combine::parse(get(&a, "type"));
                        cur_mode = Mode::parse(get(&a, "mode"));
                    }
                    "polypoint" => {
                        in_polypoint = true;
                        cur_points.clear();
                        cur_combine = Combine::parse(get(&a, "type"));
                        cur_mode = Mode::parse(get(&a, "mode"));
                        // FUN_0040c7e0: only "off" opens a polypoint; a
                        // missing attribute leaves the constructor default (closed).
                        cur_closed = !matches!(
                            get(&a, "closed").map(|s| s.trim().to_ascii_lowercase()).as_deref(),
                            Some("off")
                        );
                    }
                    "line" => {
                        if in_polygon {
                            cur_poly_lines.push(PolyLine {
                                k: expr_attr(&a, "k")?,
                                b: expr_attr(&a, "b")?,
                                kind: LineKind::parse(get(&a, "type")),
                            });
                        } else if in_polypoint {
                            cur_points.push((expr_attr(&a, "x")?, expr_attr(&a, "y")?));
                        }
                    }
                    "circle" => {
                        if let Some(c) = cur_char.as_mut() {
                            c.primitives.push(Primitive::Circle {
                                combine: Combine::parse(get(&a, "type")),
                                mode: Mode::parse(get(&a, "mode")),
                                radius: expr_attr(&a, "radius")?,
                                xcenter: expr_attr(&a, "xcenter")?,
                                ycenter: expr_attr(&a, "ycenter")?,
                            });
                        }
                    }
                    "ellipse" => {
                        if let Some(c) = cur_char.as_mut() {
                            c.primitives.push(Primitive::Ellipse {
                                combine: Combine::parse(get(&a, "type")),
                                mode: Mode::parse(get(&a, "mode")),
                                xcenter: expr_attr(&a, "xcenter")?,
                                ycenter: expr_attr(&a, "ycenter")?,
                                width: expr_attr(&a, "width")?,
                                height: expr_attr(&a, "height")?,
                                angle: expr_attr(&a, "angle")?,
                            });
                        }
                    }
                    _ => {}
                }
            }
            Event::End(e) => {
                let tag = e.name();
                let tag = std::str::from_utf8(tag.as_ref())?.to_string();
                match tag.as_str() {
                    "polygon" => {
                        if let Some(c) = cur_char.as_mut() {
                            c.primitives.push(Primitive::Polygon {
                                combine: cur_combine,
                                mode: cur_mode,
                                lines: std::mem::take(&mut cur_poly_lines),
                            });
                        }
                        in_polygon = false;
                    }
                    "polypoint" => {
                        if let Some(c) = cur_char.as_mut() {
                            c.primitives.push(Primitive::PolyPoint {
                                combine: cur_combine,
                                mode: cur_mode,
                                closed: cur_closed,
                                points: std::mem::take(&mut cur_points),
                            });
                        }
                        in_polypoint = false;
                    }
                    "characteristic" => {
                        if let (Some(b), Some(ch)) = (cur_block.as_mut(), cur_char.take()) {
                            b.characteristics.push(ch);
                        }
                    }
                    "block" => {
                        if let Some(b) = cur_block.take() {
                            lib.blocks.push(b);
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(lib)
}
