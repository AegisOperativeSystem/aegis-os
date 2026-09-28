# Architecture

Aegis OS is three repositories. Each one has a single release artifact.

```text
aegis-os
  components/aegis-common     pure install plan, validation, package parsing
  components/aegis-shell      GTK4 layer-shell panel and launcher
  components/aegis-installer  GTK4 live installer
  components/aegis-pkg        GTK4 client for the aegis pacman repo
  session/                    labwc, theme, foot, Wayland session
  profiles/live/              mkarchiso profile (UEFI systemd-boot)
  scripts/build-iso.sh        local repo, then mkarchiso
  .github/workflows/iso.yml   tag vX.Y.Z uploads the .iso

aegis-kernel
  config/fragments/           mainline kconfig overrides, no out-of-tree schedulers
  scripts/prepare-config.sh   Arch config + fragment + olddefconfig
  PKGBUILD                    linux-aegis and linux-aegis-headers
  .github/workflows/release.yml

aegis-pkgs
  packages/                   PKGBUILDs
  scripts/build-all.sh        makepkg + repo-add
  scripts/publish-repo.sh     rolling GitHub Release tag x86_64
  .github/workflows/repo.yml
```

## Boot and session

The ISO boots with `uefi.systemd-boot` only. `aegis-live-setup` creates the passwordless `live` user and greetd starts `aegis-session` as that user. The session runs labwc. Labwc autostart paints the background, starts `aegis-shell`, and on the live image runs `pkexec aegis-installer`. A polkit rule lets `live` install without a password. The installed system uses `tuigreet` and does not autologin.

## Installer

`aegis-common` builds the action list: GPT, 1 GiB ESP, root, `pacstrap`, fstab, locale, user, locked root, `bootctl`, mkinitcpio, NetworkManager, greetd. The GUI requires the operator to type the disk name. The backend refuses anything that is not a whole `/dev/sdX`, `/dev/vdX`, `/dev/nvmeXnY`, or `/dev/mmcblkN`, and it refuses firmware that is not UEFI. Passwords are passed to `chpasswd` on stdin and are redacted from logs.

The first images install the Arch `linux` package. After `linux-aegis` is in the pacman repository, set `AEGIS_KERNEL_PKG=linux-aegis`.

## Kernel

The package tracks longterm 6.18, currently 6.18.54. The base config is the Arch `linux` config. Fragments only override preempt, hz, cpufreq, energy model, zswap, zstd, and debug info. Profiles:

| Profile | Default governor | Preempt cmdline | Hz |
| --- | --- | --- | --- |
| balanced | schedutil | `preempt=voluntary` | 1000 |
| performance | performance | `preempt=full` | 1000 |
| powersave | powersave | `preempt=none` | 250 |

CPU mitigations stay at the upstream defaults. `mitigations=off` is an unsupported local override.

## Packages

`aegis-pkg` is a pacman front end. It accepts only package names that match the pacman character set and runs `pkexec pacman`. The repository URL is:

```text
Server = https://github.com/AegisOperativeSystem/aegis-pkgs/releases/download/x86_64
```

`aegis-mirrorlist` appends that block on install. Signature policy starts as `Optional TrustAll` until `aegis-keyring` is populated. The signed snippet ships at `/usr/share/aegis/pacman/aegis-signed.conf`.

## CI

Lint workflows run on every pull request. ISO upload, kernel compilation, and repository publication run on `vX.Y.Z` tags or `workflow_dispatch`. Kernel config checks do not compile the kernel. The release workflow does.
