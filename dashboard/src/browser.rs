use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, TryRecvError};

use serde::Deserialize;

use crate::model::{BoxSize, Finding, ReceiveStatus, Shift};

/// Seconds without a status update before a browser counts as not responding.
/// A container check can sit on one slow page for a while, so this is generous.
const STALE_SECONDS: f64 = 45.0;

/// One container result written by the Playwright process.
#[derive(Clone, Deserialize)]
pub struct Shot {
    pub container: String,
    pub finding: Finding,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub screenshot: String,
}

#[derive(Clone)]
pub struct ImportBatch {
    pub terminal_id: String,
    pub date: String,
    pub shift: Option<Shift>,
    pub results: Vec<Shot>,
}

#[derive(Clone, Default, Deserialize)]
pub struct Tab {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub title: String,
}

#[derive(Deserialize)]
struct SessionFile {
    rev: u64,
    #[serde(default)]
    session_id: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    message: String,
    #[serde(default)]
    login: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    tabs: Vec<Tab>,
    #[serde(default)]
    date: String,
    #[serde(default)]
    shift: Option<Shift>,
    #[serde(default)]
    results: Vec<Shot>,
    #[serde(default)]
    heartbeat: f64,
}

/// What the desk shows for one browser window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrowserState {
    Starting,
    NeedsLogin,
    Online,
    SignedOut,
    Checking,
    NotResponding,
    Error,
}

impl BrowserState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Starting => "Starting",
            Self::NeedsLogin => "Needs sign-in",
            Self::Online => "Online",
            Self::SignedOut => "Signed out — sign in again",
            Self::Checking => "Checking",
            Self::NotResponding => "Not responding",
            Self::Error => "Error",
        }
    }

    /// The user has to do something in this browser.
    pub fn needs_attention(self) -> bool {
        matches!(
            self,
            Self::NeedsLogin | Self::SignedOut | Self::NotResponding | Self::Error
        )
    }
}

/// One Playwright browser, one terminal.
pub struct BrowserSlot {
    pub terminal_id: String,
    pub message: String,
    pub url: String,
    pub tabs: Vec<Tab>,
    pub opened_at: f64,
    pub last_seen: f64,
    status: String,
    login: String,
    session_id: String,
    last_rev: u64,
    child: Option<Child>,
    closing: bool,
}

impl BrowserSlot {
    pub fn state(&self, now: f64) -> BrowserState {
        if self.status == "error" {
            return BrowserState::Error;
        }
        if self.last_seen == 0.0 {
            return if now - self.opened_at > STALE_SECONDS {
                BrowserState::NotResponding
            } else {
                BrowserState::Starting
            };
        }
        if now - self.last_seen > STALE_SECONDS {
            return BrowserState::NotResponding;
        }
        if self.status == "checking" {
            return BrowserState::Checking;
        }
        match self.login.as_str() {
            "online" => BrowserState::Online,
            "logged_out" => BrowserState::SignedOut,
            _ if self.status == "starting" => BrowserState::Starting,
            _ => BrowserState::NeedsLogin,
        }
    }

    pub fn seconds_since_seen(&self, now: f64) -> Option<u64> {
        (self.last_seen > 0.0).then(|| (now - self.last_seen).max(0.0) as u64)
    }

    pub fn is_closing(&self) -> bool {
        self.closing
    }
}

/// All open terminal browsers. Each runs `desk_browser.py` in its own process
/// and talks through data/browsers/<terminal>.json and <terminal>.cmd.json.
pub struct BrowserManager {
    root: PathBuf,
    pub slots: Vec<BrowserSlot>,
    notices: Vec<String>,
}

impl BrowserManager {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            slots: Vec::new(),
            notices: Vec::new(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn any_open(&self) -> bool {
        !self.slots.is_empty()
    }

    pub fn get(&self, terminal_id: &str) -> Option<&BrowserSlot> {
        self.slots.iter().find(|slot| slot.terminal_id == terminal_id)
    }

    pub fn take_notices(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notices)
    }

