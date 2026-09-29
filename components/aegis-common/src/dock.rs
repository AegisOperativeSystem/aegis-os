#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockFavorite {
    pub id: &'static str,
    pub label: &'static str,
    pub icon: &'static str,
    pub argv: &'static [&'static str],
}

pub fn dock_favorites(live: bool) -> Vec<DockFavorite> {
    let mut favorites = vec![
        DockFavorite {
            id: "firefox.desktop",
            label: "Firefox",
            icon: "web-browser",
            argv: &["firefox"],
        },
        DockFavorite {
            id: "org.gnome.Nautilus.desktop",
            label: "Files",
            icon: "system-file-manager",
            argv: &["nautilus"],
        },
        DockFavorite {
            id: "org.gnome.TextEditor.desktop",
            label: "Text Editor",
            icon: "text-editor",
            argv: &["gnome-text-editor"],
        },
        DockFavorite {
            id: "foot.desktop",
            label: "Terminal",
            icon: "utilities-terminal",
            argv: &["foot"],
        },
        DockFavorite {
            id: "aegis-pkg.desktop",
            label: "Packages",
            icon: "system-software-install",
            argv: &["aegis-pkg"],
        },
    ];
    if live {
        favorites.push(DockFavorite {
            id: "aegis-installer.desktop",
            label: "Install",
            icon: "drive-harddisk",
            argv: &["pkexec", "aegis-installer"],
        });
    }
    favorites
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_dock_pins_the_installer() {
        let favorites = dock_favorites(true);
        assert_eq!(favorites.len(), 6);
        assert_eq!(favorites[5].argv, ["pkexec", "aegis-installer"]);
    }

    #[test]
    fn installed_dock_keeps_daily_apps() {
        let favorites = dock_favorites(false);
        assert_eq!(
            favorites.iter().map(|item| item.id).collect::<Vec<_>>(),
            vec![
                "firefox.desktop",
                "org.gnome.Nautilus.desktop",
                "org.gnome.TextEditor.desktop",
                "foot.desktop",
                "aegis-pkg.desktop",
            ]
        );
    }
}
