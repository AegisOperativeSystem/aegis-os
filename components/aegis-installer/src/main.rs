use aegis_common::install::{format_size, install_system, parse_lsblk, BlockDevice};
use aegis_common::validate::{Filesystem, PlanInput};
use aegis_common::LIVE_MARKER;
use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box, Button, ComboBoxText, Entry, Label, Orientation, Stack,
    TextView,
};
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc::{self, TryRecvError};
use std::thread;

fn main() -> glib::ExitCode {
    let app = Application::builder()
        .application_id("org.aegis.Installer")
        .build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Install Aegis OS")
        .default_width(720)
        .default_height(520)
        .build();
    let root = Box::new(Orientation::Vertical, 12);
    root.set_margin_top(24);
    root.set_margin_bottom(24);
    root.set_margin_start(24);
    root.set_margin_end(24);
    let title = Label::new(Some("Install Aegis OS"));
    title.add_css_class("title");
    title.set_xalign(0.0);
    root.append(&title);

    if !Path::new(LIVE_MARKER).exists() && std::env::args().all(|arg| arg != "--force") {
        let message = Label::new(Some(
            "The installer runs from the Aegis live image. Boot the ISO and start again.",
        ));
        message.set_xalign(0.0);
        message.set_wrap(true);
        root.append(&message);
        window.set_child(Some(&root));
        window.present();
        return;
    }

    let stack = Stack::new();
    stack.set_vexpand(true);
    let disks = list_disks();
    let disk_combo = ComboBoxText::new();
    for disk in &disks {
        disk_combo.append(
            Some(&disk.path),
            &format!("{} ({})", disk.path, format_size(disk.size_bytes)),
        );
    }
    if !disks.is_empty() {
        disk_combo.set_active(Some(0));
    }
    let filesystem = ComboBoxText::new();
    filesystem.append(Some("ext4"), "ext4");
    filesystem.append(Some("btrfs"), "btrfs");
    filesystem.set_active(Some(0));
    let hostname = Entry::new();
    hostname.set_text("aegis");
    let username = Entry::new();
    let password = Entry::new();
    password.set_visibility(false);
    let timezone = Entry::new();
    timezone.set_text("UTC");
    let confirm = Entry::new();
    let summary = Label::new(None);
    summary.set_xalign(0.0);
    summary.set_wrap(true);
    let log = TextView::new();
    log.set_editable(false);
    log.set_monospace(true);
    let status = Label::new(Some("Ready"));
    status.set_xalign(0.0);

    stack.add_named(&disk_page(&disk_combo, &filesystem), Some("disk"));
    stack.add_named(
        &account_page(&hostname, &username, &password, &timezone),
        Some("account"),
    );
    stack.add_named(&summary_page(&summary, &confirm), Some("summary"));
    stack.add_named(&progress_page(&log), Some("progress"));
    root.append(&stack);
    root.append(&status);

    let controls = Box::new(Orientation::Horizontal, 8);
    let back = Button::with_label("Back");
    let next = Button::with_label("Next");
    controls.append(&back);
    controls.append(&next);
    root.append(&controls);
    window.set_child(Some(&root));

    let state = Rc::new(RefCell::new(Page::Disk));
    let widgets = Widgets {
        stack: stack.clone(),
        disk_combo: disk_combo.clone(),
        filesystem: filesystem.clone(),
        hostname: hostname.clone(),
        username: username.clone(),
        password: password.clone(),
        timezone: timezone.clone(),
        confirm: confirm.clone(),
        summary: summary.clone(),
        log: log.clone(),
        status: status.clone(),
        next: next.clone(),
        disks,
    };
    let forward = widgets.clone();
    let backward = widgets.clone();
    let page = state.clone();
    next.connect_clicked(move |_| advance(&forward, &page));
    let page = state.clone();
    back.connect_clicked(move |_| retreat(&backward, &page));
    window.present();
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Disk,
    Account,
    Summary,
    Progress,
}

#[derive(Clone)]
struct Widgets {
    stack: Stack,
    disk_combo: ComboBoxText,
    filesystem: ComboBoxText,
    hostname: Entry,
    username: Entry,
    password: Entry,
    timezone: Entry,
    confirm: Entry,
    summary: Label,
    log: TextView,
    status: Label,
    next: Button,
    disks: Vec<BlockDevice>,
}

fn disk_page(disks: &ComboBoxText, filesystem: &ComboBoxText) -> Box {
    let page = form();
    page.append(&heading("Target disk"));
    page.append(&note(
        "The selected disk is erased. Aegis OS requires UEFI and creates a 1 GiB EFI system partition plus a root partition.",
    ));
    page.append(disks);
    page.append(&heading("Filesystem"));
    page.append(filesystem);
    page
}

fn account_page(hostname: &Entry, username: &Entry, password: &Entry, timezone: &Entry) -> Box {
    let page = form();
    page.append(&heading("Hostname"));
    page.append(hostname);
    page.append(&heading("Username"));
    page.append(username);
    page.append(&heading("Password"));
    page.append(password);
    page.append(&heading("Timezone"));
    page.append(timezone);
    page.append(&note("Use a Region/City name such as Europe/Rome, or UTC."));
    page
}

