use aegis_common::tour::{mark_tour_done, tour_slides};
use aegis_common::LIVE_MARKER;
use gtk4::gdk::Display;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    style_context_add_provider_for_display, Align, Application, ApplicationWindow, Box, Button,
    CssProvider, Label, Orientation, Stack, STYLE_PROVIDER_PRIORITY_APPLICATION,
};
use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

const STYLE: &str = include_str!("../assets/style.css");

fn main() -> glib::ExitCode {
    let app = Application::builder()
        .application_id("org.aegis.Tour")
        .build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &Application) {
    load_css();
    let live = std::path::Path::new(LIVE_MARKER).exists();
    let slides = tour_slides(live);
    let last = slides.len().saturating_sub(1);
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Welcome to Aegis OS")
        .default_width(720)
        .default_height(460)
        .decorated(false)
        .resizable(false)
        .build();
    window.add_css_class("tour");

    let root = Box::new(Orientation::Vertical, 16);
    root.set_margin_top(32);
    root.set_margin_bottom(24);
    root.set_margin_start(32);
    root.set_margin_end(32);
    let stack = Stack::new();
    stack.set_vexpand(true);
    stack.set_hexpand(true);
    for (index, slide) in slides.iter().enumerate() {
        let page = Box::new(Orientation::Vertical, 12);
        page.set_valign(Align::Center);
        let kicker = Label::new(Some(slide.kicker));
        kicker.add_css_class("kicker");
        kicker.set_xalign(0.0);
        let title = Label::new(Some(slide.title));
        title.add_css_class("title");
        title.set_xalign(0.0);
        title.set_wrap(true);
        let body = Label::new(Some(slide.body));
        body.add_css_class("body");
        body.set_xalign(0.0);
        body.set_wrap(true);
        body.set_max_width_chars(42);
        page.append(&kicker);
        page.append(&title);
        page.append(&body);
        stack.add_named(&page, Some(&index.to_string()));
    }

    let dots = Label::new(Some(&dot_text(0, slides.len())));
    dots.add_css_class("dots");
    dots.set_xalign(0.0);
    let back = Button::with_label("Back");
    back.set_sensitive(false);
    let next = Button::with_label(if last == 0 { "Start" } else { "Next" });
    next.add_css_class("suggested");
    let skip = Button::with_label("Skip");
    let actions = Box::new(Orientation::Horizontal, 8);
    actions.set_halign(Align::End);
    actions.append(&skip);
    actions.append(&back);
    actions.append(&next);
    root.append(&stack);
    root.append(&dots);
    root.append(&actions);
    window.set_child(Some(&root));

    let index = Rc::new(Cell::new(0usize));
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let count = slides.len();
    let finish = Rc::new({
        let window = window.clone();
        let home = home.clone();
        move || {
            if let Some(home) = &home {
                let _ = mark_tour_done(home);
            }
            window.close();
        }
    });

    {
        let home = home.clone();
        window.connect_close_request(move |_| {
            if let Some(home) = &home {
                let _ = mark_tour_done(home);
            }
            glib::Propagation::Proceed
        });
    }

    let skip_finish = finish.clone();
    skip.connect_clicked(move |_| skip_finish.as_ref()());

    let show_page = Rc::new({
        let stack = stack.clone();
        let dots = dots.clone();
        let back = back.clone();
        let next = next.clone();
        let index = index.clone();
        move |page: usize| {
            index.set(page);
            stack.set_visible_child_name(&page.to_string());
            dots.set_text(&dot_text(page, count));
            back.set_sensitive(page > 0);
            next.set_label(if page + 1 == count {
                "Start using Aegis"
            } else {
                "Next"
            });
        }
    });

    let show_back = show_page.clone();
    let index_back = index.clone();
    back.connect_clicked(move |_| {
        let page = index_back.get();
        if page > 0 {
            show_back(page - 1);
        }
    });

    let show_next = show_page.clone();
    let index_next = index.clone();
    next.connect_clicked(move |_| {
        let page = index_next.get();
        if page + 1 >= count {
            finish.as_ref()();
            return;
        }
        show_next(page + 1);
    });

    window.present();
}

fn dot_text(current: usize, count: usize) -> String {
    (0..count)
        .map(|index| if index == current { "●" } else { "○" })
        .collect::<Vec<_>>()
        .join(" ")
}

fn load_css() {
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
}
