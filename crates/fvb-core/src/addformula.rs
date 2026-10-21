//! «Добавить новую формулу» — the original's formula dialog (DIALOG 359,
//! `AddFormulaDlg`, opened by «Добавить» of the formula-editor page,
//! `0x486df0`). A tree of templates (`0x401c70`) on the left; selecting an
//! item (TVN_SELCHANGED `0x403260` → `0x403280`) replaces the child form
//! (`FormulaWnd`) hosted in the placeholder with the dialog named by the
//! item's lParam — a group (lParam 0) shows no form and disables OK. On OK
//! (`0x403530`) the channel name is checked (DDX `0x403180`, `0x4033a0`) and
//! the form's fields are turned into one formula string (`0x4129e0`).
//!
//! Everything here is pure: the tree, the forms (labels and control ids, as
//! in the dialog templates), the field kinds and defaults the form's
//! `OnInitDialog` sets (`0x411f40`, `0x411c70`), the argument rules of
//! `0x412700` and the formula templates of `0x4129e0`.

use std::collections::BTreeMap;

/// Message 396 — an empty name or one not starting with a letter.
pub const MSG_BAD_NAME: &str = "Неправильное имя канала.";
/// Message 473 — the name of a record channel or of another formula.
pub const MSG_NAME_TAKEN: &str = "Данное имя канала уже существует.";
/// First item of the 3Io / 3Ion lists (string 3299, `0x411c70`).
pub const NOT_COUNTED: &str = "Не учитывать";
/// First item of the 24 current lists of the differential forms (string 136).
pub const NOT_USED: &str = "Не использ.";
/// Inserted and selected when a list's default channel is missing (string 191).
pub const NO_CHANNEL: &str = "Канал отсутствует";
/// The text of DIALOG 488 («Частоты»), the form without fields.
pub const NO_PARAMS: &str = "Параметры отсутствуют";
/// Dialog id of the free-formula form, selected when the dialog opens
/// (`0x401c10` → `0x403280(400)`).
pub const FREE_FORMULA: u16 = 400;

/// Control ids shared by the forms (one id = one meaning in every form).
pub mod id {
    pub const FORMULA: u16 = 0x40d;
    pub const IA: u16 = 0x3ef;
    pub const IB: u16 = 0x3f0;
    pub const IC: u16 = 0x3f1;
    pub const UAB: u16 = 0x3f2;
    pub const UBC: u16 = 0x3f3;
    pub const I0: u16 = 0x411;
    pub const UA: u16 = 0x413;
    pub const UB: u16 = 0x414;
    pub const UC: u16 = 0x415;
    pub const X0: u16 = 0x41a;
    pub const X1: u16 = 0x41b;
    pub const R0: u16 = 0x41c;
    pub const R1: u16 = 0x41d;
    pub const CHAN: u16 = 0x421;
    pub const UF: u16 = 0x422;
    pub const IF: u16 = 0x423;
    pub const HARMONIC: u16 = 0x424;
    pub const U0: u16 = 0x426;
    pub const UOS: u16 = 0x428;
    pub const UF1: u16 = 0x429;
    pub const UF2: u16 = 0x42a;
    pub const UNI: u16 = 0x42b;
    pub const UNK: u16 = 0x42c;
    pub const CHAN1: u16 = 0x441;
    pub const CHAN2: u16 = 0x442;
    pub const GROUP_SEQ: u16 = 0x7d9;
    pub const GROUP_TRI: u16 = 0x7da;
    pub const KTORM: u16 = 0x7f9;
    pub const ITR: u16 = 0x7fa;
    pub const I0N: u16 = 0x7fb;
    pub const RM: u16 = 0x7fc;
    pub const XM: u16 = 0x7fd;
    pub const K0: u16 = 0x849;
    pub const F0: u16 = 0x84a;
    pub const DROP_I0: u16 = 0x87f;
}

/// The 24 current lists I1…I24 of the differential-current forms
/// (`DAT_004c2ef0`): I1 = 0x7e0, I2 = 0x7df, I3…I24 = 0x7e1…0x7f6.
pub const CURRENTS24: [u16; 24] = {
    let mut a = [0u16; 24];
    a[0] = 0x7e0;
    a[1] = 0x7df;
    let mut i = 2;
    while i < 24 {
        a[i] = 0x7e1 + (i as u16 - 2);
        i += 1;
    }
    a
};

/// A node of the template tree. A leaf names the form (dialog id) it hosts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Node {
    Group(&'static str, &'static [Node]),
    Leaf(&'static str, u16),
}

use Node::{Group, Leaf};

const P_Q_S_PHASE: &[Node] = &[Leaf("Активная P", 413), Leaf("Реактивная Q", 415), Leaf("Полная S", 414)];