    pub fn open(&mut self, terminal_id: &str) -> Result<(), String> {
        if self.get(terminal_id).is_some() {
            return Err("That terminal's browser is already open. Use Show to bring it forward.".into());
        }
        let script = self.root.join("desk_browser.py");
        if !script.exists() {
            return Err(format!("Could not find {}", script.display()));
        }
        let python = python_bin(&self.root);
        let dir = self.browsers_dir();
        fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
        let _ = fs::remove_file(self.status_path(terminal_id));
        let _ = fs::remove_file(self.command_path(terminal_id));

        let session_id = format!("{}-{}", terminal_id, (now_secs() * 1000.0) as u64);
        let log_path = dir.join(format!("{terminal_id}.log"));
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|err| format!("Could not open {}: {err}", log_path.display()))?;
        let err_log = log
            .try_clone()
            .map_err(|err| format!("Could not open browser log: {err}"))?;

        let child = Command::new(&python)
            .arg(&script)
            .arg("--terminal")
            .arg(terminal_id)
            .arg("--session")
            .arg(&session_id)
            .current_dir(&self.root)
            .env("DESK_CREDENTIALS_DIR", crate::credentials::credentials_dir())
            .stdin(Stdio::null())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(err_log))
            .spawn()
            .map_err(|err| format!("Could not start {python}: {err}"))?;

        self.slots.push(BrowserSlot {
            terminal_id: terminal_id.to_string(),
            message: "Opening the browser.".into(),
            url: String::new(),
            tabs: Vec::new(),
            opened_at: now_secs(),
            last_seen: 0.0,
            status: "starting".into(),
            login: String::new(),
            session_id,
            last_rev: 0,
            child: Some(child),
            closing: false,
        });
        Ok(())
    }

    pub fn check(
        &mut self,
        terminal_id: &str,
        containers: &[String],
        date: &str,
        shift: Shift,
    ) -> Result<(), String> {
        let state = self
            .get(terminal_id)
            .map(|slot| slot.state(now_secs()))
            .ok_or("Open the browser and sign in before checking containers.")?;
        if matches!(state, BrowserState::NeedsLogin | BrowserState::SignedOut) {
            return Err("Sign in in that browser first, then check containers.".into());
        }
        self.write_command(
            terminal_id,
            serde_json::json!({
                "action": "check",
                "containers": containers,
                "date": date,
                "shift": shift.key(),
            }),
        )
    }

    pub fn focus(&mut self, terminal_id: &str) -> Result<(), String> {
        self.write_command(terminal_id, serde_json::json!({ "action": "focus" }))
    }

    pub fn close(&mut self, terminal_id: &str) -> Result<(), String> {
        self.write_command(terminal_id, serde_json::json!({ "action": "close" }))?;
        if let Some(slot) = self.slots.iter_mut().find(|slot| slot.terminal_id == terminal_id) {
            slot.closing = true;
            slot.message = "Closing the browser.".into();
        }
        Ok(())
    }

    /// Read every status file. Returns container results once per finished check.
    pub fn poll(&mut self) -> Vec<ImportBatch> {
        let mut batches = Vec::new();
        let mut finished = Vec::new();
        let now = now_secs();
        let dir = self.browsers_dir();

        for (index, slot) in self.slots.iter_mut().enumerate() {
            let before = slot.state(now);
            let file = fs::read_to_string(dir.join(format!("{}.json", slot.terminal_id)))
                .ok()
                .and_then(|text| serde_json::from_str::<SessionFile>(&text).ok())
                .filter(|file| file.session_id == slot.session_id);

            if let Some(file) = file {
                slot.status = file.status.clone();
                slot.message = file.message.clone();
                slot.login = file.login.clone();
                slot.url = file.url.clone();
                slot.tabs = file.tabs.clone();
                slot.last_seen = file.heartbeat;

                if file.rev != slot.last_rev {
                    slot.last_rev = file.rev;
                    if !file.results.is_empty() {
                        batches.push(ImportBatch {
                            terminal_id: slot.terminal_id.clone(),
                            date: file.date,
                            shift: file.shift,
                            results: file.results,
                        });
                    }
                }
                if file.status == "closed" {
                    finished.push((index, None));
                    continue;
                }
            }

            if let Some(child) = slot.child.as_mut() {
                if let Ok(Some(status)) = child.try_wait() {
                    let reason = if slot.closing || status.success() {
                        None
                    } else {
                        Some(format!(
                            "{} browser stopped ({status}). See data/browsers/{}.log.",
                            slot.terminal_id.to_uppercase(),
                            slot.terminal_id
                        ))
                    };
                    finished.push((index, reason));
                    continue;
                }
            }

            let after = slot.state(now);
            if after != before && after.needs_attention() && before != BrowserState::Starting {
                self.notices.push(format!(
                    "{}: {}",
                    slot.terminal_id.to_uppercase(),
                    after.label()
                ));
            }
        }

        for (index, reason) in finished.into_iter().rev() {
            let slot = self.slots.remove(index);
            self.notices.push(reason.unwrap_or_else(|| {
                format!("{} browser closed.", slot.terminal_id.to_uppercase())
            }));
        }
        batches
    }

    fn browsers_dir(&self) -> PathBuf {
        self.root.join("data").join("browsers")
    }

    fn status_path(&self, terminal_id: &str) -> PathBuf {
        self.browsers_dir().join(format!("{terminal_id}.json"))
    }

    fn command_path(&self, terminal_id: &str) -> PathBuf {
        self.browsers_dir().join(format!("{terminal_id}.cmd.json"))
    }

    fn write_command(&self, terminal_id: &str, payload: serde_json::Value) -> Result<(), String> {
        if self.get(terminal_id).is_none() {
            return Err("That browser is not open.".into());
        }
        fs::create_dir_all(self.browsers_dir()).map_err(|err| err.to_string())?;
        let path = self.command_path(terminal_id);
        let temporary = path.with_extension("tmp");
        fs::write(&temporary, payload.to_string()).map_err(|err| err.to_string())?;
        fs::rename(&temporary, &path).map_err(|err| err.to_string())?;
        Ok(())
    }
}

