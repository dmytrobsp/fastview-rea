//! Parse a `.fvb` XML document into the [`Library`] model.
//!
//! Mirrors the original loader `FUN_0040c7e0` (ledger L5): the document is read
//! into a node tree and walked **by structure** — the root's children are
//! blocks, their children are characteristics, and only the characteristic's
//! children are dispatched by tag (`parameter var polygon polypoint circle
//! ellipse`; anything else is ignored). The children of a polygon/polypoint are
//! its lines whatever their tag. Attribute names and values compare exactly
//! (case-sensitive); a missing attribute is an empty string. Expression
//! attributes are stored as text — an empty or malformed one only fails when
//! the characteristic is evaluated.
//!
//! The on-disk file is UTF-16 XML; callers should decode to UTF-8 first (the
//! GUI does this via [`crate::load_fvb_bytes`]). This parser accepts UTF-8.

use crate::expr::Expr;
use crate::model::*;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("xml error: {0}")]
    Xml(#[from] quick_xml::Error),
    #[error("xml attr error: {0}")]
    Attr(#[from] quick_xml::events::attributes::AttrError),
    #[error("utf8 error: {0}")]
    Utf8(#[from] std::str::Utf8Error),
}

/// One element of the document tree.
#[derive(Debug, Default)]
struct Node {
    tag: String,
    attrs: Vec<(String, String)>,
    children: Vec<Node>,
}

impl Node {
    fn from_start(e: &BytesStart) -> Result<Node, ParseError> {
        let mut attrs = Vec::new();
        for a in e.attributes() {
            let a = a?;
            let key = std::str::from_utf8(a.key.as_ref())?.to_string();
            let val = a.unescape_value()?.into_owned();
            attrs.push((key, val));
        }
        Ok(Node {
            tag: std::str::from_utf8(e.name().as_ref())?.to_string(),
            attrs,
            children: Vec::new(),
        })
    }

    /// `FUN_00408ab0`: exact-key lookup, empty string when absent.
    fn attr(&self, name: &str) -> &str {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
            .unwrap_or("")
    }

    fn opt_attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    fn expr(&self, name: &str) -> Expr {
        Expr::lenient(self.attr(name))
    }
}

/// Read the whole document into a tree; returns the root element.
fn read_tree(xml: &str) -> Result<Node, ParseError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut stack: Vec<Node> = vec![Node::default()];
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Eof => break,
            Event::Start(e) => stack.push(Node::from_start(&e)?),
            Event::Empty(e) => {
                let n = Node::from_start(&e)?;
                stack.last_mut().expect("document node").children.push(n);
            }
            Event::End(_) if stack.len() > 1 => {
                let n = stack.pop().expect("open element");
                stack.last_mut().expect("parent").children.push(n);
            }
            _ => {}
        }
        buf.clear();
    }
    while stack.len() > 1 {
        let n = stack.pop().expect("open element");
        stack.last_mut().expect("parent").children.push(n);
    }
    let doc = stack.pop().expect("document node");
    Ok(doc.children.into_iter().next().unwrap_or_default())
}

fn characteristic(node: &Node) -> Characteristic {
    let mut ch = Characteristic {
        name: node.attr("name").to_string(),
        dimension: Dimension::parse(node.attr("dimension")),
        dimension_text: node.attr("dimension").to_string(),
        parameters: Vec::new(),
        vars: Vec::new(),
        primitives: Vec::new(),
    };
    for c in &node.children {
        let combine = Combine::parse(c.opt_attr("type"));
        let mode = Mode::parse(c.opt_attr("mode"));
        match c.tag.as_str() {
            "parameter" => ch.parameters.push(Parameter {
                name: c.attr("name").to_string(),
                default: c.expr("default"),
            }),
            "var" => ch.vars.push(Var {
                name: c.attr("name").to_string(),
                expr: c.expr("expression"),
            }),
            "polygon" => ch.primitives.push(Primitive::Polygon {
                combine,
                mode,
                lines: c
                    .children
                    .iter()
                    .map(|l| PolyLine {
                        k: l.expr("k"),
                        b: l.expr("b"),
                        kind: LineKind::parse(l.opt_attr("type")),
                    })
                    .collect(),
            }),
            "polypoint" => ch.primitives.push(Primitive::PolyPoint {
                combine,
                mode,
                // Only exactly "off" opens a polypoint; missing = closed.
                closed: c.opt_attr("closed") != Some("off"),
                points: c.children.iter().map(|l| (l.expr("x"), l.expr("y"))).collect(),
            }),
            "circle" => ch.primitives.push(Primitive::Circle {
                combine,
                mode,
                radius: c.expr("radius"),
                xcenter: c.expr("xcenter"),
                ycenter: c.expr("ycenter"),
            }),
            "ellipse" => ch.primitives.push(Primitive::Ellipse {
                combine,
                mode,
                xcenter: c.expr("xcenter"),
                ycenter: c.expr("ycenter"),
                width: c.expr("width"),
                height: c.expr("height"),
                angle: c.expr("angle"),
            }),
            _ => {}
        }
    }
    ch
}

/// Parse a UTF-8 `.fvb` document.
pub fn parse_library(xml: &str) -> Result<Library, ParseError> {
    let root = read_tree(xml)?;
    let blocks = root
        .children
        .iter()
        .map(|b| Block {
            name: b.attr("name").to_string(),
            characteristics: b.children.iter().map(characteristic).collect(),
        })
        .collect();
    Ok(Library { blocks })
}
