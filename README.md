# Aegis OS

Ultra-lightweight Arch-based operating system. The live image, installer, and desktop session are built from this repository.

| Repository | Role |
| --- | --- |
| [aegis-os](https://github.com/AegisOperativeSystem/aegis-os) | Live profile, desktop, installer, package client |
| [aegis-kernel](https://github.com/AegisOperativeSystem/aegis-kernel) | `linux-aegis` 6.18 longterm package |
| [aegis-pkgs](https://github.com/AegisOperativeSystem/aegis-pkgs) | PKGBUILDs and the `x86_64` pacman repository |

The desktop is a GTK4 shell on the wlroots compositor labwc. The installer is a GTK4 front end over a tested `sfdisk` / `pacstrap` / `systemd-boot` backend. Packages stay pacman-compatible.

Read [ARCHITECTURE.md](ARCHITECTURE.md) for the layout and [docs/integration.md](docs/integration.md) for the release order.

```bash
cargo test -p aegis-common
./scripts/lint.sh
sudo ./scripts/build-iso.sh
```

Tag `v1.0.0` to build the ISO and upload it to GitHub Releases. x86_64 UEFI only. The live session starts the panel, the dock, and a first-run tour.