impl Drop for BrowserManager {
    /// Ask every browser to close when the desk quits.
    fn drop(&mut self) {
        let ids: Vec<String> = self.slots.iter().map(|slot| slot.terminal_id.clone()).collect();
        for id in ids {
            let _ = self.write_command(&id, serde_json::json!({ "action": "close" }));
        }
    }
}

pub fn now_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs_f64())
        .unwrap_or(0.0)
}

pub fn repo_root() -> PathBuf {
    if let Ok(cwd) = std::env::current_dir() {
        let mut dir = cwd;
        for _ in 0..6 {
            if dir.join("desk_browser.py").exists() {
                return dir;
            }
            if !dir.pop() {
                break;
            }
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// Python with Playwright installed. Prefer the project's `.venv`.
pub fn python_bin(root: &Path) -> String {
    for candidate in [".venv/bin/python", ".venv/Scripts/python.exe", "venv/bin/python"] {
        let path = root.join(candidate);
        if path.exists() {
            return path.to_string_lossy().into_owned();
        }
    }
    if Command::new("python3")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
    {
        "python3".to_string()
    } else {
        "python".to_string()
    }
}

/// One receiving-board row read from a public schedule by `schedules.py`.
#[derive(Clone, Debug, Deserialize)]
pub struct ScheduleRow {
    pub date: String,
    pub shift: Shift,
    pub line: String,
    pub size: BoxSize,
    pub status: ReceiveStatus,
}

#[derive(Debug)]
pub struct ShotResult {
    pub screenshot: String,
    pub receiving: Vec<ScheduleRow>,
}

/// Background run of `screenshot.py --terminal ID --json` for a public empty schedule.
pub struct ScheduleShot {
    pub terminal_id: String,
    rx: Receiver<Result<ShotResult, String>>,
}

impl ScheduleShot {
    pub fn start(root: &Path, terminal_id: &str) -> Result<Self, String> {
        let script = root.join("screenshot.py");
        if !script.exists() {
            return Err(format!("Could not find {}", script.display()));
        }
        let python = python_bin(root);
        let root = root.to_path_buf();
        let id = terminal_id.to_string();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let outcome = Command::new(&python)
                .arg(&script)
                .arg("--terminal")
                .arg(&id)
                .arg("--json")
                .current_dir(&root)
                .stdin(Stdio::null())
                .output()
                .map_err(|err| format!("Could not start {python}: {err}"))
                .and_then(|output| parse_shot_output(&output.stdout, &output.stderr));
            let _ = tx.send(outcome);
        });
        Ok(Self {
            terminal_id: terminal_id.to_string(),
            rx,
        })
    }

    /// `None` while still running.
    pub fn poll(&self) -> Option<Result<ShotResult, String>> {
        match self.rx.try_recv() {
            Ok(outcome) => Some(outcome),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err("Screenshot process stopped.".into())),
        }
    }
}

