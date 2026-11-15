use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use crate::config::{state_path, State};
use crate::error::BackupError;

pub fn load() -> Result<State, BackupError> {
    load_from(&state_path())
}

pub fn load_from(path: &Path) -> Result<State, BackupError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(State::default()),
        Err(err) => return Err(BackupError::ReadFailed(err)),
    };
    match State::from_json(&text) {
        Ok(state) => Ok(state),
        Err(err) => {
            tracing::debug!("{err:?}");
            let bak = path.with_extension("json.bak");
            let _ = fs::rename(path, &bak);
            eprintln!("[!] saved state was corrupt, moved to {}", bak.display());
            Ok(State::default())
        }
    }
}

pub fn save(state: &State) -> Result<(), BackupError> {
    save_to(&state_path(), state)
}

pub fn save_to(path: &Path, state: &State) -> Result<(), BackupError> {
    let json = state
        .to_json()
        .map_err(|err| BackupError::Corrupt(err.to_string()))?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(BackupError::WriteFailed)?;
    }
    let tmp = path.with_extension("json.tmp");
    if let Err(err) = write_tmp(&tmp, json.as_bytes()) {
        let _ = fs::remove_file(&tmp);
        return Err(BackupError::WriteFailed(err));
    }
    fs::rename(&tmp, path).map_err(|err| {
        let _ = fs::remove_file(&tmp);
        BackupError::WriteFailed(err)
    })
}

pub fn update<F: FnOnce(&mut State)>(change: F) -> Result<(), BackupError> {
    update_at(&state_path(), change)
}

pub fn update_at<F: FnOnce(&mut State)>(path: &Path, change: F) -> Result<(), BackupError> {
    let mut state = load_from(path)?;
    change(&mut state);
    save_to(path, &state)
}

fn write_tmp(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = open_tmp(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

#[cfg(unix)]
fn open_tmp(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn open_tmp(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
}