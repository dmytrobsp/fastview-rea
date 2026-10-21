//! The original's in-house XML settings tree (`FUN_004aa220` node, reader
//! `0x48e470`, serializer `0x48e1c0` / `0x4aa750`): an element has a tag,
//! attributes kept in a map ordered by name and ordered children. The same
//! tree is used by the `.omp` ОМП settings file and by the per-record
//! settings file (`.set`, see [`crate::recset`]).

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use std::collections::BTreeMap;

/// One element.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Node {
    pub tag: String,
    pub attrs: BTreeMap<String, String>,
    pub children: Vec<Node>,
}

impl Node {
    pub fn new(tag: &str) -> Node {
        Node { tag: tag.into(), ..Default::default() }
    }

    pub fn set(&mut self, k: &str, v: &str) {
        self.attrs.insert(k.into(), v.into());
    }

    /// The attribute, or `default` when absent (`0x48da40`).
    pub fn get(&self, k: &str, default: &str) -> String {
        self.attrs.get(k).cloned().unwrap_or_else(|| default.into())
    }

    /// The first child with this tag.
    pub fn child(&self, tag: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.tag == tag)
    }

    /// The first child with this tag, appended when missing.
    pub fn child_mut(&mut self, tag: &str) -> &mut Node {
        let i = match self.children.iter().position(|c| c.tag == tag) {
            Some(i) => i,
            None => {
                self.children.push(Node::new(tag));
                self.children.len() - 1
            }
        };
        &mut self.children[i]
    }

    /// `0x4aa750`: `<tag a="v" …/>` or `>` + children one tab deeper +
    /// `</tag>`; `& " < >` escaped in values; `\n` line ends.
    pub fn write(&self, indent: &str, out: &mut String) {
        out.push_str(indent);
        out.push('<');
        out.push_str(&self.tag);
        for (k, v) in &self.attrs {
            let v = v.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;").replace('>', "&gt;");
            out.push_str(&format!(" {k}=\"{v}\""));
        }
        if self.children.is_empty() {
            out.push_str("/>\n");
            return;
        }
        out.push_str(">\n");
        let deeper = format!("{indent}\t");
        for c in &self.children {
            c.write(&deeper, out);
        }
        out.push_str(&format!("{indent}</{}>\n", self.tag));
    }

    /// The document text: the declaration, then the root element.
    pub fn to_document(&self) -> String {
        let mut out = String::from("<?xml version=\"1.0\" encoding=\"utf-16\"?>\n");
        self.write("", &mut out);
        out
    }

    /// The document as the original writes the file: a BOM, then UTF-16LE
    /// with every `\n` written as CR LF.
    pub fn to_file_bytes(&self) -> Vec<u8> {
        utf16le_crlf(&self.to_document())
    }

    /// Parse a document's root element (`0x4aa220`: the declaration and
    /// comments skipped; attribute values in double quotes).
    pub fn parse(xml: &str) -> Result<Node, String> {
        fn from_start(e: &BytesStart) -> Result<Node, String> {
            let mut n = Node::new(std::str::from_utf8(e.name().as_ref()).map_err(|e| e.to_string())?);
            for a in e.attributes() {
                let a = a.map_err(|e| e.to_string())?;
                let k = std::str::from_utf8(a.key.as_ref()).map_err(|e| e.to_string())?.to_string();
                let raw = std::str::from_utf8(&a.value).map_err(|e| e.to_string())?;
                n.attrs.insert(k, unescape(raw));
            }
            Ok(n)
        }
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);
        let mut stack = vec![Node::default()];
        loop {
            match reader.read_event().map_err(|e| e.to_string())? {
                Event::Eof => break,
                Event::Start(e) => stack.push(from_start(&e)?),
                Event::Empty(e) => {
                    let n = from_start(&e)?;
                    stack.last_mut().expect("document node").children.push(n);
                }
                Event::End(_) if stack.len() > 1 => {
                    let n = stack.pop().expect("open element");
                    stack.last_mut().expect("parent").children.push(n);
                }
                _ => {}
            }
        }
        while stack.len() > 1 {
            let n = stack.pop().expect("open element");
            stack.last_mut().expect("parent").children.push(n);
        }
        stack.pop().and_then(|d| d.children.into_iter().next()).ok_or_else(|| "empty document".into())
    }

    /// Parse a file's bytes (UTF-16 with BOM or UTF-8).
    pub fn from_file_bytes(bytes: &[u8]) -> Result<Node, String> {
        Node::parse(&crate::decode_fvb(bytes))
    }
}

/// Attribute values are unescaped by four replacements in this order
/// (`0x4aa400`), so `&amp;lt;` comes out as `<`.
pub fn unescape(v: &str) -> String {
    v.replace("&amp;", "&").replace("&quot;", "\"").replace("&lt;", "<").replace("&gt;", ">")
}

/// A wide text file as the original's `_wfopen(…, "w, ccs=UTF-16LE")` writes
/// it: a BOM, then UTF-16LE with every `\n` written as CR LF.
pub fn utf16le_crlf(text: &str) -> Vec<u8> {
    let mut out = vec![0xFF, 0xFE];
    for u in text.replace('\n', "\r\n").encode_utf16() {
        out.extend_from_slice(&u.to_le_bytes());
    }
    out
}
