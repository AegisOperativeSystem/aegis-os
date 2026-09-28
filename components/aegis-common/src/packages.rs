#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoPackage {
    pub repository: String,
    pub name: String,
    pub version: String,
    pub installed: bool,
}

pub fn is_safe_package_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() || first.is_ascii_digit() => {}
        _ => return false,
    }
    name.len() <= 128
        && name.chars().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '@' | '.' | '_' | '+' | '-')
        })
}

pub fn parse_pacman_sl(output: &str) -> Vec<RepoPackage> {
    output
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let repository = parts.next()?.to_string();
            let name = parts.next()?.to_string();
            let version = parts.next()?.to_string();
            if !is_safe_package_name(&name) {
                return None;
            }
            let installed = line.contains("[installed");
            Some(RepoPackage {
                repository,
                name,
                version,
                installed,
            })
        })
        .collect()
}

pub fn pacman_sync_args(names: &[String]) -> Result<Vec<String>, String> {
    checked_args(&["pacman", "-S", "--needed", "--noconfirm"], names)
}

pub fn pacman_remove_args(names: &[String]) -> Result<Vec<String>, String> {
    checked_args(&["pacman", "-Rns", "--noconfirm"], names)
}

fn checked_args(prefix: &[&str], names: &[String]) -> Result<Vec<String>, String> {
    if names.is_empty() {
        return Err("no packages selected".to_string());
    }
    for name in names {
        if !is_safe_package_name(name) {
            return Err(format!("refusing package name {name}"));
        }
    }
    let mut args = prefix
        .iter()
        .map(|item| (*item).to_string())
        .collect::<Vec<_>>();
    args.extend(names.iter().cloned());
    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sync_list() {
        let output = "aegis aegis-shell 0.1.0-1\naegis aegis-pkg 0.1.0-1 [installed]\n";
        let parsed = parse_pacman_sl(output);
        assert_eq!(parsed.len(), 2);
        assert!(parsed[1].installed);
        assert_eq!(parsed[0].name, "aegis-shell");
    }

    #[test]
    fn rejects_shell_metacharacters() {
        assert!(!is_safe_package_name("bash;rm"));
        assert!(pacman_sync_args(&["ok".to_string()]).is_ok());
        assert!(pacman_remove_args(&["$(reboot)".to_string()]).is_err());
    }
}
