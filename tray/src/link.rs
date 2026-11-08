//! One `nothingctl watch` child at a time: its JSON lines come out as [`Event`]s and verbs go to
//! its stdin. When it exits (no device connected, the link dropped) it is started again.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Deserialize;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

const RETRY: Duration = Duration::from_secs(3);

#[derive(Clone, Debug, Deserialize)]
pub struct Model {
    pub base: String,
    pub id: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Battery {
    pub id: String,
    pub level: u8,
    pub charging: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Choice {
    pub mode: Option<String>,
    pub available: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Eq {
    pub preset: Option<String>,
    pub available: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub name: String,
    pub model: Model,
    pub battery: Vec<Battery>,
    pub anc: Option<Choice>,
    pub eq: Eq,
    pub spatial: Option<Choice>,
    /// Every other key: the on/off settings, read by their JSON key.
    #[serde(flatten)]
    pub rest: serde_json::Map<String, serde_json::Value>,
}

impl State {
    pub fn toggle(&self, key: &str) -> Option<bool> {
        self.rest.get(key).and_then(|v| v.as_bool())
    }

    pub fn set_toggle(&mut self, key: &str, on: bool) {
        self.rest.insert(key.into(), on.into());
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum Line {
    State(Box<State>),
    Ack {},
    Error { message: String },
    Disconnected,
}

pub enum Event {
    State(Box<State>),
    /// A command was refused; the link stays up.
    Failed(String),
    /// No link: the reason, from the child's last line on stderr when it gave one.
    Down(String),
}

#[derive(Clone, Default)]
pub struct Commands(Arc<Mutex<Option<ChildStdin>>>);

impl Commands {
    /// Writes one verb line (`anc high`, `low-latency on`) to the running child.
    pub fn send(&self, verb: &str) {
        if let Some(stdin) = self.0.lock().unwrap().as_mut() {
            let _ = writeln!(stdin, "{verb}").and_then(|()| stdin.flush());
        }
    }
}

/// `nothingctl` beside this executable, else the one on PATH.
fn nothingctl() -> PathBuf {
    let name = if cfg!(windows) { "nothingctl.exe" } else { "nothingctl" };
    std::env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.parent()?.join(name)))
        .filter(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from(name))
}

fn command() -> Command {
    let mut cmd = Command::new(nothingctl());
    cmd.arg("watch").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // A console child of a GUI app opens a console window unless told not to.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

fn run(commands: &Commands, tx: &UnboundedSender<Event>) -> String {
    let mut child = match command().spawn() {
        Ok(child) => child,
        Err(e) => return format!("Cannot start nothingctl: {e}"),
    };
    *commands.0.lock().unwrap() = child.stdin.take();
    let mut stderr = child.stderr.take();
    let errors = std::thread::spawn(move || {
        let mut text = String::new();
        if let Some(s) = stderr.as_mut() {
            let _ = s.read_to_string(&mut text);
        }
        text
    });
    let mut down = None;
    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let event = match serde_json::from_str::<Line>(&line) {
                Ok(Line::State(s)) => Event::State(s),
                Ok(Line::Error { message }) => Event::Failed(message),
                Ok(Line::Disconnected) => {
                    down = Some("Disconnected".to_string());
                    continue;
                }
                Ok(Line::Ack {}) | Err(_) => continue,
            };
            if tx.send(event).is_err() {
                break;
            }
        }
    }
    *commands.0.lock().unwrap() = None;
    let _ = child.kill();
    let _ = child.wait();
    let stderr = errors.join().unwrap_or_default();
    let last = stderr.lines().rev().find(|l| !l.trim().is_empty()).map(|l| l.trim().trim_start_matches("error: "));
    down.or(last.map(sentence)).unwrap_or_else(|| "Disconnected".into())
}

fn sentence(s: &str) -> String {
    let mut chars = s.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// Starts the watcher thread.
pub fn start() -> (Commands, UnboundedReceiver<Event>) {
    let commands = Commands::default();
    let (tx, rx) = unbounded_channel();
    let thread_commands = commands.clone();
    std::thread::spawn(move || {
        loop {
            let reason = run(&thread_commands, &tx);
            if tx.send(Event::Down(reason)).is_err() {
                return;
            }
            std::thread::sleep(RETRY);
        }
    });
    (commands, rx)
}
