//! Expression evaluator for the FastView formula language (`Calculator`).
//!
//! Reconstructed from the decompiled original (`FastView.exe`, see
//! `docs/original/` L1/L2), not inferred from the data:
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
    /// Channel ratio k (`+0xf0`): primary/secondary conversion applied to
    /// displayed values.
    fn ratio_k(&self, _ch: usize) -> f64 {
        1.0
    }
    /// r(e): primary/secondary when values are not shown as primary.
    fn ratio_r(&self, _ch: usize) -> f64 {
        1.0
    }
    /// s(e): k unless the data is primary and shown as secondary.
    fn ratio_s(&self, _ch: usize) -> f64 {
        1.0
    }
    /// The channel's canonical unit `+0x7c` ("" for ADC codes, `0x414c30`).
    fn unit(&self, _ch: usize) -> String {
        String::new()
    }
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
    /// 0x162: no channel of that name (channel mode).
    #[error("Имя канала задано неправильно.")]
    BadChannelName(String),
    /// 0x42e: a computed channel whose samples are neither all scalar nor all
    /// complex.
    #[error("Результат не действителен")]
    InvalidResult,
}

/// An [`ExprError`] together with the formula text it came from, displayed
/// as the original's message box does: "<formula>: <message>", except
/// 0x430 / 0x431, built as "<message>: '<formula>'" (`0x41fd00`,
/// `0x41fe80`), and 0x162, the bare message (the resolver throws it with
/// no text, `0x42ef9f` / `0x42f1cf` → `0x42dfe0`; VM: «Быстрый анализ»
/// with a formula on a name the record no longer has).
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub struct FormulaError {
    pub formula: String,
    pub error: ExprError,
}

impl std::fmt::Display for FormulaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.error {
            ExprError::ArgCount(..) | ExprError::NeedsChannel(_) => write!(f, "{}: '{}'", self.error, self.formula),
            ExprError::BadChannelName(_) => write!(f, "{}", self.error),
            _ => write!(f, "{}: {}", self.formula, self.error),
        }
    }
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
    /// Flags: bit 0 scalar valid, bit 1 complex valid, bit 2 channel
    /// reference. Arithmetic ANDs them; only a bare channel atom keeps the
    /// channel itself.
    f: u8,
    ch: Option<usize>,
}

