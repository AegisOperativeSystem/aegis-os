use aegis_common::install::build_actions;
use aegis_common::validate::{Filesystem, PlanInput};

fn main() {
    let mut args = std::env::args().skip(1);
    let disk = args.next().unwrap_or_else(|| "/dev/vda".to_string());
    let hostname = args.next().unwrap_or_else(|| "aegis".to_string());
    let username = args.next().unwrap_or_else(|| "aegis".to_string());
    let timezone = args.next().unwrap_or_else(|| "UTC".to_string());
    let filesystem = match args.next().as_deref() {
        Some("btrfs") => Filesystem::Btrfs,
        _ => Filesystem::Ext4,
    };
    let kernel = args.next().unwrap_or_else(|| "linux".to_string());
    let input = PlanInput {
        disk,
        hostname,
        username,
        password: "plan-only-secret".to_string(),
        timezone,
        filesystem,
    };
    match build_actions(&input, &kernel) {
        Ok(actions) => {
            for action in actions {
                println!("{action}");
            }
        }
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}