fn summary_page(summary: &Label, confirm: &Entry) -> Box {
    let page = form();
    page.append(&heading("Confirm"));
    page.append(summary);
    page.append(&note(
        "Type the disk name, for example vda, to arm the installer.",
    ));
    page.append(confirm);
    page
}

fn progress_page(log: &TextView) -> Box {
    let page = form();
    let scrolled = gtk4::ScrolledWindow::builder()
        .child(log)
        .vexpand(true)
        .build();
    page.append(&scrolled);
    page
}

fn advance(widgets: &Widgets, page: &Rc<RefCell<Page>>) {
    let current = *page.borrow();
    match current {
        Page::Disk => {
            if widgets.disk_combo.active_id().is_none() {
                return;
            }
            show(widgets, page, Page::Account);
        }
        Page::Account => {
            if let Err(err) = plan_from(widgets) {
                widgets.status.set_text(&err);
                return;
            }
            if let Ok(plan) = plan_from(widgets) {
                widgets.summary.set_text(&describe(&plan));
            }
            show(widgets, page, Page::Summary);
        }
        Page::Summary => {
            let Ok(plan) = plan_from(widgets) else {
                return;
            };
            let expected = plan.disk.rsplit('/').next().unwrap_or_default().to_string();
            if widgets.confirm.text().as_str() != expected {
                widgets.summary.set_text(&format!(
                    "{}\n\nType {expected} to confirm.",
                    describe(&plan)
                ));
                return;
            }
            show(widgets, page, Page::Progress);
            widgets.next.set_sensitive(false);
            start_install(widgets, plan);
        }
        Page::Progress => {}
    }
}

fn retreat(widgets: &Widgets, page: &Rc<RefCell<Page>>) {
    let previous = match *page.borrow() {
        Page::Account => Page::Disk,
        Page::Summary => Page::Account,
        Page::Disk | Page::Progress => return,
    };
    show(widgets, page, previous);
}

fn show(widgets: &Widgets, page: &Rc<RefCell<Page>>, next: Page) {
    let name = match next {
        Page::Disk => "disk",
        Page::Account => "account",
        Page::Summary => "summary",
        Page::Progress => "progress",
    };
    widgets.stack.set_visible_child_name(name);
    *page.borrow_mut() = next;
}

fn start_install(widgets: &Widgets, plan: PlanInput) {
    let (sender, receiver) = mpsc::channel::<String>();
    let status = widgets.status.clone();
    let log = widgets.log.clone();
    thread::spawn(move || {
        let mut report = |line: &str| {
            let _ = sender.send(line.to_string());
        };
        let kernel = std::env::var("AEGIS_KERNEL_PKG").unwrap_or_else(|_| "linux".to_string());
        match install_system(&plan, &kernel, &mut report) {
            Ok(()) => {
                let _ = sender.send("Installed. Reboot into Aegis OS.".to_string());
            }
            Err(err) => {
                let _ = sender.send(format!("Install failed: {err}"));
            }
        }
    });
    glib::timeout_add_local(std::time::Duration::from_millis(200), move || loop {
        match receiver.try_recv() {
            Ok(line) => {
                status.set_text(&line);
                let buffer = log.buffer();
                let mut end = buffer.end_iter();
                buffer.insert(&mut end, &format!("{line}\n"));
            }
            Err(TryRecvError::Empty) => return glib::ControlFlow::Continue,
            Err(TryRecvError::Disconnected) => return glib::ControlFlow::Break,
        }
    });
}

fn plan_from(widgets: &Widgets) -> Result<PlanInput, String> {
    let disk = widgets
        .disk_combo
        .active_id()
        .ok_or_else(|| "select a disk".to_string())?
        .to_string();
    let filesystem = match widgets.filesystem.active_id().as_deref() {
        Some("btrfs") => Filesystem::Btrfs,
        _ => Filesystem::Ext4,
    };
    let input = PlanInput {
        disk,
        hostname: widgets.hostname.text().to_string(),
        username: widgets.username.text().to_string(),
        password: widgets.password.text().to_string(),
        timezone: widgets.timezone.text().to_string(),
        filesystem,
    };
    aegis_common::validate::validate_plan(&input)?;
    Ok(input)
}

fn describe(plan: &PlanInput) -> String {
    format!(
        "Disk {}\nHostname {}\nUser {}\nTimezone {}\nFilesystem {}\n\nThis destroys every partition on the disk.",
        plan.disk, plan.hostname, plan.username, plan.timezone, plan.filesystem
    )
}

fn list_disks() -> Vec<BlockDevice> {
    let Ok(output) = std::process::Command::new("lsblk")
        .args(["-dn", "-b", "-o", "NAME,SIZE,TYPE"])
        .output()
    else {
        return Vec::new();
    };
    parse_lsblk(&String::from_utf8_lossy(&output.stdout))
}

fn form() -> Box {
    let page = Box::new(Orientation::Vertical, 8);
    page.set_margin_top(8);
    page
}

fn heading(text: &str) -> Label {
    let label = Label::new(Some(text));
    label.set_xalign(0.0);
    label
}

fn note(text: &str) -> Label {
    let label = Label::new(Some(text));
    label.add_css_class("muted");
    label.set_xalign(0.0);
    label.set_wrap(true);
    label
}