/// The tree exactly as `0x401c70` inserts it (labels from the string table;
/// every group is inserted expanded, TVIS_EXPANDED).
pub const TREE: &[Node] = &[
    Leaf("Формула", FREE_FORMULA),
    Group(
        "Анализ сигналов",
        &[
            Leaf("Угол", 443),
            Leaf("Cos угла между Uф и Iф", 444),
            Leaf("Угол между сигналами", 490),
            Leaf("Среднеквадратичное значение", 445),
            Leaf("Действующее значение гармоники", 446),
            Leaf("Постоянная составляющая", 447),
            Leaf("Аварийная составляющая", 448),
            Leaf("Частоты (для БМРЗ и БРЧН)", 488),
        ],
    ),
    Group("Токи", &[Leaf("Ток Ib", 401), Leaf("Ток 3Io", 402), Leaf("Ток I1", 403), Leaf("Ток I2", 404)]),
    Group(
        "Напряжения",
        &[
            Group("Линейные напряжения", &[Leaf("По фазным напряжениям", 449), Leaf("По линейным напряжениям", 450)]),
            Group("Фазные напряжения", &[Leaf("Ua", 532), Leaf("Ub", 533), Leaf("Uc", 534)]),
            Group(
                "Симметричные составляющие",
                &[
                    Leaf("Напряжение 3Uo", 407),
                    Group("Напряжение U1", &[Leaf("По фазным напряжениям", 410), Leaf("По линейным напряжениям", 408)]),
                    Group("Напряжение U2", &[Leaf("По фазным напряжениям", 411), Leaf("По линейным напряжениям", 409)]),
                ],
            ),
            Leaf("U неисправности цепей напряжения", 1069),
        ],
    ),
    Group(
        "Мощности",
        &[
            Leaf("Коэффициент мощности", 491),
            Group("Фазная", P_Q_S_PHASE),
            Group(
                "Трехфазной цепи",
                &[
                    Group("Метод 3-х ваттметров", &[Leaf("Активная P", 452), Leaf("Реактивная Q", 453), Leaf("Полная S", 454)]),
                    Group("Метод 2-х ваттметров", &[Leaf("Активная P", 456), Leaf("Реактивная Q", 457), Leaf("Полная S", 458)]),
                ],
            ),
        ],
    ),
    Group(
        "Сопротивления",
        &[
            Group("Фаза-фаза", &[Leaf("Активное R", 527), Leaf("Реактивное X", 528), Leaf("Полное Z", 526)]),
            Group(
                "Фаза-земля",
                &[
                    Group("По удельным параметрам", &[Leaf("Активное R", 530), Leaf("Реактивное X", 531), Leaf("Полное Z", 529)]),
                    Group(
                        "По коэффициентам компенсации",
                        &[Leaf("Активное R", 541), Leaf("Реактивное X", 542), Leaf("Полное Z", 540)],
                    ),
                ],
            ),
        ],
    ),
    Group(
        "Дифференциальная защита",
        &[
            Group(
                "Приведение плеч",
                &[
                    Leaf("По коэффициенту приведения", 501),
                    Leaf("По коэффициенту трансформации TT", 502),
                    Leaf("По мощности", 503),
                    Leaf("По номинальному току", 504),
                ],
            ),
            Group(
                "Учет группы соединений",
                &[
                    Group("По последовательностям", &[Leaf("Канал Ia", 505), Leaf("Канал Ib", 506), Leaf("Канал Ic", 507)]),
                    Group("Цифровой треугольник", &[Leaf("Канал Ia", 508), Leaf("Канал Ib", 520), Leaf("Канал Ic", 521)]),
                ],
            ),
            Group("Дифференциальный ток", &[Leaf("Сумма токов", 522)]),
            Group(
                "Ток торможения",
                &[Leaf("Сумма модулей токов", 525), Leaf("Разность токов", 523), Leaf("Максимальный из токов", 524)],
            ),
        ],
    ),
];

/// Every leaf of the tree in display order: (label, dialog id).
pub fn leaves() -> Vec<(&'static str, u16)> {
    fn walk(nodes: &[Node], out: &mut Vec<(&'static str, u16)>) {
        for n in nodes {
            match *n {
                Leaf(l, d) => out.push((l, d)),
                Group(_, kids) => walk(kids, out),
            }
        }
    }
    let mut out = Vec::new();
    walk(TREE, &mut out);
    out
}

/// What a control of a form is, with the initial state `0x411f40` gives it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    /// A channel drop-down list (`0x411c70`): an optional leading item
    /// ([`NOT_COUNTED`] / [`NOT_USED`]), the record's analog channels, then
    /// the computed channels; the default is selected when present.
    Channel { none: Option<&'static str>, default: Option<&'static str> },
    /// An edit box with its initial text.
    Edit(&'static str),
    /// A drop-down list of fixed values and the initially selected index.
    List(&'static [&'static str], usize),
    /// Harmonic numbers 0 … samples per period / 2 (selected 0).
    Harmonic,
    /// A check box (initially checked).
    Check,
}

