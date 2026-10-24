//! Behaviour of the formula language pinned to the decompiled `Calculator`
//! in the original `FastView.exe` (ledger L2). Each test names the address
//! whose code it reproduces.

use fvb_core::expr::{eval_str, Context, ExprError};

fn c() -> Context {
    Context::new()
}

fn ev(src: &str) -> f64 {
    eval_str(src, &c()).unwrap()
}

/// 0x419f20: `pow := unary ['^' unary]` with a single `^`; the parse returns
/// and the remaining tokens are ignored, so `2^3^2` is `2^3`.
#[test]
fn power_does_not_chain() {
    assert_eq!(ev("2^3^2"), 8.0);
    assert_eq!(ev("2^3*4"), 32.0);
    assert_eq!(ev("2*3^2"), 18.0);
    assert_eq!(ev("2*3^2^5"), 18.0);
}

/// 0x419fc0: one optional sign, applied to the primary before `^`.
#[test]
fn unary_minus_binds_tighter_than_power() {
    assert_eq!(ev("-2^2"), 4.0);
    assert_eq!(ev("-2^3"), -8.0);
    assert!(eval_str("--2", &c()).is_err());
    assert_eq!(ev("+2"), 2.0);
}

/// 0x41f7cb: integer exponent only; n=0 -> 1; n>0 repeated multiply; n<0
/// returns the base unchanged (original bug).
#[test]
fn power_integer_semantics() {
    assert_eq!(ev("0^0"), 1.0);
    assert_eq!(ev("(-3)^3"), -27.0);
    assert_eq!(ev("2^-1"), 2.0);
    assert_eq!(ev("5^-3"), 5.0);
    assert_eq!(eval_str("2^0.5", &c()), Err(ExprError::NonIntegerPower));
}

/// 0x41a23d..: trig in degrees, converted as v*pi/180 (multiply then divide).
#[test]
fn trig_conversion_order_is_bit_exact() {
    // 1.5, 2.3, 3.0, 4.6 round differently under v*(pi/180) (to_radians).
    for v in [1.5f64, 2.3, 3.0, 4.6, 30.0, 33.3, 45.0, 89.9, 123.456] {
        let r = v * std::f64::consts::PI / 180.0;
        assert_eq!(ev(&format!("sin:({v})")), r.sin());
        assert_eq!(ev(&format!("cos:({v})")), r.cos());
        assert_eq!(ev(&format!("tg:({v})")), r.tan());
        assert_eq!(ev(&format!("ctg:({v})")), 1.0 / r.tan());
    }
}

/// No domain checks anywhere: IEEE inf/NaN flow through.
#[test]
fn no_domain_checks() {
    assert!(ev("sqrt:(0-4)").is_nan());
    assert_eq!(ev("1/0"), f64::INFINITY);
    assert_eq!(ev("ctg:(0)"), f64::INFINITY);
    assert!(ev("tg:(90)") > 1e15);
}

/// 0x41f390: a function takes a parenthesised list or ONE bare atom.
#[test]
fn bare_atom_function_argument() {
    assert_eq!(ev("sin:30"), ev("sin:(30)"));
    assert_eq!(ev("sin:30+1"), ev("sin:(30)") + 1.0);
    assert!(eval_str("sin:-30", &c()).is_err());
    assert!(eval_str("sin:cos:30", &c()).is_err());
}

/// FUN_0041fd00: exact argument counts.
#[test]
fn argument_count_is_checked() {
    assert!(matches!(eval_str("sin:(1,2)", &c()), Err(ExprError::ArgCount("sin:", 1, 2))));
    assert!(matches!(eval_str("cosui:(1)", &c()), Err(ExprError::ArgCount("cosui:", 2, 1))));
}

/// Operands carry a scalar and a complex pair; trig/sqrt/exp/^ update only the
/// scalar while mod:/re:/im: read the complex pair (0x41a23d.., 0x420610).
#[test]
fn scalar_and_complex_slots_are_independent() {
    // Arithmetic keeps both in sync, so mod: is |x| on plain numbers.
    assert_eq!(ev("mod:(0-7)"), 7.0);
    assert_eq!(ev("mod:(3*(0-2))"), 6.0);
    assert_eq!(ev("re:(5)"), 5.0);
    assert_eq!(ev("im:(5)"), 0.0);
    // sin: leaves the complex pair at its argument (30), so mod: reads 30.
    assert_eq!(ev("mod:(sin:(30))"), 30.0);
    // ^ is scalar-only too: re: of 3^2 is still 3.
    assert_eq!(ev("re:(3^2)"), 3.0);
}

