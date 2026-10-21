//! Expression evaluator for the FastView formula language (`Calculator`).
//!
//! Reconstructed from the decompiled original (`FastView.exe`, see
//! `notes/RECONSTRUCTION_LEDGER.md` L1/L2), not inferred from the data:
//!
//! * **Tokenizer** (`FUN_00418c40`): the formula is lowercased (text inside
//!   `[...]` keeps its case), only U+0020 is skipped, and at each position the
//!   static token table (`FUN_00401000`, 70 entries) is tried *in table order*
//!   as a plain prefix match — so `pi` splits `pid` into `pi` + `d`. Numbers are
//!   digits and `.` (no exponent, no leading `.`, at most one `.`); identifiers
//!   start with a letter, `_` or `"` (quoted names run to the next `"`); `[...]`
//!   is a DIMENSION token, ignored after an atom. A `,` followed by `,` or `)`
//!   gets a `null` value inserted. Unknown identifiers are an error even when
//!   they sit in a part of the formula the parser later ignores.
//! * **Grammar** (`0x419af0` .. `0x41f960`), recursive descent that evaluates
//!   directly:
//!
//! ```text
//! list  := expr (',' expr)*
//! expr  := term (('+' | '-') term)*                 left-assoc
//! term  := pow (('*' | '/') unary ['^' unary])*     left-assoc
//! pow   := unary ['^' unary]                        ONE '^' only, no chaining
//! unary := ['+' | '-'] primary                      one sign only
//! primary := func group | group | atom
//! group := '(' list <any token> | atom              the ')' is never checked
//! ```
//!
//!   Consequences pinned by tests: `-2^2 = 4`, `2^3^2 = 8` (the rest of the
//!   formula after the first `^`-chain is dropped), `--2` is an error, trailing
//!   tokens are ignored, `sin:30` (bare atom argument) is legal.
//! * **Values** carry a scalar and a complex pair (`+0x18`, `+8/+0x10` of the
//!   0x28-byte operand). Numbers/variables set both (`re = v, im = 0`);
//!   `+ - * /` update both; `sin cos tg ctg sqrt exp ^ re im` write only the
//!   scalar, `mod:` reads only the complex pair. The formula's result is the
//!   scalar of its single top-level value.
//! * Trig arguments are **degrees**, converted as `v * pi / 180` (in that
//!   order); `ctg = 1 / tan`; there are no domain checks (IEEE inf/NaN flow
//!   through); `^` needs an integer exponent, `n = 0 -> 1`, `n > 0` is repeated
//!   multiplication and `n < 0` returns the base unchanged (original bug).
//!
//! Variables are resolved by name through a [`Context`] map, mirroring the
//! name-keyed map lookup in the original binary.

use num_complex::Complex64;
use std::collections::HashMap;
use std::f64::consts::PI;

/// The oscillogram side of the formula language's channel mode (the
/// original's `CalcChannels`, vtable `0x4c65c4`): channel lookup by name and
/// the per-channel methods the channel functions call (ledger L6).
pub trait Channels {
    /// Resolve an identifier to a channel (vtable slot 0); `None` makes the
    /// tokenizer drop the identifier.
    fn resolve(&self, name: &str) -> Option<usize>;
    /// Instantaneous value at `idx` (`FUN_00426020`).
    fn sample(&self, ch: usize, idx: usize) -> f64;
    /// RMS fundamental phasor at `idx` (`FUN_00428ac0`).
    fn phasor(&self, ch: usize, idx: usize) -> Complex64;
    /// True RMS over one period ending at `idx` (`FUN_004272a0(idx, 1)`).
    fn rms(&self, ch: usize, idx: usize) -> f64;
    /// Mean over one period ending at `idx` (`FUN_00426c90(idx, 1)`).
    fn a0(&self, ch: usize, idx: usize) -> f64;
    /// RMS magnitude of harmonic `n` at `idx` (`FUN_00426240`).
    fn fourie(&self, ch: usize, n: i32, idx: usize) -> f64;
    /// Fault (emergency) component at `idx` (`FUN_00427670`).
    fn fault(&self, ch: usize, idx: usize) -> f64;
    /// Frequency at `idx` (vtable slot 1: the nominal frequency).
    fn frequency(&self, idx: usize) -> f64;
    /// Samples before `arg:` is valid (vtable slot 2: one period).
    fn warmup(&self) -> usize;
}