/// The kind of the control `id` (`0x411f40`).
pub fn kind(id: u16) -> Kind {
    let ch = |default| Kind::Channel { none: None, default };
    match id {
        id::FORMULA => Kind::Edit("Ia + Ib"),
        id::IA => ch(Some("Ia")),
        id::IB => ch(Some("Ib")),
        id::IC => ch(Some("Ic")),
        id::UAB => ch(Some("Uab")),
        id::UBC => ch(Some("Ubc")),
        id::UA => ch(Some("Ua")),
        id::UB => ch(Some("Ub")),
        id::UC => ch(Some("Uc")),
        id::U0 => ch(Some("3U0")),
        id::I0 => Kind::Channel { none: Some(NOT_COUNTED), default: Some("3I0") },
        id::I0N => Kind::Channel { none: Some(NOT_COUNTED), default: Some("3I0n") },
        id::X0 | id::X1 | id::R0 | id::R1 => Kind::Edit("1."),
        id::RM | id::XM | id::K0 | id::F0 => Kind::Edit("0."),
        id::ITR => Kind::Edit("0.25"),
        id::HARMONIC => Kind::Harmonic,
        id::GROUP_SEQ => Kind::List(&["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11"], 0),
        id::GROUP_TRI => Kind::List(&["0", "1", "11"], 0),
        id::KTORM => Kind::List(&["1", "0.5"], 1),
        id::DROP_I0 => Kind::Check,
        0x7e0 => Kind::Channel { none: Some(NOT_USED), default: Some("Ia") },
        0x7df => Kind::Channel { none: Some(NOT_USED), default: Some("Ib") },
        x if CURRENTS24.contains(&x) => Kind::Channel { none: Some(NOT_USED), default: None },
        // 0x421 0x422 0x423 0x428–0x42c 0x441 0x442: no default (first item).
        _ => ch(None),
    }
}

/// A form: its fields as (label, control id) in the template's row order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Form {
    pub dialog: u16,
    pub fields: &'static [(&'static str, u16)],
}

const F_IABC: &[(&str, u16)] = &[("Ia:", id::IA), ("Ib:", id::IB), ("Ic:", id::IC)];
const F_UABC: &[(&str, u16)] = &[("Ua:", id::UA), ("Ub:", id::UB), ("Uc:", id::UC)];
const F_ULIN: &[(&str, u16)] = &[("Uab:", id::UAB), ("Ubc:", id::UBC)];
const F_CHAN: &[(&str, u16)] = &[("Канал:", id::CHAN)];
const F_PF: &[(&str, u16)] = &[("Iф1:", id::IA), ("Uф2-ф3:", id::UA)];
const F_3WM: &[(&str, u16)] =
    &[("Ia:", id::IA), ("Ib:", id::IB), ("Ic:", id::IC), ("Ua:", id::UA), ("Ub:", id::UB), ("Uc:", id::UC)];
const F_2WM: &[(&str, u16)] = &[("Ia:", id::IA), ("Ic:", id::IB), ("Uab:", id::UAB), ("Ubc:", id::UBC)];
const F_ZL: &[(&str, u16)] = &[("Iф1:", id::IA), ("Iф2:", id::IB), ("Uф1-ф2:", id::UA), ("Iтр:", id::ITR)];
const F_ZF: &[(&str, u16)] = &[
    ("Uф:", id::UA),
    ("Iф:", id::IA),
    ("3Io:", id::I0),
    ("3Ion:", id::I0N),
    ("R0:", id::R0),
    ("R1:", id::R1),
    ("Rm:", id::RM),
    ("X0:", id::X0),
    ("X1:", id::X1),
    ("Xm:", id::XM),
    ("Iтр:", id::ITR),
];
const F_ZK: &[(&str, u16)] = &[
    ("Uф:", id::UA),
    ("Iф:", id::IA),
    ("3Io:", id::I0),
    ("3Ion:", id::I0N),
    ("K0:", id::K0),
    ("Km:", id::RM),
    ("Ф0:", id::F0),
    ("Фm:", id::XM),
    ("Iтр:", id::ITR),
];
const F_UPH: &[(&str, u16)] =
    &[("Uab:", id::UAB), ("Ubc:", id::UBC), ("3Uo:", id::U0), ("KтрUл:", id::R0), ("Kтр3Uo:", id::R1)];
const F_SEQ: &[(&str, u16)] =
    &[("Ia:", id::IA), ("Ib:", id::IB), ("Ic:", id::IC), ("Группа:", id::GROUP_SEQ), ("Удалять нулевую последовательность", id::DROP_I0)];
const F_TRI: &[(&str, u16)] = &[("Ia:", id::IA), ("Ib:", id::IB), ("Ic:", id::IC), ("Группа:", id::GROUP_TRI)];
const I24_LABELS: [&str; 24] = [
    "I1:", "I2:", "I3:", "I4:", "I5:", "I6:", "I7:", "I8:", "I9:", "I10:", "I11:", "I12:", "I13:", "I14:", "I15:", "I16:",
    "I17:", "I18:", "I19:", "I20:", "I21:", "I22:", "I23:", "I24:",
];
const F_I24: &[(&str, u16)] = &{
    let mut a = [("", 0u16); 24];
    let mut i = 0;
    while i < 24 {
        a[i] = (I24_LABELS[i], CURRENTS24[i]);
        i += 1;
    }
    a
};
const F_I24K: &[(&str, u16)] = &{
    let mut a = [("Kторм:", id::KTORM); 25];
    let mut i = 0;
    while i < 24 {
        a[i + 1] = (I24_LABELS[i], CURRENTS24[i]);
        i += 1;
    }
    a
};