impl Value {
    fn real(v: f64) -> Value {
        Value { v, re: v, im: 0.0, f: 3, ch: None }
    }
    /// A function result: scalar written (flags 1), complex pair left as it was.
    fn scalar(self, v: f64) -> Value {
        Value { v, f: 1, ch: None, ..self }
    }
    /// A function result with both slots written (flags 3).
    fn both(v: f64, z: Complex64) -> Value {
        Value { v, re: z.re, im: z.im, f: 3, ch: None }
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
    /// `[...]`: the text between the brackets, case kept.
    Dimension(String),
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

    /// Channel-mode evaluation at `idx` returning the scalar if its flag bit
    /// 0 is set and the complex pair if bit 1 is set (`0x415780`).
    pub fn eval_sample(&self, ctx: &Context, chans: &dyn Channels, idx: usize) -> Result<(Option<f64>, Option<Complex64>), ExprError> {
        let v = self.eval_value(Env { ctx, chans: Some(chans), idx })?;
        Ok(((v.f & 1 != 0).then_some(v.v), (v.f & 2 != 0).then(|| v.z())))
    }

    fn eval_env(&self, env: Env) -> Result<f64, ExprError> {
        Ok(self.eval_value(env)?.v)
    }

    fn eval_value(&self, env: Env) -> Result<Value, ExprError> {
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
        Ok(vals[0])
    }

    /// [`Expr::eval`] with the formula text attached to the error.
    pub fn eval_formula(&self, ctx: &Context) -> Result<f64, FormulaError> {
        self.eval(ctx).map_err(|error| FormulaError {
            formula: self.src.clone(),
            error,
        })
    }
}

/// The unit of a channel-mode formula (`0x415bd0`, run after the samples):
/// the text of a `[...]` where the evaluation stopped (the grammar stops at
/// the same token for every sample); else the dimension walk from the start
/// (`0x415d10`…`0x4166b0`) — the text of the `[...]` where the walk stops,
/// or the unit it carried. A formula that does not parse has none.
pub fn formula_unit(src: &str, chans: &dyn Channels) -> String {
    let Ok((toks, _)) = tokenize(src, Some(chans)) else { return String::new() };
    let mut p = Parser { toks, pos: 0 };
    if p.list().is_err() {
        return String::new();
    }
    if let Tok::Dimension(t) = p.peek() {
        return t.clone();
    }
    let mut w = Walk { toks: &p.toks, pos: 0, chans };
    let mut out = vec![String::new()];
    w.list(&mut out);
    match w.toks.get(w.pos) {
        Some(Tok::Dimension(t)) => t.clone(),
        _ => out.swap_remove(0),
    }
}

/// `src` with each channel reference that `rename` maps — given the name
/// as the tokenizer passes it to the resolver (lower-cased) — written as
/// `"new"`; everything else is kept as typed. The scan is the
/// tokenizer's (functions, numbers, `[...]`, quoted and bare names); a
/// formula it rejects comes back unchanged. Ours alone (no counterpart in
/// the original): a reference follows its channel across «Кодировка».
pub fn rename_channel_refs(src: &str, rename: &dyn Fn(&str) -> Option<String>) -> String {
    // The lower-cased text as the tokenizer scans it, with each of its
    // char boundaries' place in `src`.
    let mut low = String::with_capacity(src.len());
    let mut at = Vec::with_capacity(src.len() + 1);
    let mut in_dim = false;
    for (o, c) in src.char_indices() {
        match c {
            '[' => in_dim = true,
            ']' => in_dim = false,
            _ => {}
        }
        at.push((low.len(), o));
        if in_dim {
            low.push(c);
        } else {
            low.extend(c.to_lowercase());
        }
    }
    at.push((low.len(), src.len()));
    let orig = |l: usize| at.iter().find(|(lo, _)| *lo == l).map(|&(_, o)| o);
    let (mut out, mut copied, mut pos) = (String::with_capacity(src.len()), 0, 0);
    while pos < low.len() {
        let rest = &low[pos..];
        let Some(c) = rest.chars().next() else { break };
        let (name, len) = if c == ' ' {
            (None, 1)
        } else if let Some((n, _)) = TABLE.iter().find(|(n, _)| rest.starts_with(n)) {
            (None, n.len())
        } else if c.is_ascii_digit() {
            (None, rest.find(|ch: char| !(ch.is_ascii_digit() || ch == '.')).unwrap_or(rest.len()))
        } else if c == '"' {
            let Some(end) = rest[1..].find('"') else { return src.to_string() };
            (Some(&rest[1..1 + end]), end + 2)
        } else if c == '_' || c.is_alphabetic() {
            let len = rest.find(|ch: char| !(ch == '_' || ch.is_alphanumeric())).unwrap_or(rest.len());
            (Some(&rest[..len]), len)
        } else if c == '[' {
            (None, rest.find(']').map(|i| i + 1).unwrap_or(rest.len()))
        } else {
            return src.to_string();
        };
        if let Some(new) = name.and_then(rename) {
            let (Some(a), Some(b)) = (orig(pos), orig(pos + len)) else { return src.to_string() };
            out.push_str(&src[copied..a]);
            out.push('"');
            out.push_str(&new);
            out.push('"');
            copied = b;
        }
        pos += len;
    }
    out.push_str(&src[copied..]);
    out
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
            ident(&body[..end], chans, &mut toks, &mut idents)?;
            pos += end + 2;
            continue;
        }
        if c == '_' || c.is_alphabetic() {
            let len = rest
                .find(|ch: char| !(ch == '_' || ch.is_alphanumeric()))
                .unwrap_or(rest.len());
            ident(&rest[..len], chans, &mut toks, &mut idents)?;
            pos += len;
            continue;
        }
        if c == '[' {
            let len = rest.find(']').map(|i| i + 1).unwrap_or(rest.len());
            toks.push(Tok::Dimension(rest[1..len].trim_end_matches(']').to_string()));
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

/// An identifier: a variable, or in channel mode a channel (`0x419126`;
/// `CalcChannels` never returns null — an unknown name is error 0x162).
fn ident(name: &str, chans: Option<&dyn Channels>, toks: &mut Vec<Tok>, idents: &mut Vec<String>) -> Result<(), ExprError> {
    match chans {
        None => {
            idents.push(name.to_string());
            toks.push(Tok::Ident(name.to_string()));
        }
        Some(c) => {
            let ch = c.resolve(name).ok_or_else(|| ExprError::BadChannelName(name.to_string()))?;
            toks.push(Tok::Chan(ch));
        }
    }
    Ok(())
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

    /// `0x4196b0`: consuming the END token is a syntax error (so is a
    /// missing `)` — the group's unchecked step consumes END).
    fn advance(&mut self) -> Result<(), ExprError> {
        if self.pos + 1 >= self.toks.len() {
            return Err(ExprError::UnexpectedEof);
        }
        self.pos += 1;
        Ok(())
    }

    fn peek_op(&self, op: u8) -> bool {
        *self.peek() == Tok::Op(op)
    }

    fn list(&mut self) -> Result<Vec<Node>, ExprError> {
        let mut items = vec![self.expr()?];
        while self.peek_op(0x00) {
            self.advance()?;
            items.push(self.expr()?);
        }
        Ok(items)
    }

    fn expr(&mut self) -> Result<Node, ExprError> {
        let mut lhs = self.term()?;
        while let Tok::Op(op @ (0x01 | 0x02)) = *self.peek() {
            self.advance()?;
            let rhs = self.term()?;
            lhs = Node::Bin(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn term(&mut self) -> Result<Node, ExprError> {
        let mut lhs = self.pow()?;
        while let Tok::Op(op @ (0x03 | 0x04)) = *self.peek() {
            self.advance()?;
            let rhs = self.pow()?;
            lhs = Node::Bin(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn pow(&mut self) -> Result<Node, ExprError> {
        let base = self.unary()?;
        if self.peek_op(0x05) {
            self.advance()?;
            let exp = self.unary()?;
            return Ok(Node::Bin(0x05, Box::new(base), Box::new(exp)));
        }
        Ok(base)
    }

    fn unary(&mut self) -> Result<Node, ExprError> {
        let sign = match *self.peek() {
            Tok::Op(op @ (0x01 | 0x02)) => {
                self.advance()?;
                op
            }
            _ => 0x01,
        };
        let p = self.primary()?;
        Ok(if sign == 0x02 { Node::Neg(Box::new(p)) } else { p })
    }

    fn primary(&mut self) -> Result<Node, ExprError> {
        if let Tok::Func(id) = *self.peek() {
            self.advance()?;
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
        self.advance()?;
        if self.peek_op(0x07) {
            return Ok(Vec::new());
        }
        let items = self.list()?;
        self.advance()?;
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
        self.advance()?;
        if matches!(self.peek(), Tok::Dimension(_)) {
            self.advance()?;
        }
        Ok(n)
    }
}

/// The dimension walk (`0x415d10`…`0x4166b0`): the evaluator's levels over
/// the same tokens, each filling element 0 of the list it is given. The
/// left operand's unit wins; `^` and the functions in `unit_rule` set it;
/// no level loops on a `[...]`, so the walk ends at the first one met
/// outside a group.
struct Walk<'a> {
    toks: &'a [Tok],
    pos: usize,
    chans: &'a dyn Channels,
}

impl Walk<'_> {
    fn peek(&self) -> &Tok {
        self.toks.get(self.pos).unwrap_or(&Tok::End)
    }

    fn advance(&mut self) {
        if self.pos + 1 < self.toks.len() {
            self.pos += 1;
        }
    }

    fn op(&self, ops: &[u8]) -> bool {
        matches!(self.peek(), Tok::Op(o) if ops.contains(o))
    }

    /// `0x415d10`: further items each walked into a fresh list and appended.
    fn list(&mut self, out: &mut Vec<String>) {
        self.add(out);
        while self.op(&[0x00]) {
            self.advance();
            let mut item = vec![String::new()];
            self.add(&mut item);
            out.push(item.swap_remove(0));
        }
    }

    /// `0x415fa0` / `0x4160d0`: right operands are walked and dropped.
    fn add(&mut self, out: &mut Vec<String>) {
        self.mul(out);
        while self.op(&[0x01, 0x02]) {
            self.advance();
            self.mul(&mut vec![String::new()]);
        }
    }

    fn mul(&mut self, out: &mut Vec<String>) {
        self.pow(out);
        while self.op(&[0x03, 0x04]) {
            self.advance();
            self.pow(&mut vec![String::new()]);
        }
    }

    /// `0x416200`: one `^`, the exponent dropped, the unit cleared.
    fn pow(&mut self, out: &mut Vec<String>) {
        self.unary(out);
        if self.op(&[0x05]) {
            self.advance();
            self.unary(&mut vec![String::new()]);
            out[0].clear();
        }
    }

    fn unary(&mut self, out: &mut Vec<String>) {
        if self.op(&[0x01, 0x02]) {
            self.advance();
        }
        self.func(out);
    }

    /// `0x416360`: most function tokens are consumed here; absig, frequency,
    /// uncn, pk and the two power / amplitude reductions are left to the
    /// leaf, so their argument group is never walked.
    fn func(&mut self, out: &mut Vec<String>) {
        let id = match *self.peek() {
            Tok::Func(id) => Some(id),
            _ => None,
        };
        if id.is_some_and(|id| !matches!(id, 0x14 | 0x19 | 0x28 | 0x29 | 0x32 | 0x33)) {
            self.advance();
        }
        self.primary(out);
        if let Some(id) = id {
            unit_rule(id, out);
        }
    }

    /// `0x4165f0`: a group walks its list into the same list, then one
    /// token is consumed unchecked as the `)`.
    fn primary(&mut self, out: &mut Vec<String>) {
        if self.op(&[0x06]) {
            self.advance();
            self.list(out);
            self.advance();
        } else {
            self.leaf(out);
        }
    }

    /// `0x4166b0`: pi, null and numbers have no unit; a channel its own;
    /// anything else leaves it. One token is always consumed.
    fn leaf(&mut self, out: &mut [String]) {
        match self.peek() {
            Tok::Pi | Tok::Null | Tok::Num(_) => out[0].clear(),
            Tok::Chan(c) => out[0] = self.chans.unit(*c),
            _ => {}
        }
        self.advance();
    }
}

/// The unit a function token gives (`0x416360`'s table; "cut" = the list
/// truncated to one element): trigonometric, sqrt, exp, arg, fault → "";
/// cosui, fourie → "" cut; frequency → "Hz"; ib, 3i0, i1, i2 → "A"; the
/// voltage functions and uniik → "V"; pf* → "W"; zf / zk / zl → "Ohm"
/// (string 3060); the power / amplitude reductions → «ном»; tormsumcur →
/// its second argument's; chan → cut; the rest keep the first argument's.
fn unit_rule(id: u8, out: &mut Vec<String>) {
    let set = |out: &mut Vec<String>, u: &str| {
        out.truncate(1);
        out[0] = u.to_string();
    };
    match id {
        0x08..=0x0b | 0x0d | 0x0e | 0x12 | 0x18 => out[0].clear(),
        0x13 | 0x16 => set(out, ""),
        0x19 => set(out, "Hz"),
        0x1a..=0x1d => set(out, "A"),
        0x1e..=0x28 | 0x3f => set(out, "V"),
        0x2a..=0x2c => set(out, "W"),
        0x2d..=0x2f => set(out, "Ohm"),
        0x32 | 0x33 => set(out, "ном"),
        0x3b => {
            if out.len() >= 2 {
                out[0] = out[1].clone();
            }
            out.truncate(1);
        }
        0x3e => out.truncate(1),
        _ => {}
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
                f: 7,
                ch: Some(*ch),
            }]
        }
        Node::Pi => vec![Value::real(PI)],
        Node::Null => vec![Value { v: 0.0, re: 0.0, im: 0.0, f: 0, ch: None }],
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
    let f = x.f & y.f;
    Ok(match op {
        0x01 => Value { v: x.v + y.v, re: x.re + y.re, im: x.im + y.im, f, ch: None },
        0x02 => Value { v: x.v - y.v, re: x.re - y.re, im: x.im - y.im, f, ch: None },
        0x03 => Value {
            v: x.v * y.v,
            re: x.re * y.re - x.im * y.im,
            im: x.re * y.im + x.im * y.re,
            f,
            ch: None,
        },
        0x04 => {
            let (re, im) = complex_div(x.re, x.im, y.re, y.im);
            Value { v: x.v / y.v, re, im, f, ch: None }
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
    match (x.f & 4 != 0, x.ch, env.chans) {
        (true, Some(ch), Some(c)) => Ok((c, ch)),
        _ => Err(ExprError::NeedsChannel(name)),
    }
}

/// Function cases (jump table `0x41f2a4`, index = token id − 8).
fn call(f: Func, name: &'static str, a: &[Value], env: Env) -> Result<Value, ExprError> {
    // `frequency:` ignores its argument; the formula dialog writes it with
    // none ("Frequency: ()", `0x4129e0` case 0x1e8). What the original's
    // evaluator does with the empty vector is not reversed: a scalar result.
    if a.is_empty() && name == "frequency:" {
        return Ok(Value::real(0.0).scalar(env.chans.map_or(0.0, |c| c.frequency(env.idx))));
    }
    let x = first(a)?;
    Ok(match f {
        Func::Sin => x.scalar(crate::trig::sin(rad(x.v))),
        Func::Cos => x.scalar(crate::trig::cos(rad(x.v))),
        Func::Tg => x.scalar(crate::trig::tan(rad(x.v))),
        Func::Ctg => x.scalar(1.0 / crate::trig::tan(rad(x.v))),
        Func::Sqrt => x.scalar(x.v.sqrt()),
        Func::Exp => x.scalar(x.v.exp()),
        Func::Re => x.scalar(x.re),
        Func::Im => x.scalar(x.im),
        Func::Mod => Value { v: x.re.hypot(x.im), f: 3, ch: None, ..x },
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
            x.scalar(crate::trig::cos(y.im.atan2(y.re) - x.im.atan2(x.re)))
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

/// Flag-bit-2 check of a channel argument (`0x41fe80`, error 0x431).
fn check(x: Value, name: &'static str) -> Result<(), ExprError> {
    if x.f & 4 != 0 {
        Ok(())
    } else {
        Err(ExprError::NeedsChannel(name))
    }
}

/// Divide, with a zero divisor replaced by 1e-6 (`0x418b70`).
fn safe(x: f64) -> f64 {
    if x == 0.0 {
        1e-6
    } else {
        x
    }
}

/// The original's rounding (`FUN_00411bf0`): floor unless at least half-way.
fn round_fv(x: f64) -> f64 {
    let f = x.floor();
    if (f - x).abs() >= 0.5 {
        x.ceil()
    } else {
        f
    }
}

/// The channel/analysis accessors (ledger L6, `docs/original/specs/calc-channels.md`).
fn channel_func(name: &'static str, a: &[Value], env: Env) -> Result<Value, ExprError> {
    let idx = env.idx;
    let x = first(a)?;
    // Like the math functions, these write the scalar of their first
    // argument and leave its complex pair as it was.
    let out = |v: f64| x.scalar(v);
    let ratio = |e: Value, f: fn(&dyn Channels, usize) -> f64| match (e.ch, env.chans) {
        (Some(ch), Some(c)) => f(c, ch),
        _ => 1.0,
    };
    let r = |e: Value| ratio(e, |c, ch| c.ratio_r(ch));
    let sr = |e: Value| ratio(e, |c, ch| c.ratio_s(ch));
    let kr = |e: Value| ratio(e, |c, ch| c.ratio_k(ch));
    let rt3 = 3f64.sqrt();
    let j = Complex64::new(0.0, 1.0);
    // Scalar and complex combined the same way, flags given.
    let lin = |v: f64, z: Complex64, f: u8| Value { v, re: z.re, im: z.im, f, ch: None };
    let scaled = |e: Value, k: f64| lin(e.v * k, e.z() * k, 3);
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
        "i1:" | "i2:" | "u1f:" | "u2f:" => {
            let (pa, pb, pc) = (a[0].z(), a[1].z(), a[2].z());
            let j3 = j * rt3 * (pb - pc);
            let num = 2.0 * pa - pb - pc;
            let z = if name == "i1:" || name == "u1f:" { num + j3 } else { num - j3 } / 6.0;
            Value::both(z.norm(), z)
        }
        // 0x41b712 / 0x41b87d: from two line voltages.
        "u1l:" | "u2l:" => {
            let (pa, pb) = (a[0].z(), a[1].z());
            let j3 = j * rt3 * pb;
            let z = if name == "u1l:" { 2.0 * pa + pb + j3 } else { 2.0 * pa + pb - j3 } / 6.0;
            Value::both(z.norm(), z)
        }
        // 0x41a611: c − (a + b).
        "ib:" => lin(a[2].v - (a[0].v + a[1].v), a[2].z() - (a[0].z() + a[1].z()), a[0].f & a[1].f & a[2].f),
        "ulf:" => lin(a[0].v - a[1].v, a[0].z() - a[1].z(), a[0].f & a[1].f),
        "ull:" => lin(-(a[0].v + a[1].v), -(a[0].z() + a[1].z()), a[0].f & a[1].f),
        // 0x41ae4d / 0x41b078 / 0x41b2a2.
        "ua:" | "ub:" | "uc:" => {
            for e in &a[..3] {
                check(*e, name)?;
            }
            let q = a[4].v / safe(a[3].v);
            let (wv, wz) = (q * a[2].v, a[2].z() * q);
            let (e0, e1) = (a[0], a[1]);
            let (v, z) = match name {
                "ua:" => (2.0 * e0.v + e1.v + wv, 2.0 * e0.z() + e1.z() + wz),
                "ub:" => (-e0.v + e1.v + wv, -e0.z() + e1.z() + wz),
                _ => (-e0.v - 2.0 * e1.v + wv, -e0.z() - 2.0 * e1.z() + wz),
            };
            lin(v / 3.0, z / 3.0, a[0].f & a[1].f & a[2].f)
        }
        // 0x41c105.
        "uncn:" => {
            let z = a[1].z() + a[2].z() - a[0].z() + (a[3].z() - a[4].z()) / rt3;
            Value::both(z.norm(), z)
        }
        // 0x41c37b.
        "pk:" => {
            let m = x.z().norm();
            out(if m <= 0.001 { 1.0 } else { x.re / m })
        }
        // 0x41c414 / 0x41c4b2 / 0x41c756: complex power, scalar 0.
        "pf:" => lin(0.0, a[0].z().conj() * a[1].z(), 2),
        "pf3wm:" => lin(
            0.0,
            a[0].z() * a[1].z().conj() + a[2].z() * a[3].z().conj() + a[4].z() * a[5].z().conj(),
            a[0].f & a[2].f & a[4].f & 2,
        ),
        "pf2wm:" => lin(0.0, a[0].z() * a[1].z().conj() - a[2].z() * a[3].z().conj(), a[0].f & a[2].f & 2),
        // 0x41c8d2 / 0x41ccc7: loop impedances with a minimum-current gate.
        "zf:" | "zk:" => {
            check(a[0], name)?;
            check(a[1], name)?;
            let (e0, e1) = (a[0], a[1]);
            let gate = if name == "zf:" { a[10].v } else { a[8].v };
            if gate > e1.z().norm() / kr(e1) {
                let (re, im) = complex_div(e0.re, e0.im, 0.0, 0.0);
                return Ok(Value { v: e0.v / 0.0, re, im, f: e0.f & 3, ch: None });
            }
            let den = if name == "zf:" {
                let d = Complex64::new(a[5].v, a[8].v);
                let k0 = (Complex64::new(a[4].v, a[7].v) / d - 1.0) / 3.0;
                let km = (Complex64::new(a[6].v, a[9].v) / d) / 3.0;
                e1.z() + k0 * a[2].z() + km * a[3].z()
            } else {
                let polar = |r: f64, deg: f64| Complex64::new(r * crate::trig::cos(rad(deg)), r * crate::trig::sin(rad(deg)));
                let za = polar(a[4].v, a[6].v);
                let zb = polar(a[5].v, a[7].v);
                e1.z() + a[2].z() * za + a[3].z() * zb
            };
            let (re, im) = complex_div(e0.re, e0.im, den.re, den.im);
            Value { v: e0.v / e1.v, re, im, f: e0.f & e1.f & 2, ch: None }
        }
        // 0x41d072: U/(I1 − I2) with a minimum-current gate on
        // |I1/k1 − I2/k2| (below it: division by zero). The divide helper
        // (0x418b00) gives Z = a/b, s = a.s/b.s, flags a & b & 2.
        "zl:" => {
            let (u, i1, i2) = (a[0], a[1], a[2]);
            let (dv, dz) = (i1.v - i2.v, i1.z() - i2.z());
            let gated = a[3].v > (i1.z() / kr(i1) - i2.z() / kr(i2)).norm();
            let (dv, dz) = if gated { (0.0, Complex64::default()) } else { (dv, dz) };
            let (re, im) = complex_div(u.re, u.im, dz.re, dz.im);
            Value { v: u.v / dv, re, im, f: u.f & i1.f & i2.f & 2, ch: None }
        }
        // 0x41f123 / 0x41f151.
        "chan:" => {
            check(x, name)?;
            Value { f: 3, ch: None, ..x }
        }
        "uniik:" => {
            check(a[0], name)?;
            check(a[1], name)?;
            lin(a[0].v + a[1].v, a[0].z() + a[1].z(), 3)
        }
        // 0x41d27a / 0x41d3ae / 0x41d51a / 0x41d67b: coefficient-scaled channel.
        "coeffpriv:" => scaled(x, a[1].v / safe(sr(x))),
        "coefftt:" => scaled(x, a[1].v / safe(a[2].v) / safe(sr(x))),
        "coeffpowerpriv:" => scaled(x, a[1].v / safe(a[2].v / safe(rt3 * a[3].v)) / safe(sr(x))),
        "coeffamppriv:" => scaled(x, a[1].v / safe(a[2].v) / safe(sr(x))),
        // 0x41e386: winding-group (triangle) currents.
        "grstriangleia:" | "grstriangleib:" | "grstriangleic:" => {
            for e in &a[..3] {
                check(*e, name)?;
            }
            let var = match name {
                "grstriangleia:" => 0,
                "grstriangleib:" => 1,
                _ => 2,
            };
            let (e, n) = (&a[..3], round_fv(a[3].v));
            let (v, z) = if n == 0.0 {
                (e[var].v, e[var].z())
            } else if n == 1.0 {
                let (p, q) = (var, (var + 1) % 3);
                ((e[p].v - e[q].v) / rt3, (e[p].z() - e[q].z()) / rt3)
            } else if n == 11.0 {
                let (p, q) = (var, (var + 2) % 3);
                ((e[p].v - e[q].v) / rt3, (e[p].z() - e[q].z()) / rt3)
            } else {
                (e[0].v, e[0].z())
            };
            let sc = sr(a[0]);
            lin(v / sc, z / sc, 3)
        }
        // 0x41e903.
        "grssumcur:" => {
            check(x, name)?;
            let (v, z) = a.iter().fold((0.0, Complex64::default()), |(v, z), e| (v + r(*e) * e.v, z + e.z() * r(*e)));
            lin(v / r(x), z / r(x), 3)
        }
        // 0x41ec06 / 0x41ed88 / 0x41efa4: restraint currents (Z keeps arg 0).
        "tormsumcur:" => {
            check(a[1], name)?;
            let sum: f64 = a[1..25].iter().map(|e| (e.z() * r(*e)).norm()).sum();
            Value { v: sum / r(a[1]) / x.v, f: 3, ch: None, ..x }
        }
        "tormsubcur:" => {
            check(a[0], name)?;
            check(a[1], name)?;
            let d = (a[1].z() * r(a[1]) - a[0].z() * r(a[0])).norm();
            Value { v: d / r(a[0]) / a[2].v, f: 3, ch: None, ..x }
        }
        "tormmaxcur:" => {
            check(x, name)?;
            let mx = a.iter().map(|e| (e.z() * r(*e)).norm()).fold(f64::NEG_INFINITY, f64::max);
            Value { v: mx / r(x), f: 3, ch: None, ..x }
        }
        // 0x41d7a7: symmetrical components rotated by the winding group.
        "grsposlia:" | "grsposlib:" | "grsposlic:" => {
            for e in &a[..3] {
                check(*e, name)?;
            }
            let op = Complex64::new(-0.5, rt3 * 0.5);
            let (p30, q30) = (Complex64::new(rt3 * 0.5, 0.5), Complex64::new(rt3 * 0.5, -0.5));
            let (e0, e1, e2) = (a[0].z(), a[1].z(), a[2].z());
            let u1 = (e0 + e1 * op + (e2 * op) * op) / 3.0;
            let u2 = (e0 + (e1 * op) * op + e2 * op) / 3.0;
            let u0 = (e0 + e1 + e2) / 3.0;
            let n = round_fv(a[3].v) as i32;
            let n0 = round_fv(a[4].v);
            let t1 = u1 * int_pow(p30, n);
            let t2 = u2 * int_pow(q30, n);
            let r = match name {
                "grsposlia:" => t1 + t2 + u0 * n0,
                "grsposlib:" => t1 * op * op + t2 * op + u0 * n0,
                _ => t1 * op + t2 * op * op + u0 * n0,
            };
            // Complex products zero the scalar slot; only n0·U0 keeps one.
            let v = n0 * (a[0].v + a[1].v + a[2].v) / 3.0;
            let sc = safe(sr(a[0]));
            lin(v / sc, r / sc, 3)
        }
        _ => return Err(ExprError::NeedsChannel(name)),
    })
}

/// Integer complex power by binary exponentiation (`0x420670`); a negative
/// exponent gives the reciprocal.
fn int_pow(z: Complex64, n: i32) -> Complex64 {
    let (mut acc, mut base, mut k) = (Complex64::new(1.0, 0.0), z, n.unsigned_abs());
    while k > 0 {
        if k & 1 == 1 {
            acc *= base;
        }
        base *= base;
        k >>= 1;
    }
    if n < 0 {
        Complex64::new(1.0, 0.0) / acc
    } else {
        acc
    }
}
