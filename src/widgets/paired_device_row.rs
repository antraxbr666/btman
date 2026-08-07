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
    pub fn new(name: &str, address: bluer::Address, sender: Sender<Message>) -> Self {
        let obj: PairedDeviceRow = Object::builder().build();
        obj.set_title(name);
        obj.set_subtitle("Not in range");

        let button = gtk::Button::with_label("Connect");
        button.add_css_class("suggested-action");
        obj.add_suffix(&button);
        obj.set_activatable_widget(Some(&button));

        *obj.imp().connect_button.borrow_mut() = Some(button);
        *obj.imp().address.borrow_mut() = address;
        *obj.imp().sender.borrow_mut() = Some(sender);
        *obj.imp().connected.borrow_mut() = false;

        obj.connect_connect_button();
        obj
    }

    pub fn get_bluer_address(&self) -> bluer::Address {
        *self.imp().address.borrow()
    }

    pub fn set_rssi(&self, rssi: i32) {
        self.set_subtitle(&format!("{} dBm", rssi));
    }

    pub fn set_connected(&self, connected: bool) {
        *self.imp().connected.borrow_mut() = connected;
        if let Some(button) = self.imp().connect_button.borrow().as_ref() {
            button.set_label(if connected { "Disconnect" } else { "Connect" });
            if connected {
                button.remove_css_class("suggested-action");
            } else {
                button.add_css_class("suggested-action");
            }
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
                        .send(Message::PopupError(string, adw::ToastPriority::High))
                        .await
                        .expect("cannot send message");
                }
            });
        });
    }
}
