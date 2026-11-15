//! Discord Rich Presence: "Playing Flash Launcher · Papa's Freezeria ·
//! 12:34 elapsed" on your Discord profile while a game runs.
//!
//! Talks to the Discord desktop app over its local IPC socket (a named pipe
//! on Windows, a Unix socket elsewhere), the same way the official SDKs do.
//! Frames are: opcode (u32 LE), length (u32 LE), JSON. No dependencies.
//!
//! One background thread owns the connection. If Discord isn't running,
//! nothing happens; the next game start tries again.

use crate::AppState;
use rusqlite::OptionalExtension;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const APP_ID: &str = "1571314169066823782";
/// Setting that turns it off ("false").
pub const SETTING: &str = "discordPresence";
/// Image key uploaded to the Discord app (Rich Presence > Art Assets),
/// shown when a game has no art with a web address.
const FALLBACK_IMAGE: &str = "flash_launcher";

const OP_HANDSHAKE: u32 = 0;
const OP_FRAME: u32 = 1;
const OP_CLOSE: u32 = 2;

#[derive(Debug, Clone, PartialEq)]
pub struct Activity {
    pub title: String,
    pub state: Option<String>,
    /// Unix seconds the session started (Discord counts up from it).
    pub start: i64,
    pub image: Option<String>,
}

enum Msg {
    Set(Activity),
    Clear,
}

pub struct Presence {
    tx: Sender<Msg>,
}

impl Presence {
    pub fn start() -> Self {
        let (tx, rx) = channel();
        std::thread::Builder::new()
            .name("discord-presence".into())
            .spawn(move || run(rx, connect))
            .expect("spawn discord thread");
        Self { tx }
    }

    /// Does nothing, for tests.
    #[cfg(test)]
    pub fn off() -> Self {
        Self { tx: channel().0 }
    }

    pub fn set(&self, a: Activity) {
        let _ = self.tx.send(Msg::Set(a));
    }

    pub fn clear(&self) {
        let _ = self.tx.send(Msg::Clear);
    }
}

trait Pipe: Read + Write + Send {}
impl<T: Read + Write + Send> Pipe for T {}

/// Discord listens on discord-ipc-0..9 (several clients can run at once).
fn connect() -> Option<Box<dyn Pipe>> {
    for i in 0..10 {
        #[cfg(windows)]
        {
            let path = format!(r"\\.\pipe\discord-ipc-{i}");
            if let Ok(f) = std::fs::OpenOptions::new().read(true).write(true).open(&path) {
                return Some(Box::new(f));
            }
        }
        #[cfg(unix)]
        {
            let dirs = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"]
                .iter()
                .filter_map(|v| std::env::var(v).ok())
                .chain(std::iter::once("/tmp".to_string()));
            for dir in dirs {
                // Flatpak and Snap Discord put the socket in a subfolder.
                for sub in ["", "app/com.discordapp.Discord/", "snap.discord/"] {
                    let path = format!("{}/{sub}discord-ipc-{i}", dir.trim_end_matches('/'));
                    if let Ok(s) = std::os::unix::net::UnixStream::connect(&path) {
                        let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
                        return Some(Box::new(s));
                    }
                }
            }
        }
    }
    None
}

fn write_frame(p: &mut dyn Pipe, op: u32, body: &Value) -> std::io::Result<()> {
    let data = serde_json::to_vec(body)?;
    let mut buf = Vec::with_capacity(8 + data.len());
    buf.extend_from_slice(&op.to_le_bytes());
    buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
    buf.extend_from_slice(&data);
    p.write_all(&buf)?;
    p.flush()
}

fn read_frame(p: &mut dyn Pipe) -> std::io::Result<(u32, Value)> {
    let mut head = [0u8; 8];
    p.read_exact(&mut head)?;
    let op = u32::from_le_bytes(head[..4].try_into().unwrap());
    let len = u32::from_le_bytes(head[4..].try_into().unwrap()) as usize;
    if len > 1 << 20 {
        return Err(std::io::Error::other("frame too big"));
    }
    let mut data = vec![0u8; len];
    p.read_exact(&mut data)?;
    Ok((op, serde_json::from_slice(&data).unwrap_or(Value::Null)))
}

fn handshake(p: &mut dyn Pipe) -> std::io::Result<()> {
    write_frame(p, OP_HANDSHAKE, &json!({ "v": 1, "client_id": APP_ID }))?;
    let (op, body) = read_frame(p)?;
    if op == OP_CLOSE || body.get("evt").and_then(Value::as_str) != Some("READY") {
        return Err(std::io::Error::other(format!("discord said no: {body}")));
    }
    Ok(())
}

/// Discord wants 2-128 characters per field.
fn fit(s: &str) -> String {
    let mut out: String = s.trim().chars().take(128).collect();
    while out.chars().count() < 2 {
        out.push(' ');
    }
    out
}

fn activity_json(a: Option<&Activity>, nonce: u64) -> Value {
    let activity = a.map(|a| {
        let mut v = json!({
            "type": 0,
            "details": fit(&a.title),
            "timestamps": { "start": a.start },
            "assets": {
                "large_image": a.image.clone().unwrap_or_else(|| FALLBACK_IMAGE.into()),
                "large_text": fit(&a.title),
            },
        });
        if let Some(s) = a.state.as_deref().filter(|s| !s.trim().is_empty()) {
            v["state"] = json!(fit(s));
        }
        v
    });
    json!({
        "cmd": "SET_ACTIVITY",
        "args": { "pid": std::process::id(), "activity": activity },
        "nonce": nonce.to_string(),
    })
}

