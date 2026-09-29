use crate::validate::{validate_plan, Filesystem, PlanInput};
use crate::MOUNT_POINT;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

const ESP_TYPE: &str = "C12A7328-F81F-11D2-BA4B-00A0C93EC93B";
const ROOT_TYPE: &str = "0FC63DAF-8483-4772-8E79-3D69D8477DE4";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Command {
        argv: Vec<String>,
        stdin: Option<String>,
    },
    Write {
        path: String,
        contents: String,
        mode: u32,
    },
    AppendCommand {
        argv: Vec<String>,
        path: String,
    },
}

impl fmt::Display for Action {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Command { argv, stdin } => {
                write!(formatter, "{}", argv.join(" "))?;
                if stdin.is_some() {
                    write!(formatter, " <redacted>")?;
                }
                Ok(())
            }
            Self::Write { path, .. } => write!(formatter, "write {path}"),
            Self::AppendCommand { argv, path } => {
                write!(formatter, "{} >> {path}", argv.join(" "))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Firmware {
    Uefi,
    Bios,
}

pub fn sfdisk_script() -> String {
    format!("label: gpt\nsize=1GiB, type={ESP_TYPE}, name=ESP\ntype={ROOT_TYPE}, name=root\n")
}

pub fn sfdisk_bios_script() -> String {
    "label: dos\ntype=83, bootable\n".to_string()
}

pub fn bios_loader(root_uuid: &str, kernel_package: &str) -> String {
    format!(
        "DEFAULT aegis\nPROMPT 0\nTIMEOUT 30\nLABEL aegis\n  LINUX vmlinuz-{kernel_package}\n  APPEND root=UUID={root_uuid} rw quiet\n  INITRD initramfs-{kernel_package}.img\n"
    )
}

pub fn loader_entry(root_uuid: &str, kernel_package: &str) -> String {
    format!(
        "title Aegis OS\nlinux /vmlinuz-{kernel_package}\ninitrd /initramfs-{kernel_package}.img\noptions root=UUID={root_uuid} rw quiet\n"
    )
}

pub fn greetd_config() -> &'static str {
    "[terminal]\nvt = 1\n\n[default_session]\ncommand = \"tuigreet --cmd aegis-session --remember --time\"\nuser = \"greeter\"\n"
}

pub fn hosts_file(hostname: &str) -> String {
    format!("127.0.0.1 localhost\n::1 localhost\n127.0.1.1 {hostname}.localdomain {hostname}\n")
}

pub fn sudoers_dropin() -> &'static str {
    "%wheel ALL=(ALL:ALL) ALL\n"
}

