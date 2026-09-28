#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopEntry {
    pub name: String,
    pub exec: String,
    pub icon: String,
    pub terminal: bool,
    pub no_display: bool,
}

pub fn parse_desktop_entry(text: &str) -> Option<DesktopEntry> {
    let mut in_entry = false;
    let mut name = None;
    let mut exec = None;
    let mut icon = String::new();
    let mut terminal = false;
    let mut no_display = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry || line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key {
            "Name" if name.is_none() => name = Some(value.trim().to_string()),
            "Exec" => exec = Some(clean_exec(value)),
            "Icon" if icon.is_empty() => icon = value.trim().to_string(),
            "Terminal" => terminal = value.trim() == "true",
            "NoDisplay" | "Hidden" => no_display = value.trim() == "true",
            _ => {}
        }
    }
    Some(DesktopEntry {
        name: name?,
        exec: exec.filter(|item| !item.is_empty())?,
        icon,
        terminal,
        no_display,
    })
}

fn clean_exec(raw: &str) -> String {
    raw.split_whitespace()
        .filter(|token| !token.starts_with('%'))
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn command_argv(entry: &DesktopEntry) -> Vec<String> {
    let mut argv = Vec::new();
    if entry.terminal {
        argv.extend(["foot".to_string(), "-e".to_string()]);
    }
    argv.extend(split_command(&entry.exec));
    argv
}

fn split_command(command: &str) -> Vec<String> {
    command
        .split_whitespace()
        .map(|part| part.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_desktop_file() {
        let text =
            "[Desktop Entry]\nName=Files\nExec=foot -e sh %f\nTerminal=false\nNoDisplay=false\n";
        let entry = parse_desktop_entry(text).unwrap();
        assert_eq!(entry.name, "Files");
        assert_eq!(entry.exec, "foot -e sh");
        assert!(entry.icon.is_empty());
        assert!(!entry.terminal);
    }

    #[test]
    fn wraps_terminal_apps() {
        let entry = DesktopEntry {
            name: "Htop".to_string(),
            exec: "htop".to_string(),
            icon: "htop".to_string(),
            terminal: true,
            no_display: false,
        };
        assert_eq!(
            command_argv(&entry),
            vec!["foot".to_string(), "-e".to_string(), "htop".to_string()]
        );
    }
}