/// The form of dialog `dialog` (labels and ids from its template).
pub fn form(dialog: u16) -> Option<Form> {
    let fields: &'static [(&'static str, u16)] = match dialog {
        400 => &[("Формула:", id::FORMULA)],
        401 => &[("Ia:", id::IA), ("Ic:", id::IC), ("3Io:", id::I0)],
        402..=404 => F_IABC,
        407 | 410 | 411 => F_UABC,
        408 | 409 => F_ULIN,
        413..=415 => F_PF,
        443 | 445 | 447 | 448 => F_CHAN,
        444 => &[("Uф:", id::UF), ("Iф:", id::IF)],
        446 => &[("Канал:", id::CHAN), ("Гармоника:", id::HARMONIC)],
        449 => &[("U фазн.1:", id::UA), ("U фазн.2:", id::UB)],
        450 => &[("U лин.1:", id::UAB), ("U лин.2:", id::UBC)],
        452..=454 => F_3WM,
        456..=458 => F_2WM,
        488 => &[],
        490 => &[("Канал 1:", id::CHAN1), ("Канал 2:", id::CHAN2)],
        491 => &[("Канал мощности:", id::CHAN1)],
        501 => &[("Канал:", id::CHAN), ("Kприв:", id::R0)],
        502 => &[("Канал:", id::CHAN), ("Kтт:", id::R0), ("Kттбаз:", id::R1)],
        503 => &[("Канал:", id::CHAN), ("Kтт:", id::R0), ("Sном:", id::R1), ("Uном:", id::X0)],
        504 => &[("Канал:", id::CHAN), ("Kтт:", id::R0), ("Iном:", id::R1)],
        505..=507 => F_SEQ,
        508 | 520 | 521 => F_TRI,
        522 | 524 => F_I24,
        523 => &[("I1:", id::IA), ("I2:", id::IB), ("Kторм:", id::KTORM)],
        525 => F_I24K,
        526..=528 => F_ZL,
        529..=531 => F_ZF,
        532..=534 => F_UPH,
        540..=542 => F_ZK,
        1069 => &[("Uос.фаза:", id::UOS), ("Uф1:", id::UF1), ("Uф2:", id::UF2), ("Uни:", id::UNI), ("Uнк:", id::UNK)],
        _ => return None,
    };
    Some(Form { dialog, fields })
}

/// Control ids whose text `0x412700` treats as a channel name.
fn is_channel_field(id: u16) -> bool {
    matches!(
        id,
        0x3ef..=0x3f4
            | 0x411
            | 0x413..=0x415
            | 0x41e..=0x423
            | 0x426
            | 0x428..=0x42c
            | 0x441
            | 0x442
            | 0x7fb
            | 0x818
            | 0x81a
            | 0x81e
            | 0x81f
            | 0x830
    ) || CURRENTS24.contains(&id)
        || (0x831..=0x848).contains(&id)
}

/// One argument as `0x412700` reads it from control `id`. A channel name is
/// wrapped in quotes unless it is a letter followed by letters and digits
/// (an empty name becomes `""`). Any other field is passed as typed, except
/// that a text of digits, blanks, '.' and ',' has its commas made dots.
pub fn arg(id: u16, text: &str) -> String {
    if is_channel_field(id) {
        let mut cs = text.chars();
        let plain = cs.next().is_some_and(char::is_alphabetic) && cs.all(char::is_alphanumeric);
        return if plain { text.to_string() } else { format!("\"{text}\"") };
    }
    if text.chars().all(|c| c.is_ascii_digit() || c.is_whitespace() || c == '.' || c == ',') {
        text.replace(',', ".")
    } else {
        text.to_string()
    }
}

/// The raw texts of a form's controls by id, as the window would return them:
/// a list's selected item, an edit's text; a check box is "1" when checked.
pub type Values = BTreeMap<u16, String>;

