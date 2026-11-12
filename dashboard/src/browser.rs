use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, TryRecvError};

use serde::Deserialize;

use crate::model::{Finding, Shift};

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

#[derive(Deserialize)]
struct SessionFile {
    rev: u64,
    #[serde(default)]
    session_id: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    terminal_id: String,
    #[serde(default)]
    message: String,
    #[serde(default)]
    date: String,
    #[serde(default)]
    shift: Option<Shift>,
    #[serde(default)]
    results: Vec<Shot>,
}

pub struct BrowserBridge {
    pub active: bool,
    pub status_text: String,
    pub message: String,
    pub terminal_id: String,
    root: PathBuf,
    session_id: String,
    last_rev: u64,
    child: Option<Child>,
    notice: Option<String>,
}

impl BrowserBridge {
    pub fn new(root: PathBuf) -> Self {
        Self {
            active: false,
            status_text: String::new(),
            message: String::new(),
            terminal_id: String::new(),
            root,
            session_id: String::new(),
            last_rev: 0,
            child: None,
            notice: None,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn take_notice(&mut self) -> Option<String> {
        self.notice.take()
    }

    pub fn open(&mut self, terminal_id: &str) -> Result<(), String> {
        if self.active {
            return Err("A browser is already open. Close it before opening another.".into());
        }
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let script = self.root.join("desk_browser.py");
        if !script.exists() {
            return Err(format!("Could not find {}", script.display()));
        }
        let python = python_bin(&self.root);
        fs::create_dir_all(self.data_dir()).map_err(|err| err.to_string())?;

        let session_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis().to_string())
            .unwrap_or_else(|_| "0".to_string());
        if let Some(existing) = self.read_session() {
            if existing.session_id == session_id {
                self.last_rev = existing.rev;
            }
        }

        let log_path = self.data_dir().join("browser.log");
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
            .stdin(Stdio::null())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(err_log))
            .spawn()
            .map_err(|err| format!("Could not start {python}: {err}"))?;

        self.child = Some(child);
        self.session_id = session_id;
        self.terminal_id = terminal_id.to_string();
        self.active = true;
        self.status_text = "starting".into();
        self.message = "Opening the browser.".into();
        Ok(())
    }

    pub fn check(&mut self, containers: &[String], date: &str, shift: Shift) -> Result<(), String> {
        if !self.active {
            return Err("Open the browser and sign in before checking containers.".into());
        }
        self.write_command("check", containers, date, shift.key())
    }

    pub fn close(&mut self) -> Result<(), String> {
        let _ = self.write_command("close", &[], "", "");
        self.active = false;
        self.status_text = "closed".into();
        self.message = "Closing the browser.".into();
        Ok(())
    }

    /// Read the Playwright status file. Returns container results once, when a check finishes.
    pub fn poll(&mut self) -> Option<ImportBatch> {
        let session = self.read_session();
        let matching = session
            .filter(|session| !self.session_id.is_empty() && session.session_id == self.session_id);
        if matching.is_none() && self.active {
            self.note_if_exited();
        }
        let session = matching?;

        self.status_text = session.status.clone();
        self.message = session.message.clone();
        if !session.terminal_id.is_empty() {
            self.terminal_id = session.terminal_id.clone();
        }

        if session.rev == self.last_rev {
            return None;
        }
        self.last_rev = session.rev;

        if self.active && matches!(session.status.as_str(), "closed" | "error") {
            self.active = false;
            self.notice = Some(session.message.clone());
        }

        if !session.results.is_empty() && matches!(session.status.as_str(), "done" | "closed") {
            return Some(ImportBatch {
                terminal_id: session.terminal_id,
                date: session.date,
                shift: session.shift,
                results: session.results,
            });
        }
        None
    }

    fn note_if_exited(&mut self) {
        let Some(child) = self.child.as_mut() else {
            return;
        };
        let Ok(Some(status)) = child.try_wait() else {
            return;
        };
        self.active = false;
        self.notice = Some(format!("Browser stopped ({status}). See data/browser.log."));
    }

    fn data_dir(&self) -> PathBuf {
        self.root.join("data")
    }

    fn read_session(&self) -> Option<SessionFile> {
        let text = fs::read_to_string(self.data_dir().join("browser_session.json")).ok()?;
        serde_json::from_str(&text).ok()
    }

    fn write_command(
        &self,
        action: &str,
        containers: &[String],
        date: &str,
        shift: &str,
    ) -> Result<(), String> {
        fs::create_dir_all(self.data_dir()).map_err(|err| err.to_string())?;
        let payload = serde_json::json!({
            "action": action,
            "containers": containers,
            "date": date,
            "shift": shift,
        });
        let path = self.data_dir().join("browser_command.json");
        let temporary = self.data_dir().join("browser_command.json.tmp");
        fs::write(&temporary, payload.to_string()).map_err(|err| err.to_string())?;
        fs::rename(&temporary, &path).map_err(|err| err.to_string())?;
        Ok(())
    }
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

/// Background run of `screenshot.py --terminal ID --json` for a public empty schedule.
pub struct ScheduleShot {
    pub terminal_id: String,
    rx: Receiver<Result<String, String>>,
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
    pub fn poll(&self) -> Option<Result<String, String>> {
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
}

fn parse_shot_output(stdout: &[u8], stderr: &[u8]) -> Result<String, String> {
    let text = String::from_utf8_lossy(stdout);
    for line in text.lines().rev() {
        if let Ok(parsed) = serde_json::from_str::<ShotLine>(line.trim()) {
            if !parsed.screenshot.is_empty() {
                return Ok(parsed.screenshot);
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
    use super::parse_shot_output;

    #[test]
    fn reads_screenshot_path_from_json_line() {
        let out = b"noise\n{\"terminal\": \"its\", \"screenshot\": \"/x/its.png\"}\n";
        assert_eq!(parse_shot_output(out, b"").unwrap(), "/x/its.png");
        let bad = b"{\"terminal\": \"its\", \"error\": \"timeout\"}\n";
        assert_eq!(parse_shot_output(bad, b"").unwrap_err(), "timeout");
        assert!(parse_shot_output(b"", b"Traceback\nModuleNotFoundError: playwright").is_err());
    }
}