pub fn build_actions(
    input: &PlanInput,
    kernel_package: &str,
    firmware: Firmware,
) -> Result<Vec<Action>, String> {
    validate_plan(input)?;
    if kernel_package.is_empty()
        || !kernel_package
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("kernel package name is invalid".to_string());
    }
    let mnt = MOUNT_POINT;
    let root = crate::validate::partition_path(
        &input.disk,
        match firmware {
            Firmware::Uefi => 2,
            Firmware::Bios => 1,
        },
    )?;
    let mkfs_root = match input.filesystem {
        Filesystem::Ext4 => vec![
            "mkfs.ext4".to_string(),
            "-F".to_string(),
            "-L".to_string(),
            "aegis".to_string(),
            root.clone(),
        ],
        Filesystem::Btrfs => vec![
            "mkfs.btrfs".to_string(),
            "-f".to_string(),
            "-L".to_string(),
            "aegis".to_string(),
            root.clone(),
        ],
    };
    let mut actions = vec![
        Action::Command {
            argv: vec![
                "sfdisk".to_string(),
                "--wipe".to_string(),
                "always".to_string(),
                "--wipe-partitions".to_string(),
                "always".to_string(),
                input.disk.clone(),
            ],
            stdin: Some(match firmware {
                Firmware::Uefi => sfdisk_script(),
                Firmware::Bios => sfdisk_bios_script(),
            }),
        },
        Action::Command {
            argv: vec!["udevadm".to_string(), "settle".to_string()],
            stdin: None,
        },
    ];
    if firmware == Firmware::Uefi {
        let esp = crate::validate::partition_path(&input.disk, 1)?;
        actions.extend([
            Action::Command {
                argv: vec![
                    "mkfs.fat".to_string(),
                    "-F32".to_string(),
                    "-n".to_string(),
                    "AEGIS_ESP".to_string(),
                    esp.clone(),
                ],
                stdin: None,
            },
            Action::Command {
                argv: mkfs_root,
                stdin: None,
            },
            Action::Command {
                argv: vec!["mount".to_string(), root.clone(), mnt.to_string()],
                stdin: None,
            },
            Action::Command {
                argv: vec!["mkdir".to_string(), "-p".to_string(), format!("{mnt}/boot")],
                stdin: None,
            },
            Action::Command {
                argv: vec!["mount".to_string(), esp, format!("{mnt}/boot")],
                stdin: None,
            },
        ]);
    } else {
        actions.extend([
            Action::Command {
                argv: mkfs_root,
                stdin: None,
            },
            Action::Command {
                argv: vec!["mount".to_string(), root.clone(), mnt.to_string()],
                stdin: None,
            },
            Action::Command {
                argv: vec!["mkdir".to_string(), "-p".to_string(), format!("{mnt}/boot")],
                stdin: None,
            },
        ]);
    }
    if input.swap_gib > 0 {
        let swap = format!("{mnt}/swapfile");
        let size = format!("{}G", input.swap_gib);
        actions.extend([
            Action::Command {
                argv: vec![
                    "fallocate".to_string(),
                    "-l".to_string(),
                    size,
                    swap.clone(),
                ],
                stdin: None,
            },
            Action::Command {
                argv: vec!["chmod".to_string(), "600".to_string(), swap.clone()],
                stdin: None,
            },
            Action::Command {
                argv: vec!["mkswap".to_string(), swap.clone()],
                stdin: None,
            },
            Action::Command {
                argv: vec!["swapon".to_string(), swap],
                stdin: None,
            },
        ]);
    }
    actions.push(Action::Command {
        argv: {
            let mut pacstrap = vec!["pacstrap".to_string(), "-K".to_string(), mnt.to_string()];
            pacstrap.extend(default_packages(kernel_package));
            pacstrap
        },
        stdin: None,
    });
    actions.push(Action::AppendCommand {
        argv: vec!["genfstab".to_string(), "-U".to_string(), mnt.to_string()],
        path: format!("{mnt}/etc/fstab"),
    });
    actions.extend([
        Action::Write {
            path: format!("{mnt}/etc/hostname"),
            contents: format!("{hostname}\n", hostname = input.hostname),
            mode: 0o644,
        },
        Action::Write {
            path: format!("{mnt}/etc/hosts"),
            contents: hosts_file(&input.hostname),
            mode: 0o644,
        },
        Action::Write {
            path: format!("{mnt}/etc/locale.conf"),
            contents: format!("LANG={}\n", input.locale),
            mode: 0o644,
        },
        Action::Write {
            path: format!("{mnt}/etc/locale.gen"),
            contents: format!("{} UTF-8\n", input.locale),
            mode: 0o644,
        },
        Action::Write {
            path: format!("{mnt}/etc/vconsole.conf"),
            contents: format!("KEYMAP={}\nFONT=ter-u16n\n", input.keymap),
            mode: 0o644,
        },
        Action::Write {
            path: format!("{mnt}/etc/sudoers.d/wheel"),
            contents: sudoers_dropin().to_string(),
            mode: 0o440,
        },
        Action::Write {
            path: format!("{mnt}/etc/greetd/config.toml"),
            contents: greetd_config().to_string(),
            mode: 0o644,
        },
        Action::Command {
            argv: chroot(&[
                "ln",
                "-sf",
                &format!("/usr/share/zoneinfo/{}", input.timezone),
                "/etc/localtime",
            ]),
            stdin: None,
        },
        Action::Command {
            argv: chroot(&["hwclock", "--systohc"]),
            stdin: None,
        },
        Action::Command {
            argv: chroot(&["locale-gen"]),
            stdin: None,
        },
        Action::Command {
            argv: chroot(&[
                "useradd",
                "-m",
                "-G",
                "wheel,video,audio,input,storage",
                "-s",
                "/bin/bash",
                &input.username,
            ]),
            stdin: None,
        },
        Action::Write {
            path: format!("{mnt}/home/{}/.config/labwc/environment", input.username),
            contents: format!(
                "WLR_RENDERER=pixman\nWLR_NO_HARDWARE_CURSORS=1\nXKB_DEFAULT_LAYOUT={}\nXDG_CURRENT_DESKTOP=Aegis\nFOOT_CONFIG=/usr/share/aegis/foot.ini\n",
                input.keymap
            ),
            mode: 0o644,
        },
        Action::Command {
            argv: chroot(&[
                "chown",
                "-R",
                &format!("{}:{}", input.username, input.username),
                &format!("/home/{}/.config", input.username),
            ]),
            stdin: None,
        },
        Action::Command {
            argv: chroot(&[
                "useradd",
                "-M",
                "-d",
                "/var/lib/greeter",
                "-s",
                "/usr/bin/nologin",
                "greeter",
            ]),
            stdin: None,
        },
        Action::Command {
            argv: chroot(&["chpasswd"]),
            stdin: Some(format!("{}:{}\n", input.username, input.password)),
        },
        Action::Command {
            argv: chroot(&["passwd", "-l", "root"]),
            stdin: None,
        },
        Action::Command {
            argv: chroot(&["mkinitcpio", "-P"]),
            stdin: None,
        },
        Action::Command {
            argv: chroot(&[
                "systemctl",
                "enable",
                "NetworkManager.service",
                "greetd.service",
            ]),
            stdin: None,
        },
    ]);
    match firmware {
        Firmware::Uefi => {
            actions.push(Action::Command {
                argv: chroot(&["bootctl", "--esp-path=/boot", "install"]),
                stdin: None,
            });
        }
        Firmware::Bios => {
            actions.push(Action::Command {
                argv: vec![
                    "extlinux".to_string(),
                    "--install".to_string(),
                    format!("{mnt}/boot"),
                ],
                stdin: None,
            });
            actions.push(Action::Command {
                argv: vec![
                    "dd".to_string(),
                    "bs=440".to_string(),
                    "count=1".to_string(),
                    "conv=notrunc".to_string(),
                    "if=/usr/lib/syslinux/bios/mbr.bin".to_string(),
                    format!("of={}", input.disk),
                ],
                stdin: None,
            });
        }
    }
    Ok(actions)
}

