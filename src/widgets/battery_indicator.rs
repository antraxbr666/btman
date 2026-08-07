use gtk::glib;
use gtk::subclass::prelude::*;
use glib::{Object, Properties};
use gtk::prelude::ObjectExt;
use std::cell::RefCell;

mod imp {
    use super::*;
    use adw::subclass::prelude::*;
    use gtk::prelude::*;

    #[derive(Properties, Default)]
    #[properties(wrapper_type = super::BatteryLevelIndicator)]
    pub struct BatteryLevelIndicator {
        pub label: RefCell<gtk::Label>,
        pub level_bar: RefCell<gtk::LevelBar>,

        #[property(get, set = Self::set_battery_level_from_i8)]
        pub battery_level: RefCell<i8>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for BatteryLevelIndicator {
        const NAME: &'static str = "BatteryLevelIndicator";
        type Type = super::BatteryLevelIndicator;
        type ParentType = adw::PreferencesRow;
    }

    #[glib::derived_properties]
    impl ObjectImpl for BatteryLevelIndicator {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj();
            let container = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            container.set_margin_top(4);
            container.set_margin_bottom(4);
            container.set_margin_start(12);
            container.set_margin_end(12);

            let label = gtk::Label::new(Some("Battery: Unavailable"));
            label.set_halign(gtk::Align::Start);
            label.set_hexpand(true);
            container.append(&label);

            let level_bar = gtk::LevelBar::new();
            level_bar.set_min_value(0.0);
            level_bar.set_max_value(100.0);
            level_bar.set_value(0.0);
            level_bar.set_halign(gtk::Align::End);
            level_bar.add_offset_value("full", 100.0);
            level_bar.add_offset_value("three-quarters", 75.0);
            level_bar.add_offset_value("half", 50.0);
            level_bar.add_offset_value("third", 25.0);
            container.append(&level_bar);

            self.label.replace(label);
            self.level_bar.replace(level_bar);

            obj.set_child(Some(&container));
        }
    }

    impl WidgetImpl for BatteryLevelIndicator {}
    impl ListBoxRowImpl for BatteryLevelIndicator {}
    impl PreferencesRowImpl for BatteryLevelIndicator {}

    impl BatteryLevelIndicator {
        pub fn set_battery_level_from_i8(&self, level: i8) {
            let level = level.clamp(-1, 100);
            let levelbar = self.level_bar.borrow();
            let label = self.label.borrow();

            if level == -1 {
                levelbar.set_value(100.0);
                levelbar.set_sensitive(false);
                label.set_label("Battery: Unavailable");
            } else {
                levelbar.set_sensitive(true);
                levelbar.set_value(level as f64);
                label.set_label(&format!("Battery: {}%", level));
            }

            self.battery_level.replace(level);
        }
    }
}

glib::wrapper! {
    pub struct BatteryLevelIndicator(ObjectSubclass<imp::BatteryLevelIndicator>)
        @extends adw::PreferencesRow, gtk::Widget, gtk::ListBoxRow,
        @implements gtk::Accessible, gtk::Orientable, gtk::Buildable, gtk::ConstraintTarget, gtk::Actionable;
}

impl BatteryLevelIndicator {
    pub fn new() -> Self {
        Object::builder().build()
    }

    pub fn set_indicator_battery_level(&self, level: i8) {
        self.imp().set_battery_level_from_i8(level);
    }
}

impl Default for BatteryLevelIndicator {
    fn default() -> Self {
        Self::new()
    }
}
