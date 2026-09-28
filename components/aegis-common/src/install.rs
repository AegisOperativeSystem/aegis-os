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

pub fn sfdisk_script() -> String {
    format!("label: gpt\nsize=1GiB, type={ESP_TYPE}, name=ESP\ntype={ROOT_TYPE}, name=root\n")
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

pub fn build_actions(input: &PlanInput, kernel_package: &str) -> Result<Vec<Action>, String> {
    validate_plan(input)?;
    if kernel_package.is_empty()
        || !kernel_package
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("kernel package name is invalid".to_string());
    }
    let esp = crate::validate::partition_path(&input.disk, 1)?;
    let root = crate::validate::partition_path(&input.disk, 2)?;
    let mnt = MOUNT_POINT;
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
            stdin: Some(sfdisk_script()),
        },
        Action::Command {
            argv: vec!["udevadm".to_string(), "settle".to_string()],
            stdin: None,
        },
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
        Action::Command {
            argv: {
                let mut pacstrap = vec!["pacstrap".to_string(), "-K".to_string(), mnt.to_string()];
                pacstrap.extend(default_packages(kernel_package));
                pacstrap
            },
            stdin: None,
        },
    ];
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
            contents: "LANG=en_US.UTF-8\n".to_string(),
            mode: 0o644,
        },
        Action::Write {
            path: format!("{mnt}/etc/locale.gen"),
            contents: "en_US.UTF-8 UTF-8\n".to_string(),
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
            argv: chroot(&["bootctl", "--esp-path=/boot", "install"]),
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
        "foot".to_string(),
        "labwc".to_string(),
        "swaybg".to_string(),
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
    if !Path::new("/sys/firmware/efi").exists() {
        return Err("Aegis OS installs on UEFI firmware only".to_string());
    }
    let actions = build_actions(input, kernel_package)?;
    let root = crate::validate::partition_path(&input.disk, 2)?;
    let outcome = execute_reporting(&actions, report).and_then(|_| {
        report("writing the systemd-boot entry");
        write_boot_entry(kernel_package, &root)
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

pub fn write_boot_entry(kernel_package: &str, root_partition: &str) -> Result<(), String> {
    let uuid = read_root_uuid(root_partition)?;
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
        }
    }

    #[test]
    fn plan_starts_with_gpt_and_hides_password() {
        let actions = build_actions(&sample(), "linux").unwrap();
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
    }

    #[test]
    fn btrfs_plan_uses_mkfs_btrfs() {
        let mut input = sample();
        input.filesystem = Filesystem::Btrfs;
        let actions = build_actions(&input, "linux-aegis").unwrap();
        assert!(actions
            .iter()
            .any(|action| action.to_string().contains("mkfs.btrfs")));
        assert!(actions
            .iter()
            .any(|action| action.to_string().contains("linux-aegis")));
    }

    #[test]
    fn rejects_bad_kernel_names() {
        assert!(build_actions(&sample(), "linux;rm").is_err());
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