pub fn default_packages(kernel_package: &str) -> Vec<String> {
    let packages = vec![
        "base".to_string(),
        kernel_package.to_string(),
        "linux-firmware".to_string(),
        "intel-ucode".to_string(),
        "amd-ucode".to_string(),
        "mkinitcpio".to_string(),
        "sudo".to_string(),
        "networkmanager".to_string(),
        "dosfstools".to_string(),
        "e2fsprogs".to_string(),
        "btrfs-progs".to_string(),
        "fastfetch".to_string(),
        "foot".to_string(),
        "pcmanfm".to_string(),
        "mousepad".to_string(),
        "pavucontrol".to_string(),
        "xdg-user-dirs".to_string(),
        "labwc".to_string(),
        "swaybg".to_string(),
        "terminus-font".to_string(),
        "ttf-dejavu".to_string(),
        "gtk4".to_string(),
        "gtk4-layer-shell".to_string(),
        "greetd".to_string(),
        "greetd-tuigreet".to_string(),
        "pipewire".to_string(),
        "pipewire-alsa".to_string(),
        "pipewire-pulse".to_string(),
        "wireplumber".to_string(),
        "xdg-desktop-portal".to_string(),
        "xdg-desktop-portal-wlr".to_string(),
        "polkit".to_string(),
        "polkit-gnome".to_string(),
        "rtkit".to_string(),
        "noto-fonts".to_string(),
        "adwaita-icon-theme".to_string(),
        "hicolor-icon-theme".to_string(),
        "aegis-session".to_string(),
        "aegis-shell".to_string(),
        "aegis-dock".to_string(),
        "aegis-tour".to_string(),
        "aegis-pkg".to_string(),
        "aegis-mirrorlist".to_string(),
    ];
    packages
}

