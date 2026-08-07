use adw::prelude::*;
use adw::subclass::prelude::*;
use async_channel::Sender;
use glib::{Object, Properties};
use gtk::glib;
use std::cell::RefCell;

use crate::message::Message;

mod imp {
    use super::*;

    #[derive(Properties, Default)]
    #[properties(wrapper_type = super::PairedDeviceRow)]
    pub struct PairedDeviceRow {
        pub address: RefCell<bluer::Address>,
        pub connect_button: RefCell<Option<gtk::Button>>,
        pub forget_button: RefCell<Option<gtk::Button>>,
        pub name_label: RefCell<Option<gtk::Label>>,
        pub spinner: RefCell<Option<gtk::Spinner>>,
        pub connected: RefCell<bool>,
        pub sender: RefCell<Option<Sender<Message>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PairedDeviceRow {
        const NAME: &'static str = "PairedDeviceRow";
        type Type = super::PairedDeviceRow;
        type ParentType = adw::ActionRow;
    }

    #[glib::derived_properties]
    impl ObjectImpl for PairedDeviceRow {
        fn constructed(&self) {
            self.parent_constructed();
        }
    }

    impl WidgetImpl for PairedDeviceRow {}
    impl ListBoxRowImpl for PairedDeviceRow {}
    impl PreferencesRowImpl for PairedDeviceRow {}
    impl ActionRowImpl for PairedDeviceRow {}
}

glib::wrapper! {
    pub struct PairedDeviceRow(ObjectSubclass<imp::PairedDeviceRow>)
        @extends adw::ActionRow, gtk::Widget, gtk::ListBoxRow, adw::PreferencesRow,
        @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget;
}

impl PairedDeviceRow {
    pub fn new(name: &str, address: bluer::Address, sender: Sender<Message>, connected: bool) -> Self {
        let obj: PairedDeviceRow = Object::builder().build();
        obj.set_title(name);
        obj.set_subtitle("Not in range");

        let button = gtk::Button::new();
        if connected {
            button.remove_css_class("suggested-action");
        } else {
            button.add_css_class("suggested-action");
        }

        let label = gtk::Label::new(Some(if connected { "Disconnect" } else { "Connect" }));
        let spinner = gtk::Spinner::new();
        spinner.start();
        spinner.set_visible(false);
        spinner.set_width_request(16);
        spinner.set_height_request(16);

        let button_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        button_box.append(&spinner);
        button_box.append(&label);
        button.set_child(Some(&button_box));

        obj.add_suffix(&button);

        let forget_button = gtk::Button::from_icon_name("edit-delete-symbolic");
        forget_button.add_css_class("flat");
        forget_button.set_tooltip_text(Some("Forget device"));
        obj.add_suffix(&forget_button);

        obj.set_activatable_widget(Some(&button));

        *obj.imp().connect_button.borrow_mut() = Some(button);
        *obj.imp().name_label.borrow_mut() = Some(label);
        *obj.imp().spinner.borrow_mut() = Some(spinner);
        *obj.imp().forget_button.borrow_mut() = Some(forget_button);
        *obj.imp().address.borrow_mut() = address;
        *obj.imp().sender.borrow_mut() = Some(sender);
        *obj.imp().connected.borrow_mut() = connected;

        obj.connect_connect_button();
        obj.connect_forget_button();
        obj
    }

    pub fn get_bluer_address(&self) -> bluer::Address {
        *self.imp().address.borrow()
    }

    pub fn set_rssi(&self, rssi: i32) {
        self.set_subtitle(&format!("Signal: {}%", rssi_to_percent(rssi)));
    }

    pub fn set_connected(&self, connected: bool) {
        *self.imp().connected.borrow_mut() = connected;
        if let Some(label) = self.imp().name_label.borrow().as_ref() {
            label.set_label(if connected { "Disconnect" } else { "Connect" });
        }
        if let Some(button) = self.imp().connect_button.borrow().as_ref() {
            if connected {
                button.remove_css_class("suggested-action");
            } else {
                button.add_css_class("suggested-action");
            }
        }
    }

    pub fn set_spinning(&self, spinning: bool) {
        if let Some(spinner) = self.imp().spinner.borrow().as_ref() {
            if spinning {
                spinner.set_visible(true);
            } else {
                spinner.stop();
                spinner.set_visible(false);
            }
        }
        if let Some(label) = self.imp().name_label.borrow().as_ref() {
            label.set_visible(!spinning);
        }
    }

    fn connect_connect_button(&self) {
        let this = self.clone();
        let button = self
            .imp()
            .connect_button
            .borrow()
            .as_ref()
            .unwrap()
            .clone();

        button.connect_clicked(move |_| {
            let this = this.clone();
            if this
                .imp()
                .spinner
                .borrow()
                .as_ref()
                .map(|s| s.is_visible())
                .unwrap_or(false)
            {
                return;
            }

            let address = this.get_bluer_address();
            let adapter_name = crate::window::BTMAN_PROPS
                .lock()
                .unwrap()
                .current_adapter
                .clone();
            let sender = this.imp().sender.borrow().as_ref().unwrap().clone();

            crate::window::runtime().spawn(async move {
                if let Err(err) =
                    crate::device::set_device_active(address, sender.clone(), adapter_name).await
                {
                    let string = err.message;
                    sender
                        .send(Message::SwitchActiveSpinner(false, address))
                        .await
                        .expect("cannot send message");
                    sender
                        .send(Message::PopupError(string, adw::ToastPriority::High))
                        .await
                        .expect("cannot send message");
                }
            });
        });
    }

    fn connect_forget_button(&self) {
        let this = self.clone();
        let forget_button = self
            .imp()
            .forget_button
            .borrow()
            .as_ref()
            .unwrap()
            .clone();

        forget_button.connect_clicked(move |_| {
            let this = this.clone();
            let address = this.get_bluer_address();
            let adapter_name = crate::window::BTMAN_PROPS
                .lock()
                .unwrap()
                .current_adapter
                .clone();
            let sender = this.imp().sender.borrow().as_ref().unwrap().clone();

            crate::window::runtime().spawn(async move {
                if let Err(err) =
                    crate::device::remove_device(address, sender.clone(), adapter_name).await
                {
                    let string = err.message;
                    sender
                        .send(Message::PopupError(string, adw::ToastPriority::High))
                        .await
                        .expect("cannot send message");
                }
            });
        });
    }
}

fn rssi_to_percent(rssi: i32) -> i32 {
    let rssi = rssi.clamp(-100, -50);
    (rssi + 100) * 2
}
