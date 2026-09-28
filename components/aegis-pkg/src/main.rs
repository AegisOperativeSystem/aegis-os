use aegis_common::packages::{pacman_remove_args, pacman_sync_args, parse_pacman_sl, RepoPackage};
use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box, Button, Label, ListBox, Orientation, ScrolledWindow,
};
use std::process::Command;

fn main() -> glib::ExitCode {
    let app = Application::builder()
        .application_id("org.aegis.Packages")
        .build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &Application) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Aegis Packages")
        .default_width(640)
        .default_height(560)
        .build();
    let root = Box::new(Orientation::Vertical, 8);
    root.set_margin_top(16);
    root.set_margin_bottom(16);
    root.set_margin_start(16);
    root.set_margin_end(16);
    let title = Label::new(Some("Aegis repository"));
    title.add_css_class("title");
    title.set_xalign(0.0);
    let status = Label::new(Some("Refresh to read the aegis repository."));
    status.set_xalign(0.0);
    status.set_wrap(true);
    let list = ListBox::new();
    let scrolled = ScrolledWindow::builder().child(&list).vexpand(true).build();
    let controls = Box::new(Orientation::Horizontal, 8);
    let refresh = Button::with_label("Refresh");
    let install = Button::with_label("Install");
    let remove = Button::with_label("Remove");
    controls.append(&refresh);
    controls.append(&install);
    controls.append(&remove);
    root.append(&title);
    root.append(&status);
    root.append(&scrolled);
    root.append(&controls);
    window.set_child(Some(&root));

    let list_refresh = list.clone();
    let status_refresh = status.clone();
    refresh.connect_clicked(move |_| reload(&list_refresh, &status_refresh));
    let list_install = list.clone();
    let status_install = status.clone();
    install.connect_clicked(move |_| {
        run_selected(&list_install, &status_install, true);
    });
    let list_remove = list.clone();
    let status_remove = status.clone();
    remove.connect_clicked(move |_| {
        run_selected(&list_remove, &status_remove, false);
    });
    reload(&list, &status);
    window.present();
}

fn reload(list: &ListBox, status: &Label) {
    while let Some(row) = list.row_at_index(0) {
        list.remove(&row);
    }
    match query_packages() {
        Ok(packages) => {
            status.set_text(&format!("{} packages in aegis", packages.len()));
            for package in packages {
                let mark = if package.installed {
                    "installed"
                } else {
                    "available"
                };
                let row = gtk4::ListBoxRow::new();
                row.set_widget_name(&package.name);
                let label = Label::new(Some(&format!(
                    "{} {} ({mark})",
                    package.name, package.version
                )));
                label.set_xalign(0.0);
                label.set_margin_top(6);
                label.set_margin_bottom(6);
                label.set_margin_start(8);
                row.set_child(Some(&label));
                list.append(&row);
            }
        }
        Err(err) => status.set_text(&err),
    }
}

fn query_packages() -> Result<Vec<RepoPackage>, String> {
    let output = Command::new("pacman")
        .args(["-Sl", "aegis"])
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(parse_pacman_sl(&String::from_utf8_lossy(&output.stdout)))
}

fn run_selected(list: &ListBox, status: &Label, install: bool) {
    let Some(row) = list.selected_row() else {
        status.set_text("Select a package first.");
        return;
    };
    let name = row.widget_name().to_string();
    let names = vec![name];
    let args = if install {
        pacman_sync_args(&names)
    } else {
        pacman_remove_args(&names)
    };
    let args = match args {
        Ok(args) => args,
        Err(err) => {
            status.set_text(&err);
            return;
        }
    };
    let mut command = Command::new("pkexec");
    command.args(&args);
    match command.status() {
        Ok(code) if code.success() => {
            status.set_text("Transaction finished.");
            reload(list, status);
        }
        Ok(code) => status.set_text(&format!("pacman exited with {code}")),
        Err(err) => status.set_text(&err.to_string()),
    }
}
