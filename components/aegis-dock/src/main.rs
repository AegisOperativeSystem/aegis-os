use aegis_common::desktop::{command_argv, parse_desktop_entry, DesktopEntry};
use aegis_common::dock::dock_favorites;
use aegis_common::LIVE_MARKER;
use gtk4::gdk::Display;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    style_context_add_provider_for_display, Align, Application, ApplicationWindow, Box, Button,
    CssProvider, FlowBox, Image, Label, Orientation, ScrolledWindow, SelectionMode, Separator,
    STYLE_PROVIDER_PRIORITY_APPLICATION,
};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::path::Path;
use std::process::Command;

const STYLE: &str = include_str!("../assets/style.css");

fn main() -> glib::ExitCode {
    let app = Application::builder()
        .application_id("org.aegis.Dock")
        .build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &Application) {
    load_css();
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Dock")
        .decorated(false)
        .resizable(false)
        .build();
    window.add_css_class("dock-window");
    if gtk4_layer_shell::is_supported() {
        window.init_layer_shell();
        window.set_layer(Layer::Top);
        window.set_namespace(Some("aegis-dock"));
        window.set_anchor(Edge::Bottom, true);
        window.set_margin(Edge::Bottom, 8);
        window.set_keyboard_mode(KeyboardMode::None);
        window.auto_exclusive_zone_enable();
    }

    let dock = Box::new(Orientation::Horizontal, 4);
    dock.add_css_class("dock");
    dock.set_halign(Align::Center);
    let live = Path::new(LIVE_MARKER).exists();
    for favorite in dock_favorites(live) {
        let button = Button::new();
        button.set_tooltip_text(Some(favorite.label));
        button.set_child(Some(&app_icon(favorite.icon)));
        let argv = favorite
            .argv
            .iter()
            .map(|part| (*part).to_string())
            .collect::<Vec<_>>();
        button.connect_clicked(move |_| spawn(&argv));
        dock.append(&button);
    }

    let separator = Separator::new(Orientation::Vertical);
    dock.append(&separator);

    let grid = Button::new();
    grid.set_tooltip_text(Some("Show applications"));
    grid.set_child(Some(&app_icon("view-app-grid-symbolic")));
    let app_for_grid = app.clone();
    grid.connect_clicked(move |_| open_grid(&app_for_grid));
    dock.append(&grid);

    window.set_child(Some(&dock));
    window.present();
}

fn open_grid(app: &Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Applications")
        .default_width(640)
        .default_height(480)
        .build();
    window.add_css_class("apps");
    let flow = FlowBox::new();
    flow.set_selection_mode(SelectionMode::None);
    flow.set_max_children_per_line(6);
    flow.set_min_children_per_line(4);
    flow.set_column_spacing(8);
    flow.set_row_spacing(8);
    flow.set_margin_top(16);
    flow.set_margin_bottom(16);
    flow.set_margin_start(16);
    flow.set_margin_end(16);
    for entry in load_apps() {
        let button = Button::new();
        button.set_tooltip_text(Some(&entry.name));
        let column = Box::new(Orientation::Vertical, 8);
        column.set_halign(Align::Center);
        column.append(&app_icon(icon_name(&entry)));
        let label = Label::new(Some(&entry.name));
        label.add_css_class("app-name");
        label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        label.set_max_width_chars(12);
        column.append(&label);
        button.set_child(Some(&column));
        let argv = command_argv(&entry);
        let window_for_close = window.clone();
        button.connect_clicked(move |_| {
            spawn(&argv);
            window_for_close.close();
        });
        flow.append(&button);
    }
    let scrolled = ScrolledWindow::builder().child(&flow).build();
    window.set_child(Some(&scrolled));
    window.present();
}

fn icon_name(entry: &DesktopEntry) -> &str {
    if entry.icon.is_empty() {
        "application-x-executable"
    } else {
        &entry.icon
    }
}

fn app_icon(name: &str) -> Box {
    let slot = Box::new(Orientation::Vertical, 0);
    slot.set_halign(Align::Center);
    slot.set_valign(Align::Center);
    let image = Image::from_icon_name(name);
    image.set_pixel_size(40);
    slot.append(&image);
    slot
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
    let path = "/usr/share/aegis/dock/style.css";
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