/// `0x4129e0`: the formula built from form `dialog` with `values`; `None`
/// for a dialog id that is no form (a group).
pub fn build(dialog: u16, values: &Values) -> Option<String> {
    let raw = |i: u16| values.get(&i).map_or("", String::as_str);
    let a = |i: u16| {
        let s = arg(i, raw(i));
        // 3Io / 3Uo / 3Ion set to «Не учитывать» → 0.
        if matches!(i, id::I0 | id::U0 | id::I0N) && s == format!("\"{NOT_COUNTED}\"") {
            return "0".to_string();
        }
        // An unused or empty differential current → 0.
        if CURRENTS24.contains(&i) && (s == format!("\"{NOT_USED}\"") || s == "\"\"") {
            return "0".to_string();
        }
        s
    };
    let join = |ids: &[u16]| ids.iter().map(|&i| a(i)).collect::<Vec<_>>().join(", ");
    let i24 = || join(&CURRENTS24);
    use id::*;
    let f = match dialog {
        400 => a(FORMULA),
        401 => format!("Ib: ({})", join(&[IA, IC, I0])),
        402 => format!("3I0: ({})", join(&[IA, IB, IC])),
        403 => format!("I1: ({})", join(&[IA, IB, IC])),
        404 => format!("I2: ({})", join(&[IA, IB, IC])),
        407 => format!("3U0: ({})", join(&[UA, UB, UC])),
        408 => format!("U1l: ({})", join(&[UAB, UBC])),
        409 => format!("U2l: ({})", join(&[UAB, UBC])),
        410 => format!("U1f: ({})", join(&[UA, UB, UC])),
        411 => format!("U2f: ({})", join(&[UA, UB, UC])),
        413 => format!("Re: (Pf: ({}))[W]", join(&[IA, UA])),
        414 => format!("Mod:(Pf: ({}))[VA]", join(&[IA, UA])),
        415 => format!("Im: (Pf: ({}))[var]", join(&[IA, UA])),
        443 => format!("Arg: ({})", a(CHAN)),
        444 => format!("CosUI: ({})", join(&[UF, IF])),
        445 => format!("Rms: ({})", a(CHAN)),
        446 => format!("Fourie: ({})", join(&[CHAN, HARMONIC])),
        447 => format!("A0: ({})", a(CHAN)),
        448 => format!("Fault: ({})", a(CHAN)),
        449 => format!("Ulf: ({})", join(&[UA, UB])),
        450 => format!("Ull: ({})", join(&[UAB, UBC])),
        452 => format!("Re: (Pf3WM: ({}))[W]", join(&[UA, IA, UB, IB, UC, IC])),
        453 => format!("Im: (Pf3WM: ({}))[var]", join(&[UA, IA, UB, IB, UC, IC])),
        454 => format!("Mod:(Pf3WM: ({}))[VA]", join(&[UA, IA, UB, IB, UC, IC])),
        456 => format!("Re: (Pf2WM: ({}))[W]", join(&[UAB, IA, UBC, IB])),
        457 => format!("Im: (Pf2WM: ({}))[var]", join(&[UAB, IA, UBC, IB])),
        458 => format!("Mod:(Pf2WM: ({}))[VA]", join(&[UAB, IA, UBC, IB])),
        488 => "Frequency: ()".to_string(),
        490 => format!("ABSig: ({})", join(&[CHAN1, CHAN2])),
        491 => format!("Pk: ({})", a(CHAN1)),
        501 => format!("CoeffPriv: ({})", join(&[CHAN, R0])),
        502 => format!("CoeffTT: ({})", join(&[CHAN, R0, R1])),
        503 => format!("CoeffPowerPriv: ({})", join(&[CHAN, R0, R1, X0])),
        504 => format!("CoeffAmpPriv: ({})", join(&[CHAN, R0, R1])),
        505..=507 => {
            // The check box gives "0" when checked (zero sequence removed).
            let drop = if raw(DROP_I0) == "1" { "0" } else { "1" };
            let ph = ["Ia", "Ib", "Ic"][(dialog - 505) as usize];
            format!("Mod:(GrsPosl{ph}: ({}, {drop}))", join(&[IA, IB, IC, GROUP_SEQ]))
        }
        508 | 520 | 521 => {
            let ph = match dialog {
                508 => "Ia",
                520 => "Ib",
                _ => "Ic",
            };
            format!("Mod:(GrsTriangle{ph}: ({}))", join(&[IA, IB, IC, GROUP_TRI]))
        }
        522 => format!("Mod:(GrsSumCur: ({}))", i24()),
        523 => format!("TormSubCur: ({})", join(&[IA, IB, KTORM])),
        524 => format!("TormMaxCur: ({})", i24()),
        525 => format!("TormSumCur: ({}, {})", a(KTORM), i24()),
        526 => format!("Mod:(Zl: ({}))", join(&[UA, IA, IB, ITR])),
        527 => format!("Re: (Zl: ({}))", join(&[UA, IA, IB, ITR])),
        528 => format!("Im: (Zl: ({}))", join(&[UA, IA, IB, ITR])),
        529..=531 => {
            let head = ["Mod:(Zf: (", "Re:(Zf: (", "Im:(Zf: ("][(dialog - 529) as usize];
            format!("{head}{}))", join(&[UA, IA, I0, I0N, R0, R1, RM, X0, X1, XM, ITR]))
        }
        532..=534 => {
            let f = ["Ua", "Ub", "Uc"][(dialog - 532) as usize];
            format!("{f}: ({})", join(&[UAB, UBC, U0, R0, R1]))
        }
        540 => format!("Mod:(Zk: ({}))", join(&[UA, IA, I0, I0N, K0, RM, F0, XM, ITR])),
        541 => format!("Re:(Zk: ({}))", join(&[UA, IA, I0, I0N, K0, RM, F0, XM, ITR])),
        // The original passes Km again in place of Фm here (0x413f69).
        542 => format!("Im:(Zk: ({}))", join(&[UA, IA, I0, I0N, K0, RM, F0, RM, ITR])),
        1069 => format!("Uncn: ({})", join(&[UOS, UF1, UF2, UNI, UNK])),
        _ => return None,
    };
    Some(f)
}