/// Evaluation environment: the variable map, and in channel mode the channel
/// source and the current sample index.
#[derive(Clone, Copy)]
struct Env<'a> {
    ctx: &'a Context,
    chans: Option<&'a dyn Channels>,
    idx: usize,
}

/// Variable/parameter environment: name -> value.
pub type Context = HashMap<String, f64>;

/// Evaluation errors. The messages are the original's string-table entries
/// (`0x157`, `0x42f`, `0x430`, `0x431`); the original shows them as
/// `"<formula>: <message>"` (see [`FormulaError`]).
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ExprError {
    #[error("Синтаксическая ошибка в формуле")]
    UnexpectedEof,
    #[error("Синтаксическая ошибка в формуле")]
    UnexpectedChar(char, usize),
    #[error("Синтаксическая ошибка в формуле")]
    Expected(&'static str),
    #[error("Синтаксическая ошибка в формуле")]
    UnknownFunction(String),
    #[error("Переменная не найдена")]
    UnknownIdent(String),
    #[error("Синтаксическая ошибка в формуле")]
    Trailing,
    #[error("Неправильное число переменных")]
    ArgCount(&'static str, usize, usize),
    #[error("Синтаксическая ошибка в формуле")]
    NonIntegerPower,
    #[error("Аргумент функции должен быть каналом")]
    NeedsChannel(&'static str),
}

/// An [`ExprError`] together with the formula text it came from, displayed
/// as the original's message box does.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{formula}: {error}")]
pub struct FormulaError {
    pub formula: String,
    pub error: ExprError,
}

/// Formula functions (token ids 0x08..0x3f of the original table). Only the
/// ones that are pure math are evaluated here; the channel/analysis accessors
/// are recognised (so tokenizing and argument counts match) but evaluating them
/// without an oscillogram channel is an error, as in the original (`0x431`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Func {
    Sin,
    Cos,
    Tg,
    Ctg,
    Sqrt,
    Exp,
    Re,
    Im,
    Mod,
    Arg,
    Cosui,
    Absig,
    /// A channel/analysis accessor (`rms: fourie: ua: zf: …`).
    Channel(&'static str),
}

/// Token table in the original's array order (`FUN_00401000`): name, id.
/// Ids 0x1e/0x1f sit after 0x22 in the array; prefix matching follows this
/// order. Names without a trailing `:` (`pi`, `null`) match as bare prefixes.
const TABLE: &[(&str, u8)] = &[
    (",", 0x00),
    ("+", 0x01),
    ("-", 0x02),
    ("*", 0x03),
    ("/", 0x04),
    ("^", 0x05),
    ("(", 0x06),
    (")", 0x07),
    ("sin:", 0x08),
    ("cos:", 0x09),
    ("tg:", 0x0a),
    ("ctg:", 0x0b),
    ("pi", 0x0c),
    ("sqrt:", 0x0d),
    ("exp:", 0x0e),
    ("re:", 0x0f),
    ("im:", 0x10),
    ("mod:", 0x11),
    ("arg:", 0x12),
    ("cosui:", 0x13),
    ("absig:", 0x14),
    ("rms:", 0x15),
    ("fourie:", 0x16),
    ("a0:", 0x17),
    ("fault:", 0x18),
    ("frequency:", 0x19),
    ("ib:", 0x1a),
    ("3i0:", 0x1b),
    ("i1:", 0x1c),
    ("i2:", 0x1d),
    ("ua:", 0x20),
    ("ub:", 0x21),
    ("uc:", 0x22),
    ("ulf:", 0x1e),
    ("ull:", 0x1f),
    ("3u0:", 0x23),
    ("u1f:", 0x24),
    ("u2f:", 0x25),
    ("u1l:", 0x26),
    ("u2l:", 0x27),
    ("uncn:", 0x28),
    ("pk:", 0x29),
    ("pf:", 0x2a),
    ("pf3wm:", 0x2b),
    ("pf2wm:", 0x2c),
    ("zf:", 0x2d),
    ("zk:", 0x2e),
    ("zl:", 0x2f),
    ("coeffpriv:", 0x30),
    ("coefftt:", 0x31),
    ("coeffpowerpriv:", 0x32),
    ("coeffamppriv:", 0x33),
    ("grsposlia:", 0x34),
    ("grsposlib:", 0x35),
    ("grsposlic:", 0x36),
    ("grstriangleia:", 0x37),
    ("grstriangleib:", 0x38),
    ("grstriangleic:", 0x39),
    ("grssumcur:", 0x3a),
    ("tormsumcur:", 0x3b),
    ("tormsubcur:", 0x3c),
    ("tormmaxcur:", 0x3d),
    ("chan:", 0x3e),
    ("uniik:", 0x3f),
    ("null", 0x40),
];

impl Func {
    /// Function for a table id, with its exact argument count (checked by
    /// `FUN_0041fd00`; `None` = not checked, as for `frequency:`).
    fn from_id(id: u8) -> Option<(Func, &'static str, Option<usize>)> {
        let name = TABLE.iter().find(|(_, i)| *i == id)?.0;
        let (f, n) = match id {
            0x08 => (Func::Sin, Some(1)),
            0x09 => (Func::Cos, Some(1)),
            0x0a => (Func::Tg, Some(1)),
            0x0b => (Func::Ctg, Some(1)),
            0x0d => (Func::Sqrt, Some(1)),
            0x0e => (Func::Exp, Some(1)),
            0x0f => (Func::Re, Some(1)),
            0x10 => (Func::Im, Some(1)),
            0x11 => (Func::Mod, Some(1)),
            0x12 => (Func::Arg, Some(1)),
            0x13 => (Func::Cosui, Some(2)),
            0x14 => (Func::Absig, Some(2)),
            0x15 | 0x17 | 0x18 | 0x29 | 0x3e => (Func::Channel(name), Some(1)),
            0x16 | 0x1e | 0x1f | 0x26 | 0x27 | 0x2a | 0x30 | 0x3f => (Func::Channel(name), Some(2)),
            0x19 => (Func::Channel(name), None),
            0x1a..=0x1d | 0x23..=0x25 => (Func::Channel(name), Some(3)),
            0x31 | 0x33 | 0x3c => (Func::Channel(name), Some(3)),
            0x2c | 0x2f | 0x32 | 0x37..=0x39 => (Func::Channel(name), Some(4)),
            0x20..=0x22 | 0x28 | 0x34..=0x36 => (Func::Channel(name), Some(5)),
            0x2b => (Func::Channel(name), Some(6)),
            0x2e => (Func::Channel(name), Some(9)),
            0x2d => (Func::Channel(name), Some(11)),
            0x3a | 0x3d => (Func::Channel(name), Some(24)),
            0x3b => (Func::Channel(name), Some(25)),
            _ => return None,
        };
        Some((f, name, n))
    }
}

/// One operand: the scalar slot and the complex pair, updated independently
/// exactly as in the original's 0x28-byte value element.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Value {
    v: f64,
    re: f64,
    im: f64,
    /// Flag bit 2 ("channel reference"); arithmetic keeps it when both sides
    /// carry it, but only a bare channel atom keeps the channel itself.
    chan: bool,
    ch: Option<usize>,
}

impl Value {
    fn real(v: f64) -> Value {
        Value { v, re: v, im: 0.0, chan: false, ch: None }
    }
    /// A function result: scalar written, complex pair left as it was.
    fn scalar(self, v: f64) -> Value {
        Value { v, chan: false, ch: None, ..self }
    }
    /// A function result with both slots written (flags 3).
    fn both(v: f64, z: Complex64) -> Value {
        Value { v, re: z.re, im: z.im, chan: false, ch: None }
    }
    fn z(self) -> Complex64 {
        Complex64::new(self.re, self.im)
    }
}

/// Lexical token.
#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Op(u8),
    Func(u8),
    Pi,
    Null,
    Num(f64),
    Ident(String),
    Chan(usize),
    Dimension,
    End,
}

/// Parsed node. Lists (`(a, b)`) are kept because the original's operands are
/// vectors: operators act on the first element of each side and keep the
/// left side's length.
#[derive(Debug, Clone, PartialEq)]
enum Node {
    Num(f64),
    Var(String),
    Chan(usize),
    Pi,
    Null,
    Neg(Box<Node>),
    Bin(u8, Box<Node>, Box<Node>),
    Call(Func, &'static str, Option<usize>, Vec<Node>),
    List(Vec<Node>),
}

/// A compiled formula. Parse once, evaluate many times against different
/// [`Context`]s (settings change, geometry re-evaluates).
#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    src: String,
    /// The parse result; a formula that does not parse is kept and reports
    /// its error when evaluated, as the original (which stores the text and
    /// tokenizes on evaluation) does.
    root: Result<Box<Node>, ExprError>,
    /// Every identifier the tokenizer saw, including ones in ignored trailing
    /// input — the original resolves them all while tokenizing.
    idents: Vec<String>,
}

impl Expr {
    /// Parse a formula string, failing on a syntax error.
    pub fn parse(src: &str) -> Result<Expr, ExprError> {
        let e = Expr::lenient(src);
        match &e.root {
            Ok(_) => Ok(e),
            Err(err) => Err(err.clone()),
        }
    }

    /// Keep a formula string whatever it contains; a syntax error (an empty
    /// attribute included) surfaces when it is evaluated.
    pub fn lenient(src: &str) -> Expr {
        Expr::compile(src, None)
    }

    /// Parse a formula in channel mode (the formula editor's computed
    /// channels): identifiers are resolved to channels by `chans`, and an
    /// identifier that resolves to nothing is dropped from the token stream,
    /// as the original's tokenizer does.
    pub fn parse_channels(src: &str, chans: &dyn Channels) -> Result<Expr, ExprError> {
        let e = Expr::compile(src, Some(chans));
        match &e.root {
            Ok(_) => Ok(e),
            Err(err) => Err(err.clone()),
        }
    }

    fn compile(src: &str, chans: Option<&dyn Channels>) -> Expr {
        let parsed = tokenize(src, chans).and_then(|(toks, idents)| {
            let mut p = Parser { toks, pos: 0 };
            Ok((Box::new(Node::List(p.list()?)), idents))
        });
        let (root, idents) = match parsed {
            Ok((root, idents)) => (Ok(root), idents),
            Err(e) => (Err(e), Vec::new()),
        };
        Expr {
            src: src.to_string(),
            root,
            idents,
        }
    }

    /// The formula text as written.
    pub fn source(&self) -> &str {
        &self.src
    }

    /// Evaluate against a [`Context`]; returns the scalar of the single
    /// top-level value. Identifier resolution is case-insensitive (keys are
    /// stored lowercased): the original lowercases the formula before lookup,
    /// and the shipped library relies on it (`ДЗМф2_Rср` vs `ДЗМФ2_Rср`).
    pub fn eval(&self, ctx: &Context) -> Result<f64, ExprError> {
        self.eval_env(Env { ctx, chans: None, idx: 0 })
    }

    /// Evaluate a channel-mode formula at sample `idx`.
    pub fn eval_at(&self, ctx: &Context, chans: &dyn Channels, idx: usize) -> Result<f64, ExprError> {
        self.eval_env(Env { ctx, chans: Some(chans), idx })
    }

    fn eval_env(&self, env: Env) -> Result<f64, ExprError> {
        let ctx = env.ctx;
        let root = self.root.as_ref().map_err(Clone::clone)?;
        for id in &self.idents {
            if !ctx.contains_key(id) {
                return Err(ExprError::UnknownIdent(id.clone()));
            }
        }
        let vals = eval_node(root, env)?;
        if vals.len() != 1 {
            return Err(ExprError::Expected("a single value"));
        }
        Ok(vals[0].v)
    }

    /// [`Expr::eval`] with the formula text attached to the error.
    pub fn eval_formula(&self, ctx: &Context) -> Result<f64, FormulaError> {
        self.eval(ctx).map_err(|error| FormulaError {
            formula: self.src.clone(),
            error,
        })
    }
}

/// Convenience: parse and evaluate in one step.
pub fn eval_str(src: &str, ctx: &Context) -> Result<f64, ExprError> {
    Expr::parse(src)?.eval(ctx)
}

fn tokenize(src: &str, chans: Option<&dyn Channels>) -> Result<(Vec<Tok>, Vec<String>), ExprError> {
    // Lowercase outside [...] (`0x418ca4`..`0x418cee` restore bracketed text).
    let mut s = String::with_capacity(src.len());
    let mut in_dim = false;
    for c in src.chars() {
        match c {
            '[' => in_dim = true,
            ']' => in_dim = false,
            _ => {}
        }
        if in_dim {
            s.push(c);
        } else {
            s.extend(c.to_lowercase());
        }
    }

    let mut toks = Vec::new();
    let mut idents = Vec::new();
    let mut pos = 0;
    while pos < s.len() {
        let rest = &s[pos..];
        let c = rest.chars().next().unwrap();
        if c == ' ' {
            pos += 1;
            continue;
        }
        if let Some((name, id)) = TABLE.iter().find(|(n, _)| rest.starts_with(n)) {
            pos += name.len();
            toks.push(match id {
                0x00..=0x07 => Tok::Op(*id),
                0x0c => Tok::Pi,
                0x40 => Tok::Null,
                _ => Tok::Func(*id),
            });
            continue;
        }
        if c.is_ascii_digit() {
            let len = rest
                .find(|ch: char| !(ch.is_ascii_digit() || ch == '.'))
                .unwrap_or(rest.len());
            let text = &rest[..len];
            if text.matches('.').count() > 1 {
                return Err(ExprError::Expected("number"));
            }
            toks.push(Tok::Num(wtof(text)));
            pos += len;
            continue;
        }
        if c == '"' {
            let body = &rest[1..];
            let end = body.find('"').ok_or(ExprError::Expected("closing \""))?;
            ident(&body[..end], chans, &mut toks, &mut idents);
            pos += end + 2;
            continue;
        }
        if c == '_' || c.is_alphabetic() {
            let len = rest
                .find(|ch: char| !(ch == '_' || ch.is_alphanumeric()))
                .unwrap_or(rest.len());
            ident(&rest[..len], chans, &mut toks, &mut idents);
            pos += len;
            continue;
        }
        if c == '[' {
            let len = rest.find(']').map(|i| i + 1).unwrap_or(rest.len());
            toks.push(Tok::Dimension);
            pos += len;
            continue;
        }
        return Err(ExprError::UnexpectedChar(c, pos));
    }

    // Post-pass: `,` followed by `,` or `)` gets a null value in between.
    let mut out = Vec::with_capacity(toks.len() + 1);
    for (i, t) in toks.iter().enumerate() {
        out.push(t.clone());
        if *t == Tok::Op(0x00) && matches!(toks.get(i + 1), Some(Tok::Op(0x00 | 0x07))) {
            out.push(Tok::Null);
        }
    }
    out.push(Tok::End);
    Ok((out, idents))
}

/// An identifier: a variable, or in channel mode a channel (`0x419126`).
fn ident(name: &str, chans: Option<&dyn Channels>, toks: &mut Vec<Tok>, idents: &mut Vec<String>) {
    match chans {
        None => {
            idents.push(name.to_string());
            toks.push(Tok::Ident(name.to_string()));
        }
        Some(c) => {
            if let Some(ch) = c.resolve(name) {
                toks.push(Tok::Chan(ch));
            }
        }
    }
}

/// `_wtof` over a digits-and-one-dot string (`"1."` is 1).
fn wtof(text: &str) -> f64 {
    text.trim_end_matches('.').parse().unwrap_or(0.0)
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> &Tok {
        &self.toks[self.pos]
    }

    fn advance(&mut self) {
        if self.pos + 1 < self.toks.len() {
            self.pos += 1;
        }
    }

    fn peek_op(&self, op: u8) -> bool {
        *self.peek() == Tok::Op(op)
    }

    fn list(&mut self) -> Result<Vec<Node>, ExprError> {
        let mut items = vec![self.expr()?];
        while self.peek_op(0x00) {
            self.advance();
            items.push(self.expr()?);
        }
        Ok(items)
    }

    fn expr(&mut self) -> Result<Node, ExprError> {
        let mut lhs = self.term()?;
        while let Tok::Op(op @ (0x01 | 0x02)) = *self.peek() {
            self.advance();
            let rhs = self.term()?;
            lhs = Node::Bin(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn term(&mut self) -> Result<Node, ExprError> {
        let mut lhs = self.pow()?;
        while let Tok::Op(op @ (0x03 | 0x04)) = *self.peek() {
            self.advance();
            let rhs = self.pow()?;
            lhs = Node::Bin(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn pow(&mut self) -> Result<Node, ExprError> {
        let base = self.unary()?;
        if self.peek_op(0x05) {
            self.advance();
            let exp = self.unary()?;
            return Ok(Node::Bin(0x05, Box::new(base), Box::new(exp)));
        }
        Ok(base)
    }

    fn unary(&mut self) -> Result<Node, ExprError> {
        let sign = match *self.peek() {
            Tok::Op(op @ (0x01 | 0x02)) => {
                self.advance();
                op
            }
            _ => 0x01,
        };
        let p = self.primary()?;
        Ok(if sign == 0x02 { Node::Neg(Box::new(p)) } else { p })
    }

    fn primary(&mut self) -> Result<Node, ExprError> {
        if let Tok::Func(id) = *self.peek() {
            self.advance();
            let args = self.group()?;
            let (f, name, n) =
                Func::from_id(id).ok_or_else(|| ExprError::UnknownFunction(format!("{id:#x}")))?;
            return Ok(Node::Call(f, name, n, args));
        }
        if self.peek_op(0x06) {
            return Ok(Node::List(self.group()?));
        }
        self.atom()
    }

    /// `0x41f390`: `'(' list` then one token consumed as the `)` without
    /// checking it, or a single bare atom. `()` leaves the `)` in place.
    fn group(&mut self) -> Result<Vec<Node>, ExprError> {
        if !self.peek_op(0x06) {
            return Ok(vec![self.atom()?]);
        }
        self.advance();
        if self.peek_op(0x07) {
            return Ok(Vec::new());
        }
        let items = self.list()?;
        self.advance();
        Ok(items)
    }

    /// `0x41f960`: number, variable, `pi`, `null`; a following DIMENSION
    /// (`[...]` unit text) is skipped.
    fn atom(&mut self) -> Result<Node, ExprError> {
        let n = match self.peek().clone() {
            Tok::Num(v) => Node::Num(v),
            Tok::Ident(name) => Node::Var(name),
            Tok::Chan(ch) => Node::Chan(ch),
            Tok::Pi => Node::Pi,
            Tok::Null => Node::Null,
            Tok::End => return Err(ExprError::UnexpectedEof),
            _ => return Err(ExprError::Expected("operand")),
        };
        self.advance();
        if *self.peek() == Tok::Dimension {
            self.advance();
        }
        Ok(n)
    }
}

fn first(vals: &[Value]) -> Result<Value, ExprError> {
    vals.first().copied().ok_or(ExprError::Expected("operand"))
}

/// Degree argument converted exactly as the original: `v * pi / 180`.
fn rad(v: f64) -> f64 {
    v * PI / 180.0
}

fn eval_node(n: &Node, env: Env) -> Result<Vec<Value>, ExprError> {
    Ok(match n {
        Node::Num(v) => vec![Value::real(*v)],
        Node::Var(name) => {
            let v = *env
                .ctx
                .get(name)
                .ok_or_else(|| ExprError::UnknownIdent(name.clone()))?;
            vec![Value::real(v)]
        }
        Node::Chan(ch) => {
            // 0x41f960: scalar = instantaneous value, complex = phasor, flags 7.
            let c = env.chans.ok_or(ExprError::NeedsChannel("chan"))?;
            let z = c.phasor(*ch, env.idx);
            vec![Value {
                v: c.sample(*ch, env.idx),
                re: z.re,
                im: z.im,
                chan: true,
                ch: Some(*ch),
            }]
        }
        Node::Pi => vec![Value::real(PI)],
        Node::Null => vec![Value { v: 0.0, re: 0.0, im: 0.0, chan: false, ch: None }],
        Node::List(items) => {
            let mut out = Vec::new();
            for it in items {
                out.extend(eval_node(it, env)?);
            }
            out
        }
        Node::Neg(a) => {
            // 0x419fc0: negate all three slots; flags kept, channel dropped.
            let mut vals = eval_node(a, env)?;
            let x = first(&vals)?;
            vals[0] = Value { v: -x.v, re: -x.re, im: -x.im, ch: None, ..x };
            vals
        }
        Node::Bin(op, a, b) => {
            let mut l = eval_node(a, env)?;
            let x = first(&l)?;
            let y = first(&eval_node(b, env)?)?;
            l[0] = binary(*op, x, y)?;
            l
        }
        Node::Call(f, name, n, args) => {
            let mut vals = Vec::new();
            for a in args {
                vals.extend(eval_node(a, env)?);
            }
            if let Some(n) = n {
                if vals.len() != *n {
                    return Err(ExprError::ArgCount(name, *n, vals.len()));
                }
            }
            vec![call(*f, name, &vals, env)?]
        }
    })
}

/// Binary operator handlers (jump table `0x41f944`): flags = lhs & rhs, the
/// channel pointer is cleared.
fn binary(op: u8, x: Value, y: Value) -> Result<Value, ExprError> {
    let chan = x.chan && y.chan;
    Ok(match op {
        0x01 => Value { v: x.v + y.v, re: x.re + y.re, im: x.im + y.im, chan, ch: None },
        0x02 => Value { v: x.v - y.v, re: x.re - y.re, im: x.im - y.im, chan, ch: None },
        0x03 => Value {
            v: x.v * y.v,
            re: x.re * y.re - x.im * y.im,
            im: x.re * y.im + x.im * y.re,
            chan,
            ch: None,
        },
        0x04 => {
            let (re, im) = complex_div(x.re, x.im, y.re, y.im);
            Value { v: x.v / y.v, re, im, chan, ch: None }
        }
        _ => {
            // 0x41f7cb: integer exponent only; scalar only.
            let n = y.v as i32;
            if f64::from(n) != y.v {
                return Err(ExprError::NonIntegerPower);
            }
            let mut acc = if n == 0 { 1.0 } else { x.v };
            for _ in 1..n.max(1) {
                acc *= x.v;
            }
            x.scalar(acc)
        }
    })
}

/// `FUN_00420ca0`: complex division (Smith's scaled form, as MSVC `<complex>`).
fn complex_div(ar: f64, ai: f64, br: f64, bi: f64) -> (f64, f64) {
    if br.abs() < bi.abs() {
        let w = br / bi;
        let d = bi + w * br;
        ((ar * w + ai) / d, (ai * w - ar) / d)
    } else {
        let w = bi / br;
        let d = br + w * bi;
        ((ar + ai * w) / d, (ai - ar * w) / d)
    }
}

/// A channel argument: the value must carry the channel flag (else error
/// 0x431). A flagged value without a channel (e.g. `rms:(a+b)`) dereferences
/// a null channel in the original; we report the same error.
fn channel_arg<'a>(x: Value, name: &'static str, env: Env<'a>) -> Result<(&'a dyn Channels, usize), ExprError> {
    match (x.chan, x.ch, env.chans) {
        (true, Some(ch), Some(c)) => Ok((c, ch)),
        _ => Err(ExprError::NeedsChannel(name)),
    }
}

/// Function cases (jump table `0x41f2a4`, index = token id − 8).
fn call(f: Func, name: &'static str, a: &[Value], env: Env) -> Result<Value, ExprError> {
    let x = first(a)?;
    Ok(match f {
        Func::Sin => x.scalar(rad(x.v).sin()),
        Func::Cos => x.scalar(rad(x.v).cos()),
        Func::Tg => x.scalar(rad(x.v).tan()),
        Func::Ctg => x.scalar(1.0 / rad(x.v).tan()),
        Func::Sqrt => x.scalar(x.v.sqrt()),
        Func::Exp => x.scalar(x.v.exp()),
        Func::Re => x.scalar(x.re),
        Func::Im => x.scalar(x.im),
        Func::Mod => Value { v: x.re.hypot(x.im), chan: false, ch: None, ..x },
        Func::Arg => {
            // 0 until the warm-up count (one period with channels, 0 in the
            // `.fvb` context); then atan2 in degrees folded into [-180, 180).
            if env.idx < env.chans.map_or(0, |c| c.warmup()) {
                return Ok(x.scalar(0.0));
            }
            let mut d = x.im.atan2(x.re) * 180.0 / PI;
            if d >= 180.0 {
                d -= 360.0;
            }
            x.scalar(d)
        }
        Func::Cosui => {
            let y = a[1];
            x.scalar((y.im.atan2(y.re) - x.im.atan2(x.re)).cos())
        }
        Func::Absig => {
            let y = a[1];
            let mut d = (y.im.atan2(y.re) - x.im.atan2(x.re)) * 180.0 / PI;
            if d > 180.0 {
                d -= 360.0;
            } else if d < -180.0 {
                d += 360.0;
            }
            x.scalar(d)
        }
        Func::Channel(_) => channel_func(name, a, env)?,
    })
}

/// The channel/analysis accessors decoded so far (ledger L6).
fn channel_func(name: &'static str, a: &[Value], env: Env) -> Result<Value, ExprError> {
    let idx = env.idx;
    let x = first(a)?;
    // Like the math functions, these write the scalar of their first
    // argument and leave its complex pair as it was.
    let out = |v: f64| x.scalar(v);
    Ok(match name {
        "rms:" => {
            let (c, ch) = channel_arg(x, name, env)?;
            out(c.rms(ch, idx))
        }
        "a0:" => {
            let (c, ch) = channel_arg(x, name, env)?;
            out(c.a0(ch, idx))
        }
        "fault:" => {
            let (c, ch) = channel_arg(x, name, env)?;
            out(c.fault(ch, idx))
        }
        "fourie:" => {
            let (c, ch) = channel_arg(x, name, env)?;
            out(c.fourie(ch, a[1].v as i32, idx))
        }
        // vtable slot 1; the `.fvb` Calculator returns 0.0. The argument is
        // parsed and ignored.
        "frequency:" => out(env.chans.map_or(0.0, |c| c.frequency(idx))),
        // 0x41ad96: sum of the three complex values, scalar = |sum|.
        "3i0:" | "3u0:" => {
            let z = a[0].z() + a[1].z() + a[2].z();
            Value::both(z.norm(), z)
        }
        // 0x41a796 / 0x41aa6e: (2A − B − C ± j√3(B − C)) / 6, scalar = |.|.
        "i1:" | "i2:" => {
            let (pa, pb, pc) = (a[0].z(), a[1].z(), a[2].z());
            let j3 = Complex64::new(0.0, 3f64.sqrt()) * (pb - pc);
            let num = 2.0 * pa - pb - pc;
            let z = if name == "i1:" { num + j3 } else { num - j3 } / 6.0;
            Value::both(z.norm(), z)
        }
        _ => return Err(ExprError::NeedsChannel(name)),
    })
}
