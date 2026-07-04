use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) fn resolve_command(command: &str) -> String {
    let path_var = std::env::var_os("PATH");
    resolve_command_with(command, path_var.as_deref())
}

fn resolve_command_with(command: &str, path_var: Option<&OsStr>) -> String {
    let command = command.trim();
    if command.is_empty() {
        return String::new();
    }

    let command_path = Path::new(command);
    if is_path_like_command(command) {
        return command.to_string();
    }
    if find_command_on_path(command, path_var).is_some() {
        return command.to_string();
    }
    if is_ruby_lsp_command(command) {
        if let Some(command_path) = find_ruby_gem_command(command, path_var) {
            return command_path.to_string_lossy().into_owned();
        }
    }

    command_path.to_string_lossy().into_owned()
}

pub(crate) fn command_available(command: &str) -> bool {
    resolve_command_path(command).map(|_| true).unwrap_or(false)
}

fn resolve_command_path(command: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH");
    resolve_command_path_with(command, path_var.as_deref())
}

fn resolve_command_path_with(command: &str, path_var: Option<&OsStr>) -> Option<PathBuf> {
    let command = command.trim();
    if command.is_empty() {
        return None;
    }

    let command_path = Path::new(command);
    if is_path_like_command(command) {
        return is_executable_file(command_path).then(|| command_path.to_path_buf());
    }

    if let Some(command_path) = find_command_on_path(command, path_var) {
        return Some(command_path);
    }

    if is_ruby_lsp_command(command) {
        return find_ruby_gem_command(command, path_var);
    }

    None
}

fn find_command_on_path(command: &str, path_var: Option<&OsStr>) -> Option<PathBuf> {
    find_commands_on_path(command, path_var).into_iter().next()
}

fn find_commands_on_path(command: &str, path_var: Option<&OsStr>) -> Vec<PathBuf> {
    let Some(path_var) = path_var else {
        return Vec::new();
    };

    let mut seen = HashSet::new();
    std::env::split_paths(path_var)
        .map(|dir| dir.join(command))
        .filter(|candidate| is_executable_file(candidate))
        .filter(|candidate| seen.insert(candidate.clone()))
        .collect()
}

fn is_ruby_lsp_command(command: &str) -> bool {
    matches!(command_name(command), "ruby-lsp" | "ruby-lsp.cmd")
}

fn find_ruby_gem_command(command: &str, path_var: Option<&OsStr>) -> Option<PathBuf> {
    find_commands_on_path("ruby", path_var)
        .into_iter()
        .filter_map(|ruby| ruby_gem_bindir(&ruby))
        .map(|bindir| bindir.join(command))
        .find(|candidate| is_executable_file(candidate))
}

fn ruby_gem_bindir(ruby: &Path) -> Option<PathBuf> {
    let output = Command::new(ruby)
        .args(["-e", "print Gem.bindir"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8(output.stdout).ok()?;
    stdout
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(PathBuf::from)
}

fn is_path_like_command(command: &str) -> bool {
    Path::new(command).is_absolute() || command.contains('/') || command.contains('\\')
}

fn command_name(command: &str) -> &str {
    Path::new(command)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(command)
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }

    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(test_name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!("nevi-{test_name}-{}-{nanos}", std::process::id()))
    }

    fn write_executable(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(path, contents).expect("write executable");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(path).expect("metadata").permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).expect("set executable bit");
        }
    }

    #[test]
    fn ruby_lsp_resolves_from_ruby_gem_bindir_when_not_on_path() {
        let root = temp_root("ruby-lsp-gem-bindir");
        let ruby_bin = root.join("ruby-bin");
        let gem_bin = root.join("gem-bin");
        let ruby = ruby_bin.join("ruby");
        let ruby_lsp = gem_bin.join("ruby-lsp");
        write_executable(
            &ruby,
            &format!("#!/bin/sh\nprintf '%s\\n' '{}'\n", gem_bin.display()),
        );
        write_executable(&ruby_lsp, "#!/bin/sh\n");

        let path_var = std::env::join_paths([ruby_bin.as_path()]).expect("join path");
        let resolved = resolve_command_path_with("ruby-lsp", Some(path_var.as_os_str()));

        let _ = fs::remove_dir_all(&root);
        assert_eq!(resolved.as_deref(), Some(ruby_lsp.as_path()));
    }

    #[test]
    fn command_on_path_uses_configured_name_for_spawn() {
        let root = temp_root("configured-command");
        let bin = root.join("bin");
        write_executable(&bin.join("ruby-lsp"), "#!/bin/sh\n");

        let path_var = std::env::join_paths([bin.as_path()]).expect("join path");
        let resolved = resolve_command_with("ruby-lsp", Some(path_var.as_os_str()));

        let _ = fs::remove_dir_all(&root);
        assert_eq!(resolved, "ruby-lsp");
    }
}