/// The unit of a built formula: the text of a trailing `[...]` (Pf → W,
/// var, VA), else "".
pub fn unit_of(formula: &str) -> String {
    formula
        .strip_suffix(']')
        .and_then(|s| s.rfind('[').map(|i| s[i + 1..].to_string()))
        .unwrap_or_default()
}

/// Why OK keeps the dialog open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameError {
    /// Message 396: empty, or not starting with a letter.
    Bad,
    /// Message 473: taken by a record channel or another formula.
    Taken,
}

impl NameError {
    pub fn message(self) -> &'static str {
        match self {
            NameError::Bad => MSG_BAD_NAME,
            NameError::Taken => MSG_NAME_TAKEN,
        }
    }
}

/// The OK-time name check: the DDX rule (`0x403180`: empty or a leading
/// digit → 396), then `0x4033a0` (leading non-letter → 396; equal, case
/// sensitively, to a record channel or another formula → 473).
pub fn check_name(name: &str, taken: &[&str]) -> Result<(), NameError> {
    if !name.chars().next().is_some_and(char::is_alphabetic) {
        return Err(NameError::Bad);
    }
    if taken.contains(&name) {
        return Err(NameError::Taken);
    }
    Ok(())
}

/// The name «Добавить» proposes (`0x486df0`): "Канал%i" with one more than
/// the largest trailing number of the existing formula names (at least 1).
pub fn default_name(existing: &[&str]) -> String {
    let next = existing
        .iter()
        .map(|n| {
            let digits: String = n.chars().rev().take_while(char::is_ascii_digit).collect();
            digits.chars().rev().collect::<String>().parse::<i64>().unwrap_or(0) + 1
        })
        .fold(1, i64::max);
    format!("Канал{next}")
}

/// The items of a channel list and the one initially selected
/// (`0x411c70`): the leading item, then `channels`; the default is looked up
/// case-insensitively (CB_FINDSTRINGEXACT); a missing default of a list
/// without a leading item inserts [`NO_CHANNEL`] at the top and selects it.
pub fn channel_items(kind: Kind, channels: &[String]) -> (Vec<String>, usize) {
    let Kind::Channel { none, default } = kind else {
        return (Vec::new(), 0);
    };
    let mut items: Vec<String> = none.map(str::to_string).into_iter().chain(channels.iter().cloned()).collect();
    let Some(def) = default else { return (items, 0) };
    let lower = def.to_lowercase();
    if let Some(i) = items.iter().position(|s| s.to_lowercase() == lower) {
        return (items, i);
    }
    if none.is_none() {
        items.insert(0, NO_CHANNEL.to_string());
    }
    (items, 0)
}

/// The harmonic numbers of 0x424: 0 … rate / line frequency / 2 (`rate` is
/// the record's integer sample rate).
pub fn harmonics(sample_rate: f64, line_freq: f64) -> Vec<String> {
    let top = (sample_rate.trunc() / line_freq) * 0.5;
    if !top.is_finite() || top < 0.0 {
        return vec!["0".into()];
    }
    (0..=top.floor() as u32).map(|i| i.to_string()).collect()
}

