use glib::{Object, Properties};
use gtk::glib;
use gtk::subclass::prelude::*;
use adw::subclass::prelude::*;
use adw::prelude::*;
use std::cell::RefCell;

use async_channel::Sender;
use crate::message::Message;

mod imp {
    use super::*;

    #[derive(Properties, Default)]
    #[properties(wrapper_type = super::DeviceActionRow)]
    pub struct DeviceActionRow {
        pub address: RefCell<bluer::Address>,
        pub rssi: RefCell<i32>,
        pub pair_button: RefCell<Option<gtk::Button>>,
        pub name_label: RefCell<Option<gtk::Label>>,
        pub spinner: RefCell<Option<gtk::Spinner>>,
        pub sender: RefCell<Option<Sender<Message>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for DeviceActionRow {
        const NAME: &'static str = "DeviceActionRow";
        type Type = super::DeviceActionRow;
        type ParentType = adw::ActionRow;
    }

    #[glib::derived_properties]
    impl ObjectImpl for DeviceActionRow {
        fn constructed(&self) {
            self.parent_constructed();
        }
    }

    impl WidgetImpl for DeviceActionRow {}
    impl ListBoxRowImpl for DeviceActionRow {}
    impl PreferencesRowImpl for DeviceActionRow {}
    impl ActionRowImpl for DeviceActionRow {}
}

glib::wrapper! {
    pub struct DeviceActionRow(ObjectSubclass<imp::DeviceActionRow>)
        @extends adw::ActionRow, gtk::Widget, gtk::ListBoxRow, adw::PreferencesRow,
        @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget;
}

impl DeviceActionRow {
    pub fn new(sender: Sender<Message>) -> Self {
        let obj: DeviceActionRow = Object::builder().build();

        let button = gtk::Button::new();
        button.add_css_class("suggested-action");

        let label = gtk::Label::new(Some("Pair"));
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
        obj.set_activatable_widget(Some(&button));

        *obj.imp().pair_button.borrow_mut() = Some(button);
        *obj.imp().name_label.borrow_mut() = Some(label);
        *obj.imp().spinner.borrow_mut() = Some(spinner);
        *obj.imp().sender.borrow_mut() = Some(sender);

        obj.connect_pair_button();
        obj
    }

    pub fn get_bluer_address(&self) -> bluer::Address {
        *self.imp().address.borrow()
    }

    pub fn set_bluer_address(&self, address: bluer::Address) {
        *self.imp().address.borrow_mut() = address;
    }

    pub fn set_rssi(&self, rssi: i32) {
        *self.imp().rssi.borrow_mut() = rssi;
        self.set_subtitle(&format!("Signal: {}%", rssi_to_percent(rssi)));
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

    fn connect_pair_button(&self) {
        let this = self.clone();
        let button = self
            .imp()
            .pair_button
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
}

fn rssi_to_percent(rssi: i32) -> i32 {
    let rssi = rssi.clamp(-100, -50);
    (rssi + 100) * 2
}