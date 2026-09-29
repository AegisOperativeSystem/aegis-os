use aegis_common::install::{format_size, install_system, parse_lsblk, repair_system, BlockDevice};
use aegis_common::validate::{Filesystem, PlanInput};
use aegis_common::LIVE_MARKER;
use gtk4::gdk::Display;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    style_context_add_provider_for_display, Application, ApplicationWindow, Box, Button,
    ComboBoxText, CssProvider, Entry, Label, Orientation, Stack, TextView,
    STYLE_PROVIDER_PRIORITY_APPLICATION,
};
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc::{self, TryRecvError};
use std::thread;

const STYLE: &str = r#"
window { background: #121820; color: #f3f6fb; }
.title { font-size: 28px; font-weight: 750; }
.steps { color: #3ee0c5; font-weight: 750; letter-spacing: 0.08em; }
.muted { color: #a7b3c4; }
entry, combobox { min-height: 36px; background: #090d12; color: #f3f6fb; border-radius: 8px; }
button.suggested { background: #3ee0c5; color: #06211c; font-weight: 750; border-radius: 10px; }
"#;

fn main() -> glib::ExitCode {
    let app = Application::builder()
        .application_id("org.aegis.Installer")
        .build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &Application) {
    load_css();
    if std::env::args().any(|arg| arg == "--repair") {
        build_repair(app);
        return;
    }
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Install Aegis OS")
        .default_width(760)
        .default_height(640)
        .build();
    let root = Box::new(Orientation::Vertical, 12);
    root.set_margin_top(24);
    root.set_margin_bottom(24);
    root.set_margin_start(24);
    root.set_margin_end(24);
    let title = Label::new(Some("Install Aegis OS"));
    title.add_css_class("title");
    title.set_xalign(0.0);
    let steps = Label::new(Some(step_text(Page::Language)));
    steps.add_css_class("steps");
    steps.set_xalign(0.0);
    root.append(&title);
    root.append(&steps);

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
    let filesystem = combo(&[("ext4", "ext4"), ("btrfs", "btrfs")]);
    let swap = combo(&[
        ("0", "No swap"),
        ("2", "2 GiB swapfile"),
        ("4", "4 GiB swapfile"),
    ]);
    let hostname = Entry::new();
    hostname.set_text("aegis");
    let username = Entry::new();
    let password = Entry::new();
    password.set_visibility(false);
    let confirm = Entry::new();
    confirm.set_visibility(false);
    let timezone = combo(&[
        ("UTC", "UTC"),
        ("Europe/Rome", "Europe/Rome"),
        ("Europe/London", "Europe/London"),
        ("Europe/Paris", "Europe/Paris"),
        ("Europe/Berlin", "Europe/Berlin"),
        ("Europe/Madrid", "Europe/Madrid"),
        ("America/New_York", "America/New York"),
        ("America/Los_Angeles", "America/Los Angeles"),
        ("America/Sao_Paulo", "America/Sao Paulo"),
        ("Asia/Tokyo", "Asia/Tokyo"),
        ("Australia/Sydney", "Australia/Sydney"),
    ]);
    let locale = combo(&[
        ("en_US.UTF-8", "English (United States)"),
        ("it_IT.UTF-8", "Italiano"),
        ("de_DE.UTF-8", "Deutsch"),
        ("fr_FR.UTF-8", "Français"),
        ("es_ES.UTF-8", "Español"),
        ("pt_BR.UTF-8", "Português (Brasil)"),
    ]);
    let keymap = combo(&[
        ("us", "US"),
        ("it", "Italian"),
        ("de", "German"),
        ("fr", "French"),
        ("es", "Spanish"),
        ("gb", "UK"),
    ]);
    let disk_confirm = Entry::new();
    let summary = Label::new(None);
    summary.set_xalign(0.0);
    summary.set_wrap(true);
    let log = TextView::new();
    log.set_editable(false);
    log.set_monospace(true);
    let status = Label::new(Some("Detecting keyboard and language"));
    status.set_xalign(0.0);
    status.set_wrap(true);

    stack.add_named(&language_page(&keymap, &locale), Some("language"));
    stack.add_named(&timezone_page(&timezone), Some("timezone"));
    stack.add_named(&disk_page(&disk_combo, &filesystem, &swap), Some("disk"));
    stack.add_named(
        &account_page(&hostname, &username, &password, &confirm),
        Some("account"),
    );
    stack.add_named(&summary_page(&summary, &disk_confirm), Some("summary"));
    stack.add_named(&progress_page(&log), Some("progress"));
    root.append(&stack);
    root.append(&status);

    let controls = Box::new(Orientation::Horizontal, 8);
    controls.set_halign(gtk4::Align::End);
    let back = Button::with_label("Back");
    let next = Button::with_label("Next");
    next.add_css_class("suggested");
    controls.append(&back);
    controls.append(&next);
    root.append(&controls);
    window.set_child(Some(&root));

    let state = Rc::new(RefCell::new(Page::Language));
    let widgets = Widgets {
        stack: stack.clone(),
        disk_combo: disk_combo.clone(),
        filesystem: filesystem.clone(),
        swap: swap.clone(),
        hostname: hostname.clone(),
        username: username.clone(),
        password: password.clone(),
        confirm: confirm.clone(),
        timezone: timezone.clone(),
        locale: locale.clone(),
        keymap: keymap.clone(),
        disk_confirm: disk_confirm.clone(),
        summary: summary.clone(),
        log: log.clone(),
        status: status.clone(),
        next: next.clone(),
        steps: steps.clone(),
    };
    let forward = widgets.clone();
    let backward = widgets.clone();
    let page = state.clone();
    next.connect_clicked(move |_| advance(&forward, &page));
    let page = state.clone();
    back.connect_clicked(move |_| retreat(&backward, &page));
    detect_region(&locale, &keymap, &timezone, &status);
    window.present();
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Language,
    Timezone,
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
    swap: ComboBoxText,
    hostname: Entry,
    username: Entry,
    password: Entry,
    confirm: Entry,
    timezone: ComboBoxText,
    locale: ComboBoxText,
    keymap: ComboBoxText,
    disk_confirm: Entry,
    summary: Label,
    log: TextView,
    status: Label,
    next: Button,
    steps: Label,
}

fn disk_page(disks: &ComboBoxText, filesystem: &ComboBoxText, swap: &ComboBoxText) -> Box {
    let page = form();
    page.append(&heading("Target disk"));
    page.append(&note(
        "The selected disk is erased. UEFI gets a 1 GiB EFI partition and systemd-boot. BIOS, including a default VirtualBox VM, gets one bootable partition and Syslinux. Attach this ISO to the virtual optical drive before powering on.",
    ));
    page.append(disks);
    page.append(&heading("Filesystem"));
    page.append(filesystem);
    page.append(&heading("Swap"));
    page.append(swap);
    page.append(&note(
        "A swapfile lives on the root filesystem. Leave it off when the disk is close to 8 GiB.",
    ));
    page
}

fn account_page(hostname: &Entry, username: &Entry, password: &Entry, confirm: &Entry) -> Box {
    let page = form();
    page.append(&heading("Hostname"));
    page.append(hostname);
    page.append(&heading("Username"));
    page.append(username);
    page.append(&heading("Password"));
    page.append(password);
    page.append(&heading("Confirm password"));
    page.append(confirm);
    page.append(&note(
        "Use at least 8 characters. The account joins the wheel group and can use sudo.",
    ));
    page
}

fn language_page(keymap: &ComboBoxText, locale: &ComboBoxText) -> Box {
    let page = form();
    page.append(&heading("Keyboard"));
    page.append(keymap);
    page.append(&heading("Language"));
    page.append(locale);
    page.append(&note(
        "Filled in from your network location. Change them if the guess is wrong. The keyboard applies to the console and the desktop. The language is the installed locale.",
    ));
    page
}

fn timezone_page(timezone: &ComboBoxText) -> Box {
    let page = form();
    page.append(&heading("Timezone"));
    page.append(timezone);
    page.append(&note(
        "Filled in from the same location. The clock and the installed system use this zone.",
    ));
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
        Page::Language => show(widgets, page, Page::Timezone),
        Page::Timezone => show(widgets, page, Page::Disk),
        Page::Disk => {
            if widgets.disk_combo.active_id().is_none() {
                widgets.status.set_text("Select a disk of at least 8 GiB");
                return;
            }
            widgets.status.set_text("Ready");
            show(widgets, page, Page::Account);
        }
        Page::Account => {
            let plan = match plan_from(widgets) {
                Ok(plan) => plan,
                Err(err) => {
                    widgets.status.set_text(&err);
                    return;
                }
            };
            widgets.summary.set_text(&format!(
                "{}\nFirmware {}",
                describe(&plan),
                firmware_name()
            ));
            widgets.status.set_text("Ready");
            show(widgets, page, Page::Summary);
        }
        Page::Summary => {
            let Ok(plan) = plan_from(widgets) else {
                return;
            };
            let expected = plan.disk.rsplit('/').next().unwrap_or_default().to_string();
            if widgets.disk_confirm.text().as_str() != expected {
                widgets
                    .status
                    .set_text(&format!("Type {expected} to confirm"));
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
        Page::Timezone => Page::Language,
        Page::Disk => Page::Timezone,
        Page::Account => Page::Disk,
        Page::Summary => Page::Account,
        Page::Language | Page::Progress => return,
    };
    show(widgets, page, previous);
}

fn show(widgets: &Widgets, page: &Rc<RefCell<Page>>, next: Page) {
    let name = match next {
        Page::Language => "language",
        Page::Timezone => "timezone",
        Page::Disk => "disk",
        Page::Account => "account",
        Page::Summary => "summary",
        Page::Progress => "progress",
    };
    widgets.stack.set_visible_child_name(name);
    widgets.steps.set_text(step_text(next));
    *page.borrow_mut() = next;
}

fn step_text(page: Page) -> &'static str {
    match page {
        Page::Language => "1 of 5  ·  Keyboard",
        Page::Timezone => "2 of 5  ·  Timezone",
        Page::Disk => "3 of 5  ·  Disk",
        Page::Account => "4 of 5  ·  Account",
        Page::Summary => "5 of 5  ·  Confirm",
        Page::Progress => "Installing",
    }
}

fn detect_region(
    locale: &ComboBoxText,
    keymap: &ComboBoxText,
    timezone: &ComboBoxText,
    status: &Label,
) {
    let locale = locale.clone();
    let keymap = keymap.clone();
    let timezone = timezone.clone();
    let status = status.clone();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(aegis_common::region::detect_region());
    });
    glib::timeout_add_local(std::time::Duration::from_millis(200), move || {
        let guess = match receiver.try_recv() {
            Ok(guess) => guess,
            Err(TryRecvError::Empty) => return glib::ControlFlow::Continue,
            Err(TryRecvError::Disconnected) => return glib::ControlFlow::Break,
        };
        let Some(guess) = guess else {
            if status.text().as_str() == "Detecting keyboard and language" {
                status.set_text("Choose a keyboard and a language");
            }
            return glib::ControlFlow::Break;
        };
        if keymap.active_id().as_deref() == Some("us") {
            keymap.set_active_id(Some(&guess.keymap));
        }
        if locale.active_id().as_deref() == Some("en_US.UTF-8") {
            locale.set_active_id(Some(&guess.locale));
        }
        if timezone.active_id().as_deref() == Some("UTC") {
            let id = guess.timezone.clone();
            if !timezone.set_active_id(Some(&id)) {
                timezone.append(Some(&id), &id);
                timezone.set_active_id(Some(&id));
            }
        }
        if status.text().as_str() == "Detecting keyboard and language" {
            status.set_text(&format!("Detected {} · {}", guess.keymap, guess.locale));
        }
        glib::ControlFlow::Break
    });
}

fn firmware_name() -> &'static str {
    if Path::new("/sys/firmware/efi").is_dir() {
        "UEFI"
    } else {
        "BIOS"
    }
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
    if widgets.password.text().as_str() != widgets.confirm.text().as_str() {
        return Err("passwords do not match".to_string());
    }
    let disk = widgets
        .disk_combo
        .active_id()
        .ok_or_else(|| "select a disk".to_string())?
        .to_string();
    let filesystem = match widgets.filesystem.active_id().as_deref() {
        Some("btrfs") => Filesystem::Btrfs,
        _ => Filesystem::Ext4,
    };
    let swap_gib = widgets
        .swap
        .active_id()
        .as_deref()
        .unwrap_or("0")
        .parse::<u8>()
        .unwrap_or(0);
    let input = PlanInput {
        disk,
        hostname: widgets.hostname.text().to_string(),
        username: widgets.username.text().to_string(),
        password: widgets.password.text().to_string(),
        timezone: active(&widgets.timezone, "UTC"),
        filesystem,
        locale: active(&widgets.locale, "en_US.UTF-8"),
        keymap: active(&widgets.keymap, "us"),
        swap_gib,
    };
    aegis_common::validate::validate_plan(&input)?;
    Ok(input)
}

fn describe(plan: &PlanInput) -> String {
    let swap = if plan.swap_gib == 0 {
        "none".to_string()
    } else {
        format!("{} GiB", plan.swap_gib)
    };
    format!(
        "Keyboard {}\nLanguage {}\nTimezone {}\nDisk {}\nHostname {}\nUser {}\nFilesystem {}\nSwap {}\n\nThis destroys every partition on the disk.",
        plan.keymap,
        plan.locale,
        plan.timezone,
        plan.disk,
        plan.hostname,
        plan.username,
        plan.filesystem,
        swap
    )
}

fn active(combo: &ComboBoxText, fallback: &str) -> String {
    combo
        .active_id()
        .map(|value| value.to_string())
        .unwrap_or_else(|| fallback.to_string())
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

fn combo(options: &[(&str, &str)]) -> ComboBoxText {
    let combo = ComboBoxText::new();
    for (id, label) in options {
        combo.append(Some(id), label);
    }
    combo.set_active(Some(0));
    combo
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

fn build_repair(app: &Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Repair Aegis OS")
        .default_width(640)
        .default_height(480)
        .build();
    let root = Box::new(Orientation::Vertical, 12);
    root.set_margin_top(24);
    root.set_margin_bottom(24);
    root.set_margin_start(24);
    root.set_margin_end(24);
    let title = Label::new(Some("Repair an installation"));
    title.add_css_class("title");
    title.set_xalign(0.0);
    root.append(&title);
    root.append(&note(
        "Checks the filesystem labeled aegis and reinstalls its bootloader. The disk is not erased.",
    ));
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
    root.append(&disk_combo);
    let confirm = Entry::new();
    root.append(&note("Type the disk name, for example vda."));
    root.append(&confirm);
    let log = TextView::new();
    log.set_editable(false);
    log.set_monospace(true);
    root.append(&progress_page(&log));
    let status = Label::new(Some("Ready"));
    status.set_xalign(0.0);
    status.set_wrap(true);
    root.append(&status);
    let button = Button::with_label("Repair");
    button.add_css_class("suggested");
    button.set_halign(gtk4::Align::End);
    root.append(&button);
    window.set_child(Some(&root));

    let combo = disk_combo.clone();
    let confirm = confirm.clone();
    let status = status.clone();
    let log = log.clone();
    button.connect_clicked(move |button| {
        let Some(disk) = combo.active_id() else {
            status.set_text("Select a disk");
            return;
        };
        let expected = disk.rsplit('/').next().unwrap_or_default();
        if confirm.text().as_str() != expected {
            status.set_text(&format!("Type {expected} to confirm"));
            return;
        }
        button.set_sensitive(false);
        status.set_text("Repairing");
        let (sender, receiver) = mpsc::channel();
        let disk = disk.to_string();
        thread::spawn(move || {
            let result = repair_system(&disk, &mut |line| {
                let _ = sender.send(line.to_string());
            });
            let _ = sender.send(match result {
                Ok(()) => "Repair finished. Restart and unplug the ISO.".to_string(),
                Err(err) => format!("Repair failed: {err}"),
            });
        });
        let log = log.clone();
        let status = status.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(200), move || {
            loop {
                match receiver.try_recv() {
                    Ok(line) => {
                        status.set_text(&line);
                        log.buffer()
                            .insert(&mut log.buffer().end_iter(), &format!("{line}\n"));
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => return glib::ControlFlow::Break,
                }
            }
            glib::ControlFlow::Continue
        });
    });
    window.present();
}

fn load_css() {
    let provider = CssProvider::new();
    provider.load_from_bytes(&glib::Bytes::from_owned(STYLE.to_owned()));
    if let Some(display) = Display::default() {
        style_context_add_provider_for_display(
            &display,
            &provider,
            STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
