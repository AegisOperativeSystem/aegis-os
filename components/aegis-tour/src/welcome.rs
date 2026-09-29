use aegis_common::LIVE_MARKER;
use gtk4::gdk::Display;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    style_context_add_provider_for_display, Align, Application, ApplicationWindow, Box, Button,
    CssProvider, Label, Orientation, STYLE_PROVIDER_PRIORITY_APPLICATION,
};
use std::process::Command;

const STYLE: &str = include_str!("../assets/style.css");

fn main() -> glib::ExitCode {
    if !std::path::Path::new(LIVE_MARKER).exists() {
        return glib::ExitCode::SUCCESS;
    }
    let app = Application::builder()
        .application_id("org.aegis.Welcome")
        .build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &Application) {
    let provider = CssProvider::new();
    let path = "/usr/share/aegis/tour/style.css";
    if std::path::Path::new(path).is_file() {
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

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Aegis OS live")
        .default_width(560)
        .default_height(360)
        .build();
    let root = Box::new(Orientation::Vertical, 12);
    root.set_margin_top(28);
    root.set_margin_bottom(24);
    root.set_margin_start(28);
    root.set_margin_end(28);
    let title = Label::new(Some("You are in the live desktop"));
    title.add_css_class("title");
    title.set_xalign(0.0);
    title.set_wrap(true);
    let body = Label::new(Some(
        "Nothing is installed yet. Install Aegis, repair an existing installation, or keep using this session.",
    ));
    body.add_css_class("body");
    body.set_xalign(0.0);
    body.set_wrap(true);
    let actions = Box::new(Orientation::Horizontal, 8);
    actions.set_halign(Align::End);
    let install = Button::with_label("Install");
    install.add_css_class("suggested");
    let repair = Button::with_label("Repair");
    let stay = Button::with_label("Use the live desktop");
    actions.append(&stay);
    actions.append(&repair);
    actions.append(&install);
    root.append(&title);
    root.append(&body);
    root.append(&actions);
    window.set_child(Some(&root));

    let install_window = window.clone();
    install.connect_clicked(move |_| {
        let _ = Command::new("pkexec").args(["aegis-installer"]).spawn();
        install_window.close();
    });
    let repair_window = window.clone();
    repair.connect_clicked(move |_| {
        let _ = Command::new("pkexec")
            .args(["aegis-installer", "--repair"])
            .spawn();
        repair_window.close();
    });
    let stay_window = window.clone();
    stay.connect_clicked(move |_| stay_window.close());
    window.present();
}