/// The initial texts of a form's controls: list selections, edit texts and
/// the check box state.
pub fn initial_values(dialog: u16, channels: &[String], sample_rate: f64, line_freq: f64) -> Values {
    let mut v = Values::new();
    for &(_, i) in form(dialog).map_or(&[][..], |f| f.fields) {
        let text = match kind(i) {
            k @ Kind::Channel { .. } => {
                let (items, sel) = channel_items(k, channels);
                items.get(sel).cloned().unwrap_or_default()
            }
            Kind::Edit(t) => t.to_string(),
            Kind::List(items, sel) => items[sel].to_string(),
            Kind::Harmonic => harmonics(sample_rate, line_freq)[0].clone(),
            Kind::Check => "1".to_string(),
        };
        v.insert(i, text);
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chans() -> Vec<String> {
        ["Ua", "Ub", "Uc", "Ia", "Ib", "Ic"].map(String::from).to_vec()
    }

    fn init(dialog: u16) -> Values {
        initial_values(dialog, &chans(), 2000.0, 50.0)
    }

    fn built(dialog: u16) -> String {
        build(dialog, &init(dialog)).unwrap()
    }

    #[test]
    fn tree_has_every_form_once_per_leaf() {
        let l = leaves();
        assert_eq!(l.len(), 57);
        assert_eq!(l[0], ("Формула", 400));
        for (label, d) in &l {
            assert!(form(*d).is_some(), "{label} {d}");
            assert!(build(*d, &init(*d)).is_some(), "{label} {d}");
        }
        let mut ids: Vec<u16> = l.iter().map(|x| x.1).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), l.len());
    }

    #[test]
    fn argument_rules() {
        assert_eq!(arg(id::IA, "Ia"), "Ia");
        assert_eq!(arg(id::IA, "Фаза А"), "\"Фаза А\"");
        assert_eq!(arg(id::IA, "3I0"), "\"3I0\"");
        assert_eq!(arg(id::IA, "I_a"), "\"I_a\"");
        assert_eq!(arg(id::IA, ""), "\"\"");
        assert_eq!(arg(id::R0, "1,5"), "1.5");
        assert_eq!(arg(id::R0, "2*1,5"), "2*1,5");
        assert_eq!(arg(id::FORMULA, "Ia + Ib"), "Ia + Ib");
        assert_eq!(arg(id::FORMULA, "1,25"), "1.25");
        assert_eq!(arg(CURRENTS24[5], "Не использ."), "\"Не использ.\"");
    }

    #[test]
    fn defaults() {
        assert_eq!(init(400)[&id::FORMULA], "Ia + Ib");
        let v = init(529);
        assert_eq!((v[&id::UA].as_str(), v[&id::IA].as_str()), ("Ua", "Ia"));
        // No 3I0 / 3I0n channel: the list stays on «Не учитывать».
        assert_eq!((v[&id::I0].as_str(), v[&id::I0N].as_str()), (NOT_COUNTED, NOT_COUNTED));
        assert_eq!((v[&id::R0].as_str(), v[&id::RM].as_str(), v[&id::ITR].as_str()), ("1.", "0.", "0.25"));
        // A missing default channel inserts «Канал отсутствует».
        let (items, sel) = channel_items(kind(id::UAB), &chans());
        assert_eq!((items[0].as_str(), sel, items.len()), (NO_CHANNEL, 0, 7));
        // Case-insensitive lookup; no default → first channel.
        let (items, sel) = channel_items(kind(id::IA), &["IA".to_string()]);
        assert_eq!((items[sel].as_str(), sel), ("IA", 0));
        assert_eq!(init(443)[&id::CHAN], "Ua");
        assert_eq!(harmonics(2000.0, 50.0).len(), 21);
        assert_eq!(init(523)[&id::KTORM], "0.5");
        assert_eq!(init(505)[&id::DROP_I0], "1");
    }

    #[test]
    fn signal_and_current_formulas() {
        assert_eq!(built(400), "Ia + Ib");
        assert_eq!(built(443), "Arg: (Ua)");
        assert_eq!(built(444), "CosUI: (Ua, Ua)");
        assert_eq!(built(490), "ABSig: (Ua, Ua)");
        assert_eq!(built(445), "Rms: (Ua)");
        assert_eq!(built(446), "Fourie: (Ua, 0)");
        assert_eq!(built(447), "A0: (Ua)");
        assert_eq!(built(448), "Fault: (Ua)");
        assert_eq!(built(488), "Frequency: ()");
        assert_eq!(built(401), "Ib: (Ia, Ic, 0)");
        assert_eq!(built(402), "3I0: (Ia, Ib, Ic)");
        assert_eq!(built(403), "I1: (Ia, Ib, Ic)");
        assert_eq!(built(404), "I2: (Ia, Ib, Ic)");
    }

    #[test]
    fn voltage_formulas() {
        assert_eq!(built(449), "Ulf: (Ua, Ub)");
        assert_eq!(built(450), "Ull: (\"Канал отсутствует\", \"Канал отсутствует\")");
        let mut v = init(450);
        v.insert(id::UAB, "Ua".into());
        v.insert(id::UBC, "Ub".into());
        assert_eq!(build(450, &v).unwrap(), "Ull: (Ua, Ub)");
        assert_eq!(build(408, &v).unwrap(), "U1l: (Ua, Ub)");
        assert_eq!(build(409, &v).unwrap(), "U2l: (Ua, Ub)");
        assert_eq!(built(407), "3U0: (Ua, Ub, Uc)");
        assert_eq!(built(410), "U1f: (Ua, Ub, Uc)");
        assert_eq!(built(411), "U2f: (Ua, Ub, Uc)");
        v.insert(id::U0, "Uc".into());
        v.insert(id::R0, "100".into());
        v.insert(id::R1, "57,7".into());
        assert_eq!(build(532, &v).unwrap(), "Ua: (Ua, Ub, Uc, 100, 57.7)");
        assert_eq!(build(533, &v).unwrap(), "Ub: (Ua, Ub, Uc, 100, 57.7)");
        assert_eq!(build(534, &v).unwrap(), "Uc: (Ua, Ub, Uc, 100, 57.7)");
        assert_eq!(built(1069), "Uncn: (Ua, Ua, Ua, Ua, Ua)");
    }

    #[test]
    fn power_formulas() {
        assert_eq!(built(491), "Pk: (Ua)");
        assert_eq!(built(413), "Re: (Pf: (Ia, Ua))[W]");
        assert_eq!(built(415), "Im: (Pf: (Ia, Ua))[var]");
        assert_eq!(built(414), "Mod:(Pf: (Ia, Ua))[VA]");
        assert_eq!(built(452), "Re: (Pf3WM: (Ua, Ia, Ub, Ib, Uc, Ic))[W]");
        assert_eq!(built(453), "Im: (Pf3WM: (Ua, Ia, Ub, Ib, Uc, Ic))[var]");
        assert_eq!(built(454), "Mod:(Pf3WM: (Ua, Ia, Ub, Ib, Uc, Ic))[VA]");
        let mut v = init(456);
        v.insert(id::UAB, "Ua".into());
        v.insert(id::UBC, "Uc".into());
        v.insert(id::IB, "Ic".into());
        assert_eq!(build(456, &v).unwrap(), "Re: (Pf2WM: (Ua, Ia, Uc, Ic))[W]");
        assert_eq!(build(457, &v).unwrap(), "Im: (Pf2WM: (Ua, Ia, Uc, Ic))[var]");
        assert_eq!(build(458, &v).unwrap(), "Mod:(Pf2WM: (Ua, Ia, Uc, Ic))[VA]");
        assert_eq!(unit_of(&built(413)), "W");
        assert_eq!(unit_of(&built(454)), "VA");
        assert_eq!(unit_of(&built(402)), "");
    }

    #[test]
    fn impedance_formulas() {
        assert_eq!(built(526), "Mod:(Zl: (Ua, Ia, Ib, 0.25))");
        assert_eq!(built(527), "Re: (Zl: (Ua, Ia, Ib, 0.25))");
        assert_eq!(built(528), "Im: (Zl: (Ua, Ia, Ib, 0.25))");
        assert_eq!(built(529), "Mod:(Zf: (Ua, Ia, 0, 0, 1., 1., 0., 1., 1., 0., 0.25))");
        assert_eq!(built(530), "Re:(Zf: (Ua, Ia, 0, 0, 1., 1., 0., 1., 1., 0., 0.25))");
        assert_eq!(built(531), "Im:(Zf: (Ua, Ia, 0, 0, 1., 1., 0., 1., 1., 0., 0.25))");
        let mut v = init(540);
        for (i, t) in [(id::I0, "Ib"), (id::I0N, "Ic"), (id::K0, "0,6"), (id::RM, "0.2"), (id::F0, "10"), (id::XM, "20")] {
            v.insert(i, t.into());
        }
        assert_eq!(build(540, &v).unwrap(), "Mod:(Zk: (Ua, Ia, Ib, Ic, 0.6, 0.2, 10, 20, 0.25))");
        assert_eq!(build(541, &v).unwrap(), "Re:(Zk: (Ua, Ia, Ib, Ic, 0.6, 0.2, 10, 20, 0.25))");
        assert_eq!(build(542, &v).unwrap(), "Im:(Zk: (Ua, Ia, Ib, Ic, 0.6, 0.2, 10, 0.2, 0.25))");
    }

    #[test]
    fn differential_formulas() {
        assert_eq!(built(501), "CoeffPriv: (Ua, 1.)");
        assert_eq!(built(502), "CoeffTT: (Ua, 1., 1.)");
        assert_eq!(built(503), "CoeffPowerPriv: (Ua, 1., 1., 1.)");
        assert_eq!(built(504), "CoeffAmpPriv: (Ua, 1., 1.)");
        assert_eq!(built(505), "Mod:(GrsPoslIa: (Ia, Ib, Ic, 0, 0))");
        let mut v = init(506);
        v.insert(id::DROP_I0, "0".into());
        v.insert(id::GROUP_SEQ, "11".into());
        assert_eq!(build(506, &v).unwrap(), "Mod:(GrsPoslIb: (Ia, Ib, Ic, 11, 1))");
        assert_eq!(built(507), "Mod:(GrsPoslIc: (Ia, Ib, Ic, 0, 0))");
        assert_eq!(built(508), "Mod:(GrsTriangleIa: (Ia, Ib, Ic, 0))");
        assert_eq!(built(520), "Mod:(GrsTriangleIb: (Ia, Ib, Ic, 0))");
        assert_eq!(built(521), "Mod:(GrsTriangleIc: (Ia, Ib, Ic, 0))");
        let rest = ", 0".repeat(22);
        assert_eq!(built(522), format!("Mod:(GrsSumCur: (Ia, Ib{rest}))"));
        assert_eq!(built(524), format!("TormMaxCur: (Ia, Ib{rest})"));
        assert_eq!(built(525), format!("TormSumCur: (0.5, Ia, Ib{rest})"));
        assert_eq!(built(523), "TormSubCur: (Ia, Ib, 0.5)");
        let mut v = init(522);
        v.insert(CURRENTS24[2], "Ic".into());
        v.insert(CURRENTS24[3], String::new());
        assert_eq!(build(522, &v).unwrap(), format!("Mod:(GrsSumCur: (Ia, Ib, Ic{}))", ", 0".repeat(21)));
    }

    #[test]
    fn name_rule() {
        let taken = ["Ia", "Канал1"];
        assert_eq!(check_name("", &taken), Err(NameError::Bad));
        assert_eq!(check_name("1abc", &taken), Err(NameError::Bad));
        assert_eq!(check_name("_x", &taken), Err(NameError::Bad));
        assert_eq!(check_name(" x", &taken), Err(NameError::Bad));
        assert_eq!(check_name("Ia", &taken), Err(NameError::Taken));
        assert_eq!(check_name("ia", &taken), Ok(()));
        assert_eq!(check_name("Мощность P", &taken), Ok(()));
        assert_eq!(NameError::Bad.message(), "Неправильное имя канала.");
        assert_eq!(default_name(&[]), "Канал1");
        assert_eq!(default_name(&["Канал1", "X7", "Y"]), "Канал8");
    }
}
