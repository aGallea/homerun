/// Resolve the full PATH from the user's login shell.
/// This picks up paths added by nvm, fnm, Homebrew, etc. that aren't
/// available in a bare launchd environment.
#[cfg(unix)]
pub fn resolve_shell_path() -> Option<String> {
    use std::process::Stdio;

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    let output = std::process::Command::new(&shell)
        .args(["-l", "-c", "echo $PATH"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        None
    } else {
        Some(path)
    }
}

/// On Windows the system PATH is inherited directly from the environment,
/// so there is no need to resolve it from a login shell.
#[cfg(windows)]
pub fn resolve_shell_path() -> Option<String> {
    None
}

/// Cached shell PATH resolved once at first use.
pub static SHELL_PATH: std::sync::LazyLock<Option<String>> = std::sync::LazyLock::new(|| {
    let path = resolve_shell_path();
    if let Some(ref p) = path {
        tracing::info!("Resolved shell PATH: {p}");
    }
    path
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_path_lazy_lock_is_accessible() {
        // Just verify the LazyLock can be dereferenced without panic.
        let _val: &Option<String> = &*SHELL_PATH;
    }

    #[cfg(windows)]
    #[test]
    fn resolve_shell_path_returns_none_on_windows() {
        assert!(resolve_shell_path().is_none());
    }

    #[cfg(unix)]
    #[test]
    fn resolve_shell_path_returns_some_on_unix() {
        // On a typical Unix system with a valid $SHELL, we expect Some.
        // If SHELL is unset the function falls back to /bin/zsh which
        // should also produce a PATH on macOS / Linux dev machines.
        let result = resolve_shell_path();
        assert!(result.is_some(), "expected Some on Unix, got None");
        let path = result.unwrap();
        assert!(!path.is_empty());
    }
}