fn chroot(args: &[&str]) -> Vec<String> {
    let mut argv = vec!["arch-chroot".to_string(), MOUNT_POINT.to_string()];
    argv.extend(args.iter().map(|arg| (*arg).to_string()));
    argv
}

pub fn read_root_uuid(root_partition: &str) -> Result<String, String> {
    let output = Command::new("blkid")
        .args(["-s", "UUID", "-o", "value", root_partition])
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let uuid = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if uuid.is_empty() {
        return Err("blkid returned an empty UUID".to_string());
    }
    Ok(uuid)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockDevice {
    pub path: String,
    pub size_bytes: u64,
}

pub fn parse_lsblk(output: &str) -> Vec<BlockDevice> {
    output
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let name = parts.next()?;
            let size = parts.next()?.parse::<u64>().ok()?;
            let kind = parts.next()?;
            if kind != "disk" || size < 8 * 1024 * 1024 * 1024 {
                return None;
            }
            let path = format!("/dev/{name}");
            crate::validate::validate_disk(&path).ok()?;
            Some(BlockDevice {
                path,
                size_bytes: size,
            })
        })
        .collect()
}

pub fn format_size(bytes: u64) -> String {
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    format!("{:.1} GiB", bytes as f64 / GIB)
}

pub fn execute(actions: &[Action]) -> Result<(), String> {
    execute_reporting(actions, &mut |_| {})
}

pub fn execute_reporting(actions: &[Action], report: &mut dyn FnMut(&str)) -> Result<(), String> {
    if !is_root() {
        return Err("the installer backend must run as root".to_string());
    }
    for action in actions {
        report(&action.to_string());
        run_action(action)?;
    }
    Ok(())
}

pub fn install_system(
    input: &PlanInput,
    kernel_package: &str,
    report: &mut dyn FnMut(&str),
) -> Result<(), String> {
    let firmware = if Path::new("/sys/firmware/efi").is_dir() {
        Firmware::Uefi
    } else {
        Firmware::Bios
    };
    report(match firmware {
        Firmware::Uefi => "firmware uefi",
        Firmware::Bios => "firmware bios",
    });
    let actions = build_actions(input, kernel_package, firmware)?;
    let root = crate::validate::partition_path(
        &input.disk,
        match firmware {
            Firmware::Uefi => 2,
            Firmware::Bios => 1,
        },
    )?;
    let outcome = execute_reporting(&actions, report).and_then(|_| {
        report("writing the boot entry");
        write_boot_entry(kernel_package, &root, firmware)
    });
    let _ = Command::new("umount").args(["-R", MOUNT_POINT]).status();
    outcome
}

fn is_root() -> bool {
    #[cfg(unix)]
    {
        #[link(name = "c")]
        unsafe extern "C" {
            fn geteuid() -> u32;
        }
        unsafe { geteuid() == 0 }
    }
    #[cfg(not(unix))]
    {
        false
    }
}

fn run_action(action: &Action) -> Result<(), String> {
    match action {
        Action::Command { argv, stdin } => run_command(argv, stdin.as_deref(), None),
        Action::AppendCommand { argv, path } => run_command(argv, None, Some(path)),
        Action::Write {
            path,
            contents,
            mode,
        } => write_file(path, contents, *mode),
    }
}

