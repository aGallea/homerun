use crate::persistence::atomic_write;
use anyhow::Result;
#[cfg(not(test))]
use std::path::PathBuf;

/// Returns the path to the auth token file: `~/.homerun/auth.json`
#[cfg(not(test))]
fn auth_file_path() -> PathBuf {
    dirs::home_dir()
        .expect("no home directory")
        .join(".homerun")
        .join("auth.json")
}

#[cfg(not(test))]
pub fn store_token(_service: &str, _account: &str, token: &str) -> Result<()> {
    store_token_at(&auth_file_path(), token)
}

#[cfg(test)]
pub fn store_token(_service: &str, _account: &str, _token: &str) -> Result<()> {
    Ok(())
}

#[cfg(not(test))]
pub fn get_token(_service: &str, _account: &str) -> Result<Option<String>> {
    get_token_at(&auth_file_path())
}

#[cfg(test)]
pub fn get_token(_service: &str, _account: &str) -> Result<Option<String>> {
    Ok(None)
}

#[cfg(not(test))]
pub fn delete_token(_service: &str, _account: &str) -> Result<()> {
    delete_token_at(&auth_file_path())
}

#[cfg(test)]
pub fn delete_token(_service: &str, _account: &str) -> Result<()> {
    Ok(())
}

fn store_token_at(path: &std::path::Path, token: &str) -> Result<()> {
    atomic_write(path, token.as_bytes(), Some(0o600))
}

fn get_token_at(path: &std::path::Path) -> Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(token) if !token.trim().is_empty() => Ok(Some(token.trim().to_string())),
        Ok(_) => Ok(None),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn delete_token_at(path: &std::path::Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_and_retrieve_token() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");
        let token = "ghp_test_token_12345";

        store_token_at(&path, token).unwrap();
        let retrieved = get_token_at(&path).unwrap();
        assert_eq!(retrieved, Some(token.to_string()));

        std::fs::write(&path, format!("  {token}\n")).unwrap();
        assert_eq!(get_token_at(&path).unwrap(), Some(token.to_string()));

        delete_token_at(&path).unwrap();
        let deleted = get_token_at(&path).unwrap();
        assert_eq!(deleted, None);
    }
}