/// arg: atan2 in degrees folded into [-180, 180).
#[test]
fn arg_range() {
    assert_eq!(ev("arg:(5)"), 0.0);
    assert_eq!(ev("arg:(0-5)"), -180.0);
}

/// FUN_00418c40: `pi` is a bare-prefix token (no colon), `null` likewise.
#[test]
fn pi_and_null_tokens() {
    assert_eq!(ev("pi"), std::f64::consts::PI);
    assert_eq!(ev("PI*2"), 2.0 * std::f64::consts::PI);
    assert_eq!(ev("null+1"), 1.0);
    // `pid` tokenizes as `pi` + `d`, so a variable named pid is unreachable.
    let ctx: Context = [("pid".to_string(), 7.0)].into_iter().collect();
    assert!(eval_str("pid", &ctx).is_err());
}

/// FUN_00418c40 lexing: only spaces are skipped; numbers have no exponent and
/// no leading dot; `[...]` units after an atom are ignored; case-insensitive.
#[test]
fn lexical_rules() {
    assert!(matches!(eval_str("1\t+2", &c()), Err(ExprError::UnexpectedChar('\t', _))));
    assert!(eval_str(".5", &c()).is_err());
    assert!(eval_str("1.2.3", &c()).is_err());
    assert_eq!(ev("1."), 1.0);
    // `1e5` is 1 followed by identifier e5, which must exist.
    assert!(matches!(eval_str("1e5", &c()), Err(ExprError::UnknownIdent(_))));
    let ctx: Context = [("e5".to_string(), 0.0)].into_iter().collect();
    assert_eq!(eval_str("1e5", &ctx).unwrap(), 1.0);
    assert_eq!(ev("SIN:(30)"), ev("sin:(30)"));
    assert_eq!(ev("2[Ом]*3"), 6.0);
}

/// 0x419950: trailing tokens after the expression are ignored, but every
/// identifier is still resolved by the tokenizer.
#[test]
fn trailing_input_is_ignored_but_resolved() {
    assert_eq!(ev("1+2)"), 3.0);
    assert_eq!(ev("2 3"), 2.0);
    assert!(matches!(eval_str("2 nosuch", &c()), Err(ExprError::UnknownIdent(_))));
    // A top-level list must reduce to exactly one value.
    assert!(eval_str("1,2", &c()).is_err());
}

/// Channel/analysis accessors are recognised but need an oscillogram channel.
#[test]
fn channel_functions_need_a_channel() {
    assert!(matches!(eval_str("rms:(1)", &c()), Err(ExprError::NeedsChannel("rms:"))));
    assert!(matches!(eval_str("chan:(1)", &c()), Err(ExprError::NeedsChannel("chan:"))));
    // pk: has no channel check: Re/|·| of a plain number.
    assert_eq!(eval_str("pk:(1)", &c()).unwrap(), 1.0);
}

/// 0x4196b0: consuming the END token throws 0x157, so a group missing its
/// `)` — whose closing step consumes without checking — is a syntax error.
#[test]
fn a_missing_close_paren_is_a_syntax_error() {
    for src in ["(1+2", "2*(3", "sin:(0", "((1)"] {
        assert_eq!(eval_str(src, &c()), Err(ExprError::UnexpectedEof), "{src}");
    }
    assert_eq!(ev("(1+2)"), 3.0);
}

/// 0x41fd00 / 0x41fe80 build "<message>: '<formula>'"; the rest
/// "<formula>: <message>".
#[test]
fn argument_errors_quote_the_formula_after_the_message() {
    use fvb_core::expr::FormulaError;
    let e = |error| FormulaError { formula: "rms:(1)".into(), error }.to_string();
    assert_eq!(e(ExprError::NeedsChannel("rms")), "Аргумент функции должен быть каналом: 'rms:(1)'");
    assert_eq!(e(ExprError::ArgCount("rms", 1, 2)), "Неправильное число переменных: 'rms:(1)'");
    assert_eq!(e(ExprError::UnexpectedEof), "rms:(1): Синтаксическая ошибка в формуле");
}
