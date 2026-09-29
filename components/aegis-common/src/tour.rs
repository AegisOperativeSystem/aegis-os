use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TourSlide {
    pub kicker: &'static str,
    pub title: &'static str,
    pub body: &'static str,
}

pub fn tour_stamp_path(home: &Path) -> PathBuf {
    home.join(".config/aegis/tour-done")
}

pub fn tour_is_done(home: &Path) -> bool {
    tour_stamp_path(home).is_file()
}

pub fn mark_tour_done(home: &Path) -> io::Result<()> {
    let path = tour_stamp_path(home);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, "1\n")
}

pub fn tour_slides(live: bool) -> Vec<TourSlide> {
    let mut slides = vec![
        TourSlide {
            kicker: "Welcome",
            title: "This is Aegis OS",
            body: "A lightweight Arch desktop. The top bar holds the clock and power controls. The dock at the bottom opens your applications.",
        },
        TourSlide {
            kicker: "Dock",
            title: "Applications sit in the dock",
            body: "Firefox, Files, the text editor, the terminal, and Packages are pinned. The grid at the right end opens every installed application.",
        },
        TourSlide {
            kicker: "Keyboard",
            title: "Two shortcuts are enough to start",
            body: "Super+Enter opens the terminal. Super+Q closes the focused window. Alt+F4 closes it as well.",
        },
    ];
    if live {
        slides.push(TourSlide {
            kicker: "Install",
            title: "The installer opens when this tour ends",
            body: "Choose a whole disk of at least 8 GiB and type its name to confirm. The installer creates a 1 GiB EFI partition and an ext4 or btrfs root. Root login stays locked.",
        });
    } else {
        slides.push(TourSlide {
            kicker: "Software",
            title: "Pacman stays the package manager",
            body: "Open Packages in the dock to install and remove software from the Aegis repository. The terminal runs the same updates with pacman.",
        });
    }
    slides.push(TourSlide {
        kicker: "Ready",
        title: "The desktop is yours",
        body: "You can open this tour again from the application grid. The next time you sign in, it stays out of the way.",
    });
    slides
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_tour_mentions_the_installer() {
        let slides = tour_slides(true);
        assert_eq!(slides.len(), 5);
        assert_eq!(slides[3].kicker, "Install");
    }

    #[test]
    fn stamp_is_created_once() {
        let home = std::env::temp_dir().join(format!("aegis-tour-{}", std::process::id()));
        let _ = fs::remove_dir_all(&home);
        assert!(!tour_is_done(&home));
        mark_tour_done(&home).unwrap();
        assert!(tour_is_done(&home));
        let _ = fs::remove_dir_all(&home);
    }
}
