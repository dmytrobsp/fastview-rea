//! Expression evaluator for the FastView `.fvb` formula language.
//!
//! Grammar (reverse-engineered from the shipped characteristic library and the
//! `Calculator` class in `FastView.exe`):
//!
//! ```text
//! expr    := term (('+' | '-') term)*
//! term    := unary (('*' | '/') unary)*
//! unary   := '-' unary | '+' unary | primary
//! primary := number | call | ident | '(' expr ')'
//! call    := ident ':' '(' expr ')'
//! ```
//!
//! * Identifiers may contain Latin or Cyrillic letters, digits and `_`
//!   (they cannot *start* with a digit).
//! * Functions use the postfix-colon form `name:(arg)`; the supported set is
//!   `tg ctg sin cos sqrt mod`.
//! * **Trigonometric arguments are in degrees** (conclusive from the literal
//!   idiom `tg:(90-F2)` found throughout the library).
//! * `mod:` is absolute value (модуль).
//!
//! Variables are resolved by name through a [`Context`] map, mirroring the
//! name-keyed `std::map` lookup in the original binary.

use std::collections::HashMap;

/// Variable/parameter environment: name -> value.
pub type Context = HashMap<String, f64>;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ExprError {
    #[error("unexpected end of expression")]
    UnexpectedEof,
    #[error("unexpected character {0:?} at byte {1}")]
    UnexpectedChar(char, usize),
    #[error("expected {0}")]
    Expected(&'static str),
    #[error("unknown function {0:?}")]
    UnknownFunction(String),
    #[error("unknown identifier {0:?}")]
    UnknownIdent(String),
    #[error("trailing input after expression")]
    Trailing,
}

/// A parsed expression AST. Parse once, evaluate many times against different
/// [`Context`]s (settings change, geometry re-evaluates).
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Num(f64),
    Var(String),
    Neg(Box<Expr>),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Pow(Box<Expr>, Box<Expr>),
    Call(Func, Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Func {
    Tg,
    Ctg,
    Sin,
    Cos,
    Sqrt,
    Mod,
}

impl Func {
    fn from_name(name: &str) -> Option<Func> {
        Some(match name {
            "tg" => Func::Tg,
            "ctg" => Func::Ctg,
            "sin" => Func::Sin,
            "cos" => Func::Cos,
            "sqrt" => Func::Sqrt,
            "mod" => Func::Mod,
            _ => return None,
        })
    }

    fn apply(self, x: f64) -> f64 {
        let deg = x.to_radians();
        match self {
            Func::Tg => deg.tan(),
            Func::Ctg => 1.0 / deg.tan(),
            Func::Sin => deg.sin(),
            Func::Cos => deg.cos(),
            Func::Sqrt => x.sqrt(),
            Func::Mod => x.abs(),
        }
    }
}

impl Expr {
    /// Parse a formula string into an AST.
    pub fn parse(src: &str) -> Result<Expr, ExprError> {
        let mut p = Parser::new(src);
        let e = p.parse_expr()?;
        p.skip_ws();
        if p.pos < p.bytes.len() {
            return Err(ExprError::Trailing);
        }
        Ok(e)
    }

    /// Evaluate against a [`Context`]. Unknown identifiers error; callers that
    /// prefer a default can pre-populate the context.
    pub fn eval(&self, ctx: &Context) -> Result<f64, ExprError> {
        Ok(match self {
            Expr::Num(n) => *n,
            // Identifier resolution is case-insensitive: the shipped library
            // contains case-mismatched names (e.g. a parameter declared
            // `ДЗМф2_Rср` is referenced as `ДЗМФ2_Rср`) that the original
            // resolves regardless of case.
            Expr::Var(name) => *ctx
                .get(&name.to_lowercase())
                .ok_or_else(|| ExprError::UnknownIdent(name.clone()))?,
            Expr::Neg(a) => -a.eval(ctx)?,
            Expr::Add(a, b) => a.eval(ctx)? + b.eval(ctx)?,
            Expr::Sub(a, b) => a.eval(ctx)? - b.eval(ctx)?,
            Expr::Mul(a, b) => a.eval(ctx)? * b.eval(ctx)?,
            Expr::Div(a, b) => a.eval(ctx)? / b.eval(ctx)?,
            Expr::Pow(a, b) => a.eval(ctx)?.powf(b.eval(ctx)?),
            Expr::Call(f, a) => f.apply(a.eval(ctx)?),
        })
    }
}

/// Convenience: parse and evaluate in one step.
pub fn eval_str(src: &str, ctx: &Context) -> Result<f64, ExprError> {
    Expr::parse(src)?.eval(ctx)
}

/// Returns true for identifier start characters (letters incl. Cyrillic, `_`).
fn is_ident_start(c: char) -> bool {
    c == '_' || c.is_alphabetic()
}
fn is_ident_part(c: char) -> bool {
    c == '_' || c.is_alphanumeric()
}

struct Parser<'a> {
    src: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Parser {
            src,
            bytes: src.as_bytes(),
            pos: 0,
        }
    }

    fn skip_ws(&mut self) {
        while self.pos < self.bytes.len() {
            let c = self.bytes[self.pos];
            // Only ASCII whitespace appears between tokens.
            if c == b' ' || c == b'\t' || c == b'\r' || c == b'\n' {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn peek(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn eat(&mut self, expect: char) -> bool {
        if self.peek() == Some(expect) {
            self.pos += expect.len_utf8();
            true
        } else {
            false
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, ExprError> {
        let mut lhs = self.parse_term()?;
        loop {
            self.skip_ws();
            match self.peek() {
                Some('+') => {
                    self.bump();
                    let rhs = self.parse_term()?;
                    lhs = Expr::Add(Box::new(lhs), Box::new(rhs));
                }
                Some('-') => {
                    self.bump();
                    let rhs = self.parse_term()?;
                    lhs = Expr::Sub(Box::new(lhs), Box::new(rhs));
                }
                _ => break,
            }
        }
        Ok(lhs)
    }

    fn parse_term(&mut self) -> Result<Expr, ExprError> {
        let mut lhs = self.parse_unary()?;
        loop {
            self.skip_ws();
            match self.peek() {
                Some('*') => {
                    self.bump();
                    let rhs = self.parse_unary()?;
                    lhs = Expr::Mul(Box::new(lhs), Box::new(rhs));
                }
                Some('/') => {
                    self.bump();
                    let rhs = self.parse_unary()?;
                    lhs = Expr::Div(Box::new(lhs), Box::new(rhs));
                }
                _ => break,
            }
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> Result<Expr, ExprError> {
        self.skip_ws();
        match self.peek() {
            Some('-') => {
                self.bump();
                Ok(Expr::Neg(Box::new(self.parse_unary()?)))
            }
            Some('+') => {
                self.bump();
                self.parse_unary()
            }
            _ => self.parse_power(),
        }
    }

    /// Exponentiation `base ^ exponent`, binding tighter than `*`/`/` and
    /// right-associative, with a signed exponent (`x^-2`).
    fn parse_power(&mut self) -> Result<Expr, ExprError> {
        let base = self.parse_primary()?;
        self.skip_ws();
        if self.peek() == Some('^') {
            self.bump();
            let exp = self.parse_unary()?;
            Ok(Expr::Pow(Box::new(base), Box::new(exp)))
        } else {
            Ok(base)
        }
    }

    fn parse_primary(&mut self) -> Result<Expr, ExprError> {
        self.skip_ws();
        let c = self.peek().ok_or(ExprError::UnexpectedEof)?;
        if c == '(' {
            self.bump();
            let e = self.parse_expr()?;
            self.skip_ws();
            if !self.eat(')') {
                return Err(ExprError::Expected(")"));
            }
            return Ok(e);
        }
        if c.is_ascii_digit() || c == '.' {
            return self.parse_number();
        }
        if is_ident_start(c) {
            return self.parse_ident_or_call();
        }
        Err(ExprError::UnexpectedChar(c, self.pos))
    }

    fn parse_number(&mut self) -> Result<Expr, ExprError> {
        let start = self.pos;
        let mut seen_dot = false;
        let mut seen_exp = false;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                self.bump();
            } else if c == '.' && !seen_dot && !seen_exp {
                seen_dot = true;
                self.bump();
            } else if (c == 'e' || c == 'E') && !seen_exp {
                seen_exp = true;
                self.bump();
                if matches!(self.peek(), Some('+') | Some('-')) {
                    self.bump();
                }
            } else {
                break;
            }
        }
        let text = &self.src[start..self.pos];
        text.parse::<f64>()
            .map(Expr::Num)
            .map_err(|_| ExprError::Expected("number"))
    }

    fn parse_ident_or_call(&mut self) -> Result<Expr, ExprError> {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if is_ident_part(c) {
                self.bump();
            } else {
                break;
            }
        }
        let name = self.src[start..self.pos].to_string();
        // Function call form: `name:(arg)`
        if self.peek() == Some(':') {
            self.bump();
            self.skip_ws();
            if !self.eat('(') {
                return Err(ExprError::Expected("( after function colon"));
            }
            let arg = self.parse_expr()?;
            self.skip_ws();
            if !self.eat(')') {
                return Err(ExprError::Expected(")"));
            }
            let func =
                Func::from_name(&name).ok_or_else(|| ExprError::UnknownFunction(name.clone()))?;
            return Ok(Expr::Call(func, Box::new(arg)));
        }
        Ok(Expr::Var(name))
    }
}
