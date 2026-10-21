//! Characteristic settings persisted across runs, as the original keeps them
//! in its settings tree (`FUN_00408cf0` save / `FUN_00409030` load; ledger
//! L5): per diagram ("Circle Chart" / "Diff Chart") the selected block, and
//! for every characteristic its visibility and each parameter's value text.
//! The original's backing file is not known; we use a tab-separated text file
//! in the platform configuration directory.

use std::collections::HashMap;
use std::path::PathBuf;

/// Key of a characteristic: (diagram, block, characteristic).
pub type CharKey = (String, String, String);

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Store {
    /// diagram → selected block name.
    pub selected: HashMap<String, String>,
    pub visible: HashMap<CharKey, bool>,
    /// (diagram, block, characteristic) → parameter → value text.
    pub values: HashMap<CharKey, HashMap<String, String>>,
    /// The original's `View` settings: XScale, YAnalogSize, RulerState, Sells.
    pub view: HashMap<String, String>,
}

/// `View/RulerState` codes (`view+0xe8`): 0 µs, 1 ms, 2 s, 3 absolute, 4 samples.
pub fn ruler_state(u: crate::app::TimeUnit) -> u8 {
    use crate::app::TimeUnit::*;
    match u {
        Micro => 0,
        Milli => 1,
        Sec => 2,
        Samples => 4,
    }
}

pub fn time_unit(code: u8) -> Option<crate::app::TimeUnit> {
    use crate::app::TimeUnit::*;
    match code {
        0 => Some(Micro),
        1 => Some(Milli),
        2 => Some(Sec),
        4 => Some(Samples),
        _ => None,
    }
}

fn config_file() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("fastview-rea").join("characteristics.tsv"))
}

/// Tabs and line breaks cannot appear in a field.
fn clean(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ")
}

impl Store {
    pub fn parse(text: &str) -> Store {
        let mut st = Store::default();
        for line in text.lines() {
            let f: Vec<&str> = line.split('\t').collect();
            match f.as_slice() {
                ["selected", d, b] => {
                    st.selected.insert(d.to_string(), b.to_string());
                }
                ["visible", d, b, c, v] => {
                    st.visible.insert((d.to_string(), b.to_string(), c.to_string()), *v == "1");
                }
                ["view", k, v] => {
                    st.view.insert(k.to_string(), v.to_string());
                }
                ["value", d, b, c, p, v] => {
                    st.values
                        .entry((d.to_string(), b.to_string(), c.to_string()))
                        .or_default()
                        .insert(p.to_string(), v.to_string());
                }
                _ => {}
            }
        }
        st
    }

    pub fn serialize(&self) -> String {
        let mut lines = Vec::new();
        for (d, b) in &self.selected {
            lines.push(format!("selected\t{}\t{}", clean(d), clean(b)));
        }
        for ((d, b, c), v) in &self.visible {
            lines.push(format!("visible\t{}\t{}\t{}\t{}", clean(d), clean(b), clean(c), u8::from(*v)));
        }
        for ((d, b, c), params) in &self.values {
            for (p, v) in params {
                lines.push(format!("value\t{}\t{}\t{}\t{}\t{}", clean(d), clean(b), clean(c), clean(p), clean(v)));
            }
        }
        for (k, v) in &self.view {
            lines.push(format!("view\t{}\t{}", clean(k), clean(v)));
        }
        lines.sort();
        lines.join("\n") + "\n"
    }

    pub fn load() -> Store {
        config_file()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|t| Store::parse(&t))
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Some(p) = config_file() {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(p, self.serialize());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let mut st = Store::default();
        st.selected.insert("Circle Chart".into(), "Блок 1".into());
        let key = ("Circle Chart".to_string(), "Блок 1".to_string(), "ДЗ 1".to_string());
        st.visible.insert(key.clone(), false);
        st.values.entry(key).or_default().insert("Zy".into(), "2*3".into());
        st.view.insert("YAnalogSize".into(), "121".into());
        assert_eq!(Store::parse(&st.serialize()), st);
    }
}
