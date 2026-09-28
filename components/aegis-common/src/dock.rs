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
        assert_eq!(favorites.len(), 3);
        assert_eq!(favorites[2].argv, ["pkexec", "aegis-installer"]);
    }

    #[test]
    fn installed_dock_keeps_daily_apps() {
        let favorites = dock_favorites(false);
        assert_eq!(
            favorites.iter().map(|item| item.id).collect::<Vec<_>>(),
            vec!["foot.desktop", "aegis-pkg.desktop"]
        );
    }
}