fn run(rx: Receiver<Msg>, connect: impl Fn() -> Option<Box<dyn Pipe>>) {
    let mut pipe: Option<Box<dyn Pipe>> = None;
    let mut want: Option<Activity> = None;
    let mut dirty = false;
    let mut nonce = 0u64;
    loop {
        // While something should show but couldn't be sent (Discord closed,
        // or opened after the game started), retry every 20 seconds.
        let wait = if dirty {
            Duration::from_secs(20)
        } else {
            Duration::from_secs(3600)
        };
        match rx.recv_timeout(wait) {
            Ok(Msg::Set(a)) => {
                want = Some(a);
                dirty = true;
            }
            Ok(Msg::Clear) => {
                want = None;
                dirty = true;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        if !dirty {
            continue;
        }
        if pipe.is_none() {
            if want.is_none() {
                // Nothing to clear on a connection we don't have.
                dirty = false;
                continue;
            }
            pipe = connect().and_then(|mut p| handshake(p.as_mut()).ok().map(|_| p));
        }
        let Some(p) = pipe.as_mut() else { continue };
        nonce += 1;
        let ok = write_frame(p.as_mut(), OP_FRAME, &activity_json(want.as_ref(), nonce))
            .and_then(|_| read_frame(p.as_mut()))
            .is_ok();
        if ok {
            dirty = false;
        } else {
            pipe = None; // Discord went away; reconnect next time.
        }
    }
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn enabled(st: &AppState) -> bool {
    st.db()
        .query_row("SELECT value FROM settings WHERE key = ?1", [SETTING], |r| {
            r.get::<_, String>(0)
        })
        .optional()
        .ok()
        .flatten()
        .is_none_or(|v| v != "false")
}

/// Shows `game_id` as being played, starting now.
pub fn playing(st: &AppState, game_id: i64) {
    if !enabled(st) {
        st.presence.clear();
        return;
    }
    let row = {
        let conn = st.db();
        // Discord fetches images by web address itself, so only art we got
        // from the web can be shown. Square icons fit best, then title cards
        // (Flashpoint's are the game's own name art).
        conn.query_row(
            "SELECT g.title, g.developer, g.year,
                (SELECT a.remote_url FROM art a WHERE a.game_id = g.id AND a.selected = 1
                    AND a.remote_url LIKE 'https://%'
                 ORDER BY CASE a.slot WHEN 'icon' THEN 0 WHEN 'wide' THEN 1 WHEN 'logo' THEN 2
                    WHEN 'cover' THEN 3 ELSE 4 END, a.source = 'archive' LIMIT 1)
             FROM games g WHERE g.id = ?1",
            [game_id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<i64>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                ))
            },
        )
        .optional()
        .ok()
        .flatten()
    };
    let Some((title, developer, year, image)) = row else {
        return;
    };
    let state = match (developer, year) {
        (Some(d), Some(y)) => Some(format!("{d} · {y}")),
        (Some(d), None) => Some(d),
        (None, Some(y)) => Some(y.to_string()),
        (None, None) => None,
    };
    st.presence.set(Activity {
        title,
        state,
        start: now_secs(),
        image,
    });
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;
    use std::sync::{Arc, Mutex};

    /// A fake Discord: answers the handshake with READY and every frame
    /// with an ack, and records what it got.
    #[test]
    fn speaks_the_ipc_protocol() {
        let dir = std::env::temp_dir().join(format!("fl-discord-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let sock = dir.join("discord-ipc-0");
        let listener = UnixListener::bind(&sock).unwrap();
        let got: Arc<Mutex<Vec<(u32, Value)>>> = Arc::default();
        let got2 = Arc::clone(&got);
        std::thread::spawn(move || {
            let (s, _) = listener.accept().unwrap();
            let mut s: Box<dyn Pipe> = Box::new(s);
            while let Ok((op, body)) = read_frame(s.as_mut()) {
                got2.lock().unwrap().push((op, body.clone()));
                let reply = if op == OP_HANDSHAKE {
                    json!({ "cmd": "DISPATCH", "evt": "READY" })
                } else {
                    json!({ "cmd": "SET_ACTIVITY", "nonce": body["nonce"] })
                };
                write_frame(s.as_mut(), OP_FRAME, &reply).unwrap();
            }
        });

        let (tx, rx) = channel();
        let path = sock.clone();
        std::thread::spawn(move || {
            run(rx, move || {
                std::os::unix::net::UnixStream::connect(&path)
                    .ok()
                    .map(|s| Box::new(s) as Box<dyn Pipe>)
            })
        });
        tx.send(Msg::Set(Activity {
            title: "Papa's Freezeria".into(),
            state: Some("Flipline Studios · 2011".into()),
            start: 1_700_000_000,
            image: None,
        }))
        .unwrap();
        tx.send(Msg::Clear).unwrap();
        for _ in 0..50 {
            if got.lock().unwrap().len() >= 3 {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let got = got.lock().unwrap();
        assert_eq!(got[0].0, OP_HANDSHAKE);
        assert_eq!(got[0].1["client_id"], APP_ID);
        let act = &got[1].1["args"]["activity"];
        assert_eq!(got[1].1["cmd"], "SET_ACTIVITY");
        assert_eq!(act["details"], "Papa's Freezeria");
        assert_eq!(act["state"], "Flipline Studios · 2011");
        assert_eq!(act["timestamps"]["start"], 1_700_000_000);
        assert_eq!(act["assets"]["large_image"], FALLBACK_IMAGE);
        assert!(
            got[2].1["args"]["activity"].is_null(),
            "clear sends a null activity"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn fields_fit_discords_limits() {
        assert_eq!(fit("Q").chars().count(), 2);
        assert_eq!(fit(&"x".repeat(300)).chars().count(), 128);
    }
}
