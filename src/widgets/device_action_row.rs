use glib::{Object, Properties};
use gtk::glib;
use gtk::subclass::prelude::*;
use adw::subclass::prelude::*;
use adw::prelude::*;
use std::cell::RefCell;

mod imp {
    use super::*;

    #[derive(Properties, Default)]
    #[properties(wrapper_type = super::DeviceActionRow)]
    pub struct DeviceActionRow {
        pub address: RefCell<bluer::Address>,
        pub rssi: RefCell<i32>,
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
            self.obj().set_activatable(true);
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
    pub fn new() -> Self {
        Object::builder()
            .build()
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
}

fn rssi_to_percent(rssi: i32) -> i32 {
    let rssi = rssi.clamp(-100, -50);
    (rssi + 100) * 2
}

impl Default for DeviceActionRow {
    fn default() -> Self {
        Self::new()
    }
}
