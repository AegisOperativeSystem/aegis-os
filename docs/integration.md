# Integration

Release order is fixed: keyring, kernel, package repository, ISO.

## 1. Create the signing key

On a trusted machine:

```bash
gpg --batch --pinentry-mode loopback --passphrase '' \
  --quick-gen-key 'Aegis OS Package Signing <package-signing@users.noreply.github.com>' ed25519 sign 2y
gpg --list-secret-keys --keyid-format LONG
```

Export the private key and store it as the `AEGIS_GPG_PRIVATE_KEY` secret on `aegis-kernel` and `aegis-pkgs`. Store the passphrase as `AEGIS_GPG_PASSPHRASE` when the key has one. Create a fine-grained or classic PAT that can publish releases in this organization and store it as `AEGIS_REPO_TOKEN` on `aegis-kernel`.

```bash
gpg --armor --export-secret-keys KEYID
cd aegis-pkgs
./scripts/export-keyring.sh KEYID
```

Commit `keys/aegis.gpg`, `keys/aegis-trusted`, and `keys/aegis-revoked`. Do not commit the private key. Keep an offline backup of the secret key.

## 2. Publish the kernel

```bash
cd aegis-kernel
git tag v0.1.0
git push origin v0.1.0
```

The kernel workflow compiles `linux-aegis`, signs it when the secret exists, uploads the package to the kernel release, and dispatches `index-kernel` to `aegis-pkgs`.

Select `balanced`, `performance`, or `powersave` from the manual workflow. The package default is `balanced`. Boot parameters, set in the installed loader entry when you want a profile other than the compiled default governor:

```text
preempt=voluntary
preempt=full
preempt=none
```

## 3. Publish the package repository

A push that changes `aegis-pkgs/packages` builds every PKGBUILD and uploads the database to the rolling release tagged `x86_64`.

```text
https://github.com/AegisOperativeSystem/aegis-pkgs/releases/download/x86_64
```

Pacman reads `aegis.db` from that directory. The publisher copies the zstd database onto that filename because a GitHub release cannot store the `repo-add` symlink.

Confirm:

```bash
curl -I https://github.com/AegisOperativeSystem/aegis-pkgs/releases/download/x86_64/aegis.db
```

## 4. Turn signature checking on

After `keys/aegis.gpg` is in the `aegis-keyring` package and that package is installed on the image:

```bash
pacman-key --populate aegis
install -m644 /usr/share/aegis/pacman/aegis-signed.conf /etc/pacman.conf.d/aegis.conf
```

Replace the `Optional TrustAll` block in `/etc/pacman.conf` with:

```ini
[aegis]
SigLevel = Required
Include = /etc/pacman.d/aegis-mirrorlist
```

## 5. Build the ISO

The ISO build clones or reuses `aegis-pkgs`, builds a local pacman database, then runs `mkarchiso`.

```bash
cd aegis-os
sudo ./scripts/build-iso.sh
```

GitHub does the same job on tag `v1.0.1` and uploads `out/*.iso` to the release. The image is x86_64 UEFI. There is no BIOS path.

The first ISO installs Arch `linux` so it can be built before `linux-aegis` exists. After the kernel package is in the `x86_64` release:

1. In `profiles/live/packages.x86_64`, replace `linux` with `linux-aegis`.
2. In `profiles/live/efiboot/loader/entries/01-aegis.conf`, replace `vmlinuz-linux` and `initramfs-linux.img` with the `linux-aegis` names.
3. Export `AEGIS_KERNEL_PKG=linux-aegis` for the live installer, or change the installer default in `components/aegis-installer`.

## 6. Install

Boot the ISO. The shell starts the installer. Choose the whole disk, account, timezone, and filesystem. Type the disk name (`vda`, `nvme0n1`) and install. The target layout is a 1 GiB FAT32 ESP mounted at `/boot` and a root filesystem, ext4 or btrfs. Root login is locked. The created user is in `wheel`. Reboot and sign in through `tuigreet`.

## 7. Day to day

`aegis-pkg` refreshes `pacman -Sl aegis` and installs or removes the selected package through `pkexec`. The shell launches desktop files from `/usr/share/applications`. Labwc keys are Super+Enter for `foot` and Super+Q to close a window.
