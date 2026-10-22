//! Single instance (`InitInstance` 0x4483f0): a start with a file argument
//! hands the file to the running instance — the original sends it by
//! WM_COPYDATA to the stored main-window handle — and exits once it is
//! accepted; a start without a file runs normally and becomes the instance
//! later starts hand files to (the newest one stores its handle).
//!
//! Portable stand-in for the window handle: the running instance listens on
//! a loopback port and writes `port token` to a file in the settings folder;
//! a hand-off must present the token, then sends one absolute path per line
//! and waits for `OK`.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

const FILE: &str = "instance";

/// The running instance's end: paths handed over by later starts.
pub struct Server {
    pub rx: Receiver<PathBuf>,
}

/// The folder of the instance file (next to our settings).
pub fn default_dir() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("fastview-rea"))
}

/// Hand `files` to the running instance; true when it accepted them.
pub fn hand_off(dir: &Path, files: &[PathBuf]) -> bool {
    let Ok(text) = std::fs::read_to_string(dir.join(FILE)) else { return false };
    let mut it = text.split_whitespace();
    let (Some(Ok(port)), Some(token)) = (it.next().map(str::parse::<u16>), it.next()) else { return false };
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let Ok(mut s) = TcpStream::connect_timeout(&addr, Duration::from_millis(500)) else { return false };
    let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
    let cwd = std::env::current_dir().unwrap_or_default();
    let mut msg = format!("{token}\n");
    for f in files {
        msg.push_str(&cwd.join(f).to_string_lossy());
        msg.push('\n');
    }
    msg.push('\n');
    if s.write_all(msg.as_bytes()).is_err() {
        return false;
    }
    let mut reply = String::new();
    BufReader::new(s).read_line(&mut reply).is_ok() && reply.trim() == "OK"
}

/// Become the instance later starts hand files to; `wake` is called after
/// each accepted hand-off (to repaint the UI).
pub fn serve(dir: &Path, wake: impl Fn() + Send + 'static) -> Option<Server> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).ok()?;
    let port = listener.local_addr().ok()?.port();
    let token = format!("{:016x}", RandomState::new().build_hasher().finish());
    std::fs::create_dir_all(dir).ok()?;
    std::fs::write(dir.join(FILE), format!("{port} {token}\n")).ok()?;
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(conn) = conn else { continue };
            let _ = conn.set_read_timeout(Some(Duration::from_secs(2)));
            let mut lines = BufReader::new(&conn).lines();
            if lines.next().and_then(Result::ok).as_deref() != Some(token.as_str()) {
                continue;
            }
            let paths: Vec<PathBuf> = lines.map_while(Result::ok).take_while(|l| !l.is_empty()).map(PathBuf::from).collect();
            if paths.iter().any(|p| tx.send(p.clone()).is_err()) {
                return;
            }
            let _ = (&conn).write_all(b"OK\n");
            wake();
        }
    });
    Some(Server { rx })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_start_hands_its_file_over() {
        let dir = std::env::temp_dir().join(format!("fv-instance-{}", std::process::id()));
        // Absolute on every platform (a bare "/x" has no drive on Windows).
        let (a, b, c) = (dir.join("a.cfg"), dir.join("b.cff"), dir.join("c.cfg"));
        assert!(!hand_off(&dir, std::slice::from_ref(&a)), "no instance yet");
        let server = serve(&dir, || {}).unwrap();
        assert!(hand_off(&dir, &[a.clone(), b.clone()]));
        let got: Vec<PathBuf> = server.rx.try_iter().collect();
        assert_eq!(got, [a, b]);
        // A wrong token is ignored.
        let text = std::fs::read_to_string(dir.join(FILE)).unwrap();
        let port = text.split_whitespace().next().unwrap();
        std::fs::write(dir.join(FILE), format!("{port} 0\n")).unwrap();
        assert!(!hand_off(&dir, &[c]));
        assert!(server.rx.try_recv().is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
