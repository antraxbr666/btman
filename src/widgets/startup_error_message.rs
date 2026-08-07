use adw::gio::{ActionGroup, ActionMap};
use glib::Object;
use gtk::glib;
use gtk::subclass::prelude::*;
use adw::subclass::prelude::*;

mod imp {
    use super::*;
    use std::cell::RefCell;
    use gtk::prelude::*;
    use adw::prelude::*;

    #[derive(Default)]
    pub struct StartupErrorMessage {
        pub run_enable_bluetooth_button: RefCell<Option<gtk::Button>>,
        pub error_toast_overlay: RefCell<Option<adw::ToastOverlay>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for StartupErrorMessage {
        const NAME: &'static str = "StartupErrorMessage";
        type Type = super::StartupErrorMessage;
        type ParentType = adw::ApplicationWindow;
    }

    impl ObjectImpl for StartupErrorMessage {
        fn constructed(&self) {
            self.parent_constructed();

            let toast_overlay = adw::ToastOverlay::new();
            let toolbar_view = adw::ToolbarView::new();

            let header_bar = adw::HeaderBar::new();
            let window_title = adw::WindowTitle::new("Error", "");
            header_bar.set_title_widget(Some(&window_title));
            toolbar_view.add_top_bar(&header_bar);

            let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
            content.set_valign(gtk::Align::Fill);
            content.set_halign(gtk::Align::Fill);

            let status_page = adw::StatusPage::builder()
                .icon_name("face-angry-symbolic")
                .title("An Error Occurred")
                .description("this usually happens when the bluetooth service is disabled")
                .valign(gtk::Align::Start)
                .build();

            let run_button = gtk::Button::new();
            run_button.add_css_class("card");
            run_button.set_valign(gtk::Align::Center);
            run_button.set_halign(gtk::Align::Center);

            let button_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
            button_box.set_margin_top(16);
            button_box.set_margin_bottom(16);
            button_box.set_margin_start(16);
            button_box.set_margin_end(16);

            let label1 = gtk::Label::new(Some("in order to fix it, you could try:"));
            label1.set_use_markup(true);
            label1.set_justify(gtk::Justification::Center);
            button_box.append(&label1);

            let label2 = gtk::Label::new(Some(
                "`sudo systemctl enable --now bluetooth`\n`sudo systemctl start bluetooth`",
            ));
            label2.set_use_markup(true);
            label2.set_justify(gtk::Justification::Center);
            label2.set_margin_top(4);
            label2.set_margin_bottom(4);
            button_box.append(&label2);

            let label3 = gtk::Label::new(Some(
                "or install the <span font_weight='bold' color='#78aeed'>bluez</span> package for your distro\n and run the above commands, then restart <span font_weight='bold' color='#78aeed'>btman</span>",
            ));
            label3.set_use_markup(true);
            label3.set_justify(gtk::Justification::Center);
            button_box.append(&label3);

            run_button.set_child(Some(&button_box));

            let toast_overlay_clone = toast_overlay.clone();
            run_button.connect_clicked(move |_| {
                if std::env::var("container").is_err() {
                    let argv = [
                        std::ffi::OsStr::new("pkexec"),
                        std::ffi::OsStr::new("systemctl"),
                        std::ffi::OsStr::new("enable"),
                        std::ffi::OsStr::new("--now"),
                        std::ffi::OsStr::new("bluetooth"),
                    ];
                    let argv2 = [
                        std::ffi::OsStr::new("pkexec"),
                        std::ffi::OsStr::new("systemctl"),
                        std::ffi::OsStr::new("start"),
                        std::ffi::OsStr::new("bluetooth"),
                    ];

                    gtk::gio::Subprocess::newv(&argv, gtk::gio::SubprocessFlags::STDERR_PIPE)
                        .expect("cannot enable bluetooth by pkexec");
                    gtk::gio::Subprocess::newv(&argv2, gtk::gio::SubprocessFlags::STDERR_PIPE)
                        .expect("cannot enable bluetooth by pkexec");

                    let toast = adw::Toast::new("applying commands through pkexec");
                    toast.set_timeout(5);
                    toast_overlay_clone.add_toast(toast);
                } else {
                    let display = gtk::gdk::Display::default().unwrap();
                    let clipboard = gtk::prelude::DisplayExt::clipboard(&display);
                    clipboard.set_text("sudo systemctl enable --now bluetooth");

                    let toast = adw::Toast::new("copied command to clipboard");
                    toast.set_timeout(5);
                    toast_overlay_clone.add_toast(toast);
                }
            });

            status_page.set_child(Some(&run_button));
            content.append(&status_page);

            toolbar_view.set_content(Some(&content));
            toast_overlay.set_child(Some(&toolbar_view));

            self.obj().set_content(Some(&toast_overlay));

            *self.error_toast_overlay.borrow_mut() = Some(toast_overlay);
            *self.run_enable_bluetooth_button.borrow_mut() = Some(run_button);
        }
    }

    impl WidgetImpl for StartupErrorMessage {}
    impl WindowImpl for StartupErrorMessage {}
    impl ApplicationWindowImpl for StartupErrorMessage {}
    impl AdwApplicationWindowImpl for StartupErrorMessage {}
}

glib::wrapper! {
    pub struct StartupErrorMessage(ObjectSubclass<imp::StartupErrorMessage>)
        @extends adw::ApplicationWindow, gtk::Widget, gtk::Window, gtk::ApplicationWindow,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager, ActionGroup, ActionMap;
}

impl StartupErrorMessage {
    pub fn new() -> Self {
        Object::builder().build()
    }
}

impl Default for StartupErrorMessage {
    fn default() -> Self {
        Self::new()
    }
}