#[derive(Deserialize)]
struct ShotLine {
    #[serde(default)]
    screenshot: String,
    #[serde(default)]
    error: String,
    #[serde(default)]
    receiving: Vec<serde_json::Value>,
}

fn parse_shot_output(stdout: &[u8], stderr: &[u8]) -> Result<ShotResult, String> {
    let text = String::from_utf8_lossy(stdout);
    for line in text.lines().rev() {
        if let Ok(parsed) = serde_json::from_str::<ShotLine>(line.trim()) {
            if !parsed.screenshot.is_empty() {
                // Skip any row this build does not understand rather than failing.
                let receiving = parsed
                    .receiving
                    .into_iter()
                    .filter_map(|row| serde_json::from_value(row).ok())
                    .collect();
                return Ok(ShotResult {
                    screenshot: parsed.screenshot,
                    receiving,
                });
            }
            if !parsed.error.is_empty() {
                return Err(parsed.error);
            }
        }
    }
    let err = String::from_utf8_lossy(stderr);
    let last = err.lines().rev().find(|line| !line.trim().is_empty()).unwrap_or("no output");
    Err(format!("Screenshot failed: {last}"))
}

#[cfg(test)]
mod tests {
    use super::{parse_shot_output, BrowserSlot, BrowserState};
    use crate::model::{BoxSize, ReceiveStatus, Shift};

    #[test]
    fn reads_screenshot_path_from_json_line() {
        let out = b"noise\n{\"terminal\": \"its\", \"screenshot\": \"/x/its.png\"}\n";
        let shot = parse_shot_output(out, b"").unwrap();
        assert_eq!(shot.screenshot, "/x/its.png");
        assert!(shot.receiving.is_empty());
        let bad = b"{\"terminal\": \"its\", \"error\": \"timeout\"}\n";
        assert_eq!(parse_shot_output(bad, b"").unwrap_err(), "timeout");
        assert!(parse_shot_output(b"", b"Traceback\nModuleNotFoundError: playwright").is_err());
    }

    #[test]
    fn reads_schedule_rows_and_skips_unknown_ones() {
        let out = br#"{"terminal": "its", "screenshot": "/x/its.png", "receiving": [
            {"date": "2026-11-12", "shift": "2nd", "line": "Yang Ming", "size": "forty_hc", "status": "dual_only"},
            {"date": "2026-11-12", "shift": "3rd", "line": "ZIM", "size": "forty", "status": "receiving"}
        ]}"#;
        let shot = parse_shot_output(&out.iter().copied().filter(|b| *b != b'\n').collect::<Vec<_>>(), b"").unwrap();
        assert_eq!(shot.receiving.len(), 1);
        let row = &shot.receiving[0];
        assert_eq!(row.shift, Shift::Second);
        assert_eq!(row.size, BoxSize::FortyHc);
        assert_eq!(row.status, ReceiveStatus::DualOnly);
        assert_eq!(row.line, "Yang Ming");
    }

    fn slot(status: &str, login: &str, last_seen: f64) -> BrowserSlot {
        BrowserSlot {
            terminal_id: "apm".into(),
            message: String::new(),
            url: String::new(),
            tabs: Vec::new(),
            opened_at: 1000.0,
            last_seen,
            status: status.into(),
            login: login.into(),
            session_id: String::new(),
            last_rev: 0,
            child: None,
            closing: false,
        }
    }

    #[test]
    fn browser_state_from_status_file() {
        assert_eq!(slot("starting", "", 0.0).state(1010.0), BrowserState::Starting);
        assert_eq!(slot("starting", "", 0.0).state(1100.0), BrowserState::NotResponding);
        assert_eq!(slot("ready", "needs_login", 1005.0).state(1010.0), BrowserState::NeedsLogin);
        assert_eq!(slot("ready", "online", 1005.0).state(1010.0), BrowserState::Online);
        assert_eq!(slot("ready", "logged_out", 1005.0).state(1010.0), BrowserState::SignedOut);
        assert_eq!(slot("checking", "online", 1005.0).state(1010.0), BrowserState::Checking);
        assert_eq!(slot("ready", "online", 1005.0).state(1100.0), BrowserState::NotResponding);
        assert_eq!(slot("error", "", 1005.0).state(1010.0), BrowserState::Error);
        assert!(BrowserState::SignedOut.needs_attention());
        assert!(!BrowserState::Online.needs_attention());
    }
}