fn run_command(
    argv: &[String],
    stdin: Option<&str>,
    append_to: Option<&str>,
) -> Result<(), String> {
    let (program, args) = argv
        .split_first()
        .ok_or_else(|| "empty command".to_string())?;
    let mut command = Command::new(program);
    command.args(args).stderr(Stdio::piped());
    if stdin.is_some() {
        command.stdin(Stdio::piped());
    }
    if append_to.is_some() {
        command.stdout(Stdio::piped());
    }
    let mut child = command.spawn().map_err(|err| format!("{program}: {err}"))?;
    if let Some(text) = stdin {
        let mut handle = child.stdin.take().ok_or("missing stdin")?;
        handle
            .write_all(text.as_bytes())
            .map_err(|err| err.to_string())?;
    }
    let output = child.wait_with_output().map_err(|err| err.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "{program} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    if let Some(path) = append_to {
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|err| err.to_string())?;
        file.write_all(&output.stdout)
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn write_file(path: &str, contents: &str, mode: u32) -> Result<(), String> {
    if let Some(parent) = Path::new(path).parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    fs::write(path, contents).map_err(|err| err.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .map_err(|err| err.to_string())?;
    }
    #[cfg(not(unix))]
    {
        let _ = mode;
    }
    Ok(())
}

pub fn write_boot_entry(
    kernel_package: &str,
    root_partition: &str,
    firmware: Firmware,
) -> Result<(), String> {
    let uuid = read_root_uuid(root_partition)?;
    match firmware {
        Firmware::Uefi => {
            write_file(
                &format!("{MOUNT_POINT}/boot/loader/loader.conf"),
                "default aegis.conf\ntimeout 3\nconsole-mode auto\n",
                0o644,
            )?;
            write_file(
                &format!("{MOUNT_POINT}/boot/loader/entries/aegis.conf"),
                &loader_entry(&uuid, kernel_package),
                0o644,
            )
        }
        Firmware::Bios => write_file(
            &format!("{MOUNT_POINT}/boot/syslinux.cfg"),
            &bios_loader(&uuid, kernel_package),
            0o644,
        ),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Volume {
    pub name: String,
    pub label: String,
    pub filesystem: String,
    pub parent: String,
}

pub fn parse_volumes(output: &str) -> Vec<Volume> {
    output
        .lines()
        .filter_map(|line| {
            let fields = line
                .split_whitespace()
                .filter_map(|field| {
                    let (key, value) = field.split_once('=')?;
                    Some((key, value.trim_matches('"')))
                })
                .collect::<std::collections::HashMap<_, _>>();
            Some(Volume {
                name: fields.get("NAME")?.to_string(),
                label: fields.get("LABEL").unwrap_or(&"").to_string(),
                filesystem: fields.get("FSTYPE").unwrap_or(&"").to_string(),
                parent: fields.get("PKNAME").unwrap_or(&"").to_string(),
            })
        })
        .collect()
}

pub fn repair_actions(
    disk: &str,
    root: &str,
    filesystem: Filesystem,
    esp: Option<&str>,
    firmware: Firmware,
) -> Result<Vec<Action>, String> {
    crate::validate::validate_disk(disk)?;
    if !is_child_partition(disk, root) {
        return Err("repair root must be a partition of the selected disk".to_string());
    }
    let check = match filesystem {
        Filesystem::Ext4 => vec![
            "sh".to_string(),
            "-c".to_string(),
            "e2fsck -f -y \"$1\"; code=$?; [ \"$code\" -le 1 ]".to_string(),
            "e2fsck".to_string(),
            root.to_string(),
        ],
        Filesystem::Btrfs => vec!["btrfs".to_string(), "check".to_string(), root.to_string()],
    };
    let mut actions = vec![
        Action::Command {
            argv: check,
            stdin: None,
        },
        Action::Command {
            argv: vec![
                "mkdir".to_string(),
                "-p".to_string(),
                MOUNT_POINT.to_string(),
            ],
            stdin: None,
        },
        Action::Command {
            argv: vec![
                "mount".to_string(),
                root.to_string(),
                MOUNT_POINT.to_string(),
            ],
            stdin: None,
        },
    ];
    match firmware {
        Firmware::Uefi => {
            let esp = esp.ok_or_else(|| "this disk has no EFI system partition".to_string())?;
            if !is_child_partition(disk, esp) {
                return Err("EFI partition is not on the selected disk".to_string());
            }
            actions.push(Action::Command {
                argv: vec![
                    "mount".to_string(),
                    esp.to_string(),
                    format!("{MOUNT_POINT}/boot"),
                ],
                stdin: None,
            });
            actions.push(Action::Command {
                argv: chroot(&["bootctl", "--esp-path=/boot", "install"]),
                stdin: None,
            });
        }
        Firmware::Bios => {
            actions.push(Action::Command {
                argv: vec![
                    "extlinux".to_string(),
                    "--install".to_string(),
                    format!("{MOUNT_POINT}/boot"),
                ],
                stdin: None,
            });
            actions.push(Action::Command {
                argv: vec![
                    "dd".to_string(),
                    "bs=440".to_string(),
                    "count=1".to_string(),
                    "conv=notrunc".to_string(),
                    "if=/usr/lib/syslinux/bios/mbr.bin".to_string(),
                    format!("of={disk}"),
                ],
                stdin: None,
            });
        }
    }
    Ok(actions)
}

fn is_child_partition(disk: &str, node: &str) -> bool {
    let Some(rest) = node.strip_prefix(disk) else {
        return false;
    };
    let rest = if disk.ends_with(|c: char| c.is_ascii_digit()) {
        let Some(rest) = rest.strip_prefix('p') else {
            return false;
        };
        rest
    } else {
        rest
    };
    !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit())
}

pub fn repair_system(disk: &str, report: &mut dyn FnMut(&str)) -> Result<(), String> {
    crate::validate::validate_disk(disk)?;
    let firmware = if Path::new("/sys/firmware/efi").is_dir() {
        Firmware::Uefi
    } else {
        Firmware::Bios
    };
    let output = Command::new("lsblk")
        .args(["-P", "-o", "NAME,LABEL,FSTYPE,PKNAME"])
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let volumes = parse_volumes(&String::from_utf8_lossy(&output.stdout));
    let parent = disk.trim_start_matches("/dev/");
    let root = volumes.iter().find(|volume| {
        volume.parent == parent
            && volume.label == "aegis"
            && matches!(volume.filesystem.as_str(), "ext4" | "btrfs")
    });
    let Some(root) = root else {
        return Err("no Aegis root with label aegis was found on that disk".to_string());
    };
    let filesystem = match root.filesystem.as_str() {
        "btrfs" => Filesystem::Btrfs,
        _ => Filesystem::Ext4,
    };
    let esp = volumes
        .iter()
        .find(|volume| volume.parent == parent && volume.filesystem == "vfat");
    let root_path = format!("/dev/{}", root.name);
    let actions = repair_actions(
        disk,
        &root_path,
        filesystem,
        esp.map(|volume| format!("/dev/{}", volume.name)).as_deref(),
        firmware,
    )?;
    let outcome = execute_reporting(&actions, report).and_then(|_| {
        report("writing the boot entry");
        write_boot_entry("linux", &root_path, firmware)
    });
    let _ = Command::new("umount").args(["-R", MOUNT_POINT]).status();
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> PlanInput {
        PlanInput {
            disk: "/dev/vda".to_string(),
            hostname: "aegis".to_string(),
            username: "leo".to_string(),
            password: "correcthorsebattery".to_string(),
            timezone: "Europe/Rome".to_string(),
            filesystem: Filesystem::Ext4,
            locale: "en_US.UTF-8".to_string(),
            keymap: "us".to_string(),
            swap_gib: 0,
        }
    }

    #[test]
    fn plan_starts_with_gpt_and_hides_password() {
        let actions = build_actions(&sample(), "linux", Firmware::Uefi).unwrap();
        let rendered = actions
            .iter()
            .map(|action| action.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("sfdisk --wipe always"));
        assert!(rendered.contains("chpasswd <redacted>"));
        assert!(!rendered.contains("correcthorsebattery"));
        assert!(rendered.contains("mkfs.ext4 -F -L aegis /dev/vda2"));
        assert!(rendered.contains("/dev/vda1"));
        assert!(actions.iter().any(|action| matches!(
            action,
            Action::Write { path, contents, .. }
                if path.ends_with("/etc/locale.gen") && contents.contains("en_US.UTF-8")
        )));
    }

    #[test]
    fn swap_plan_creates_a_swapfile() {
        let mut input = sample();
        input.swap_gib = 4;
        input.locale = "it_IT.UTF-8".to_string();
        input.keymap = "it".to_string();
        let actions = build_actions(&input, "linux", Firmware::Uefi).unwrap();
        let rendered = actions
            .iter()
            .map(|action| action.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("fallocate -l 4G /mnt/swapfile"));
        assert!(rendered.contains("mkswap /mnt/swapfile"));
        assert!(rendered.contains("swapon /mnt/swapfile"));
        assert!(actions.iter().any(|action| matches!(
            action,
            Action::Write { contents, .. } if contents.contains("KEYMAP=it")
        )));
        assert!(actions.iter().any(|action| matches!(
            action,
            Action::Write { contents, .. } if contents.contains("LANG=it_IT.UTF-8")
        )));
    }

    #[test]
    fn btrfs_plan_uses_mkfs_btrfs() {
        let mut input = sample();
        input.filesystem = Filesystem::Btrfs;
        let actions = build_actions(&input, "linux-aegis", Firmware::Uefi).unwrap();
        assert!(actions
            .iter()
            .any(|action| action.to_string().contains("mkfs.btrfs")));
        assert!(actions
            .iter()
            .any(|action| action.to_string().contains("linux-aegis")));
    }

    #[test]
    fn rejects_bad_kernel_names() {
        assert!(build_actions(&sample(), "linux;rm", Firmware::Uefi).is_err());
    }

    #[test]
    fn repair_checks_the_labeled_root_and_rewrites_bios_boot() {
        let rendered = repair_actions(
            "/dev/vda",
            "/dev/vda1",
            Filesystem::Ext4,
            None,
            Firmware::Bios,
        )
        .unwrap()
        .iter()
        .map(|action| action.to_string())
        .collect::<Vec<_>>()
        .join("\n");
        assert!(rendered.contains("e2fsck -f -y"));
        assert!(rendered.contains("/dev/vda1"));
        assert!(rendered.contains("extlinux --install /mnt/boot"));
        assert!(rendered.contains("of=/dev/vda"));
    }

    #[test]
    fn volumes_keep_an_empty_label() {
        let volumes = parse_volumes(
            "NAME=\"vda1\" LABEL=\"\" FSTYPE=\"vfat\" PKNAME=\"vda\"\nNAME=\"vda2\" LABEL=\"aegis\" FSTYPE=\"ext4\" PKNAME=\"vda\"\n",
        );
        assert_eq!(volumes[1].label, "aegis");
        assert_eq!(volumes[0].filesystem, "vfat");
    }

    #[test]
    fn bios_plan_installs_syslinux() {
        let actions = build_actions(&sample(), "linux", Firmware::Bios).unwrap();
        let rendered = actions
            .iter()
            .map(|action| action.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(actions.iter().any(|action| matches!(
            action,
            Action::Command { stdin: Some(script), .. } if script.contains("label: dos")
        )));
        assert!(rendered.contains("extlinux --install /mnt/boot"));
        assert!(rendered.contains(
            "dd bs=440 count=1 conv=notrunc if=/usr/lib/syslinux/bios/mbr.bin of=/dev/vda"
        ));
        assert!(rendered.contains("mkfs.ext4 -F -L aegis /dev/vda1"));
        assert!(!rendered.contains("bootctl"));
        assert!(!rendered.contains("mkfs.fat"));
    }

    #[test]
    fn parses_lsblk_and_skips_small_or_optical() {
        let output =
            "sda 34359738368 disk\nsr0 1073741312 rom\nnvme0n1 1073741824 disk\nloop0 4096 loop\n";
        let devices = parse_lsblk(output);
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].path, "/dev/sda");
        assert_eq!(format_size(32 * 1024 * 1024 * 1024), "32.0 GiB");
    }
}
