use aegis_common::desktop::{command_argv, parse_desktop_entry, DesktopEntry};
use gtk4::gdk::Display;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    style_context_add_provider_for_display, Application, ApplicationWindow, Box, Button,
    CssProvider, Label, Orientation, ScrolledWindow, SelectionMode,
    STYLE_PROVIDER_PRIORITY_APPLICATION,
};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::path::Path;
use std::process::Command;

const STYLE: &str = include_str!("../assets/style.css");

fn main() -> glib::ExitCode {
    let app = Application::builder()
        .application_id("org.aegis.Shell")
        .build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Aegis")
        .decorated(false)
        .resizable(false)
        .build();
    load_css();
    window.add_css_class("panel");
    if gtk4_layer_shell::is_supported() {
        window.init_layer_shell();
        window.set_layer(Layer::Top);
        window.set_namespace(Some("aegis-shell"));
        window.set_anchor(Edge::Top, true);
        window.set_anchor(Edge::Left, true);
        window.set_anchor(Edge::Right, true);
        window.set_keyboard_mode(KeyboardMode::None);
        window.set_exclusive_zone(32);
    } else {
        window.set_default_size(1280, 32);
    }

    let bar = Box::new(Orientation::Horizontal, 4);
    bar.add_css_class("panel-box");
    bar.set_size_request(-1, 32);
    let brand = Label::new(Some("Aegis"));
    brand.add_css_class("brand");
    let apps = Button::with_label("Apps");
    let terminal = Button::with_label("Terminal");
    let clock = Label::new(Some("--:--"));
    clock.add_css_class("clock");
    clock.set_hexpand(true);
    let logout = Button::with_label("Exit");
    let reboot = Button::with_label("Reboot");
    let power = Button::with_label("Power");
    power.add_css_class("danger");
    bar.append(&brand);
    bar.append(&apps);
    bar.append(&terminal);
    bar.append(&clock);
    bar.append(&logout);
    bar.append(&reboot);
    bar.append(&power);
    window.set_child(Some(&bar));

    let clock_label = clock.clone();
    glib::timeout_add_local(std::time::Duration::from_secs(1), move || {
        if let Ok(now) = glib::DateTime::now_local() {
            if let Ok(text) = now.format("%H:%M") {
                clock_label.set_text(text.as_str());
            }
        }
        glib::ControlFlow::Continue
    });

    terminal.connect_clicked(|_| spawn(&["foot".to_string()]));
    logout.connect_clicked(|_| spawn(&["labwc".to_string(), "--exit".to_string()]));
    reboot.connect_clicked(|_| spawn(&["systemctl".to_string(), "reboot".to_string()]));
    power.connect_clicked(|_| spawn(&["systemctl".to_string(), "poweroff".to_string()]));

    let app_for_launcher = app.clone();
    apps.connect_clicked(move |_| open_launcher(&app_for_launcher));
    window.present();
}

fn open_launcher(app: &Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Apps")
        .default_width(420)
        .default_height(520)
        .build();
    load_css();
    let list = gtk4::ListBox::new();
    list.set_selection_mode(SelectionMode::None);
    let scrolled = ScrolledWindow::builder().child(&list).build();
    window.set_child(Some(&scrolled));
    for entry in load_apps() {
        let button = Button::with_label(&entry.name);
        button.set_hexpand(true);
        button.connect_clicked(move |_| spawn(&command_argv(&entry)));
        list.append(&button);
    }
    window.present();
}

fn load_apps() -> Vec<DesktopEntry> {
    let mut apps = Vec::new();
    let Ok(entries) = std::fs::read_dir("/usr/share/applications") else {
        return apps;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("desktop") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        if let Some(parsed) = parse_desktop_entry(&text) {
            if !parsed.no_display {
                apps.push(parsed);
            }
        }
    }
    apps.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    apps
}

fn spawn(argv: &[String]) {
    let Some((program, args)) = argv.split_first() else {
        return;
    };
    let _ = Command::new(program).args(args).spawn();
}

fn load_css() {
    let provider = CssProvider::new();
    let path = "/usr/share/aegis/shell/style.css";
    if Path::new(path).is_file() {
        provider.load_from_path(path);
    } else {
        provider.load_from_bytes(&glib::Bytes::from_owned(STYLE.to_owned()));
    }
    if let Some(display) = Display::default() {
        style_context_add_provider_for_display(
            &display,
            &provider,
            STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
