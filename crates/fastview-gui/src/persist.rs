//! The program's own settings across runs (the original's registry keys:
//! `View`, `Settings Parameters`, the workspace, the MRU …), in a
//! tab-separated text file in the platform configuration directory. The
//! circle / differential block state is not here: it is each record's, in
//! its `.set` (`0x408cf0` / `0x409030`, L9.162).

use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Store {
    /// The original's `View` settings: XScale, YAnalogSize, RulerState, Sells.
    pub view: HashMap<String, String>,
    /// `Settings Parameters` (`0x448c00` / `0x448da0`): Dst folder,
    /// UTC for Comtrade2013, Display Encoding.
    pub settings: HashMap<String, String>,
}

/// `View/RulerState` codes (`view+0xe8`): 0 µs, 1 ms, 2 s, 3 absolute, 4 samples.
pub fn ruler_state(u: crate::app::TimeUnit) -> u8 {
    use crate::app::TimeUnit::*;
    match u {
        Micro => 0,
        Milli => 1,
        Sec => 2,
        Absolute => 3,
        Samples => 4,
    }
}

pub fn time_unit(code: u8) -> Option<crate::app::TimeUnit> {
    use crate::app::TimeUnit::*;
    match code {
        0 => Some(Micro),
        1 => Some(Milli),
        2 => Some(Sec),
        3 => Some(Absolute),
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
                ["view", k, v] => {
                    st.view.insert(k.to_string(), v.to_string());
                }
                ["settings", k, v] => {
                    st.settings.insert(k.to_string(), v.to_string());
                }
                _ => {}
            }
        }
        st
    }

    pub fn serialize(&self) -> String {
        let mut lines = Vec::new();
        for (k, v) in &self.view {
            lines.push(format!("view\t{}\t{}", clean(k), clean(v)));
        }
        for (k, v) in &self.settings {
            lines.push(format!("settings\t{}\t{}", clean(k), clean(v)));
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
        st.view.insert("YAnalogSize".into(), "121".into());
        st.settings.insert("Dst folder".into(), "/data/dst".into());
        assert_eq!(Store::parse(&st.serialize()), st);
    }
}
