use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filesystem {
    Ext4,
    Btrfs,
}

impl Filesystem {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ext4 => "ext4",
            Self::Btrfs => "btrfs",
        }
    }
}

impl fmt::Display for Filesystem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanInput {
    pub disk: String,
    pub hostname: String,
    pub username: String,
    pub password: String,
    pub timezone: String,
    pub filesystem: Filesystem,
}

pub fn validate_disk(disk: &str) -> Result<(), String> {
    let ok = disk.starts_with("/dev/")
        && !disk.contains("..")
        && !disk.contains(' ')
        && (is_lettered(disk, "/dev/sd")
            || is_lettered(disk, "/dev/vd")
            || is_nvme(disk)
            || is_mmc(disk));
    if ok {
        Ok(())
    } else {
        Err(format!(
            "disk must be a whole device such as /dev/nvme0n1 or /dev/sda, got {disk}"
        ))
    }
}

fn is_lettered(disk: &str, prefix: &str) -> bool {
    let Some(rest) = disk.strip_prefix(prefix) else {
        return false;
    };
    rest.len() == 1 && rest.chars().next().is_some_and(|c| c.is_ascii_lowercase())
}

fn is_nvme(disk: &str) -> bool {
    let Some(rest) = disk.strip_prefix("/dev/nvme") else {
        return false;
    };
    let Some((controller, namespace)) = rest.split_once('n') else {
        return false;
    };
    !controller.is_empty()
        && controller.chars().all(|c| c.is_ascii_digit())
        && !namespace.is_empty()
        && namespace.chars().all(|c| c.is_ascii_digit())
}

fn is_mmc(disk: &str) -> bool {
    let Some(rest) = disk.strip_prefix("/dev/mmcblk") else {
        return false;
    };
    !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit())
}

pub fn partition_path(disk: &str, index: u8) -> Result<String, String> {
    validate_disk(disk)?;
    if index == 0 {
        return Err("partition index starts at 1".to_string());
    }
    let needs_p = disk.chars().last().is_some_and(|c| c.is_ascii_digit());
    if needs_p {
        Ok(format!("{disk}p{index}"))
    } else {
        Ok(format!("{disk}{index}"))
    }
}

pub fn validate_hostname(hostname: &str) -> Result<(), String> {
    let bytes = hostname.as_bytes();
    let ok = !bytes.is_empty()
        && bytes.len() <= 63
        && bytes[0].is_ascii_alphanumeric()
        && bytes[bytes.len() - 1].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-');
    if ok {
        Ok(())
    } else {
        Err("hostname must be a lowercase DNS label".to_string())
    }
}

pub fn validate_username(username: &str) -> Result<(), String> {
    let reserved = ["root", "live", "greeter"];
    let bytes = username.as_bytes();
    let ok = (1..=32).contains(&bytes.len())
        && (bytes[0].is_ascii_lowercase() || bytes[0] == b'_')
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'_' || *b == b'-')
        && !reserved.contains(&username);
    if ok {
        Ok(())
    } else {
        Err("username must be a lowercase account name and must not be reserved".to_string())
    }
}

pub fn validate_password(password: &str) -> Result<(), String> {
    if password.chars().count() >= 8 && !password.contains('\n') && !password.contains('\0') {
        Ok(())
    } else {
        Err("password must be at least 8 characters".to_string())
    }
}

pub fn validate_timezone(timezone: &str) -> Result<(), String> {
    if timezone == "UTC" {
        return Ok(());
    }
    let Some((area, city)) = timezone.split_once('/') else {
        return Err("timezone must look like Region/City".to_string());
    };
    let part_ok = |part: &str| {
        !part.is_empty()
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '+')
    };
    if part_ok(area) && part_ok(city) && !timezone.contains("..") {
        Ok(())
    } else {
        Err("timezone must look like Region/City".to_string())
    }
}

pub fn validate_plan(input: &PlanInput) -> Result<(), String> {
    validate_disk(&input.disk)?;
    validate_hostname(&input.hostname)?;
    validate_username(&input.username)?;
    validate_password(&input.password)?;
    validate_timezone(&input.timezone)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_whole_disks() {
        assert!(validate_disk("/dev/sda").is_ok());
        assert!(validate_disk("/dev/vdb").is_ok());
        assert!(validate_disk("/dev/nvme0n1").is_ok());
        assert!(validate_disk("/dev/mmcblk0").is_ok());
    }

    #[test]
    fn rejects_partitions_and_paths() {
        assert!(validate_disk("/dev/sda1").is_err());
        assert!(validate_disk("/dev/nvme0n1p1").is_err());
        assert!(validate_disk("/dev/../sda").is_err());
        assert!(validate_disk("sda").is_err());
    }

    #[test]
    fn builds_partition_paths() {
        assert_eq!(partition_path("/dev/sda", 1).unwrap(), "/dev/sda1");
        assert_eq!(partition_path("/dev/nvme0n1", 2).unwrap(), "/dev/nvme0n1p2");
        assert_eq!(partition_path("/dev/mmcblk0", 1).unwrap(), "/dev/mmcblk0p1");
    }

    #[test]
    fn checks_account_fields() {
        assert!(validate_hostname("aegis").is_ok());
        assert!(validate_hostname("-bad").is_err());
        assert!(validate_username("leo").is_ok());
        assert!(validate_username("root").is_err());
        assert!(validate_password("correcthorsebattery").is_ok());
        assert!(validate_password("short").is_err());
        assert!(validate_timezone("Europe/Rome").is_ok());
        assert!(validate_timezone("UTC").is_ok());
        assert!(validate_timezone("../etc").is_err());
    }
}
