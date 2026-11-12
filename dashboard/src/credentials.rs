//! Saved terminal logins, one owner-only JSON file per login system.
//!
//! Folder: `~/Library/Application Support/EmptyReturnDesk/credentials/`
//! (mode 700, files mode 600). `DESK_CREDENTIALS_DIR` overrides it. The
//! Python browser reads the same files to fill sign-in forms; it never
//! submits them.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::ACCOUNTS;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Login {
    pub account: String,
    pub user_id: String,
    pub password: String,
    #[serde(default)]
    pub updated_at: String,
}

pub fn credentials_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("DESK_CREDENTIALS_DIR") {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir);
        }
    }
    crate::store::state_path()
        .parent()
        .map(|dir| dir.join("credentials"))
        .unwrap_or_else(|| PathBuf::from("credentials"))
}

fn file_for(dir: &Path, account: &str) -> Result<PathBuf, String> {
    // Only known account ids become file names.
    if !ACCOUNTS.iter().any(|(id, _)| *id == account) {
        return Err(format!("Unknown login system: {account}"));
    }
    Ok(dir.join(format!("{account}.json")))
}

pub fn load(dir: &Path, account: &str) -> Option<Login> {
    let path = file_for(dir, account).ok()?;
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn save(dir: &Path, login: &Login) -> Result<(), String> {
    let path = file_for(dir, &login.account)?;
    make_private_dir(dir)?;
    let json = serde_json::to_string_pretty(login).map_err(|err| err.to_string())?;
    let temporary = path.with_extension("json.tmp");
    write_private(&temporary, json.as_bytes())?;
    fs::rename(&temporary, &path).map_err(|err| err.to_string())?;
    set_mode(&path, 0o600)
}

pub fn delete(dir: &Path, account: &str) -> Result<(), String> {
    let path = file_for(dir, account)?;
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.to_string()),
    }
}

fn make_private_dir(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|err| format!("Could not create {}: {err}", dir.display()))?;
    set_mode(dir, 0o700)
}

#[cfg(unix)]
fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let _ = fs::remove_file(path);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|err| err.to_string())?;
    file.write_all(bytes).map_err(|err| err.to_string())
}

#[cfg(not(unix))]
fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    fs::write(path, bytes).map_err(|err| err.to_string())
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(|err| err.to_string())
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("desk-cred-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn save_load_delete_round_trip() {
        let dir = temp_dir("round").join("credentials");
        let login = Login {
            account: "emodal".into(),
            user_id: "trucker".into(),
            password: "pw".into(),
            updated_at: "now".into(),
        };
        save(&dir, &login).unwrap();
        let back = load(&dir, "emodal").unwrap();
        assert_eq!(back.user_id, "trucker");
        assert_eq!(back.password, "pw");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let file_mode = fs::metadata(dir.join("emodal.json")).unwrap().permissions().mode();
            let dir_mode = fs::metadata(&dir).unwrap().permissions().mode();
            assert_eq!(file_mode & 0o777, 0o600);
            assert_eq!(dir_mode & 0o777, 0o700);
        }

        delete(&dir, "emodal").unwrap();
        assert!(load(&dir, "emodal").is_none());
        let _ = fs::remove_dir_all(dir.parent().unwrap());
    }

    #[test]
    fn rejects_unknown_account_names() {
        let dir = temp_dir("bad");
        let login = Login {
            account: "../evil".into(),
            ..Login::default()
        };
        assert!(save(&dir, &login).is_err());
        assert!(load(&dir, "../evil").is_none());
    }
}
