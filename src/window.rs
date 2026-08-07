/* window.rs
 *
 * Copyright 2023 kaii
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */
use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::gio::Settings;
use gtk::glib::clone;
use gtk::{gio, glib, Accessible, Buildable, ConstraintTarget, Grid, Native, Root, ShortcutManager, Widget, Window};

use crate::agent::register_bluetooth_agent;
use crate::application::BtmanApplication;
use crate::device_action_row::DeviceActionRow;
use crate::message::Message;
use crate::startup_error_message::StartupErrorMessage;
use crate::{bluetooth_settings, device};
use crate::singletons::BtmanProperties;

use adw::glib::wrapper;
use async_channel::Sender;
use std::cell::{OnceCell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::{Mutex, OnceLock};
use tokio::runtime::Runtime;
use lazy_static::lazy_static;

use crate::bluetooth_state::{devices_lut, adapters_lut};

lazy_static! {
    pub static ref BTMAN_PROPS: Mutex<BtmanProperties> = Mutex::new(BtmanProperties::new());
}

mod imp {
    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/io.github.antraxbr666.Btman/gtk/window.ui")]
    pub struct BtmanWindow {
        #[template_child]
        pub main_listbox: TemplateChild<gtk::ListBox>,
        #[template_child]
        pub powered_switch_row: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub discoverable_switch_row: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub listbox_image_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub window_title: TemplateChild<adw::WindowTitle>,
        #[template_child]
        pub bluetooth_group: TemplateChild<adw::PreferencesGroup>,

        pub battery_level_indicator: RefCell<Option<crate::battery_indicator::BatteryLevelIndicator>>,
        pub settings: OnceCell<Settings>,
        pub display_pass_key_dialog: RefCell<Option<adw::MessageDialog>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for BtmanWindow {
        const NAME: &'static str = "BtmanWindow";
        type Type = super::BtmanWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for BtmanWindow {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.setup_settings();
            obj.preload_settings();
        }
    }
    impl WidgetImpl for BtmanWindow {}

    impl WindowImpl for BtmanWindow {
        fn close_request(&self) -> glib::Propagation {
            self.obj().save_settings().expect("cannot save window size");
            glib::Propagation::Proceed
        }
    }
    impl ApplicationWindowImpl for BtmanWindow {}
    impl AdwApplicationWindowImpl for BtmanWindow {}
}

wrapper! {
    pub struct BtmanWindow(ObjectSubclass<imp::BtmanWindow>)
        @extends Widget, adw::ApplicationWindow, BtmanApplication,
        @implements gio::ActionGroup, gio::ActionMap, Accessible, Buildable, ConstraintTarget, Native, Root, ShortcutManager, gtk::ApplicationWindow, Grid, Window;
}

pub fn runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        Runtime::new().expect("Setting up tokio runtime needs to succeed.")
    })
}

impl BtmanWindow {
    pub fn new<P: IsA<gtk::Application>>(application: &P) -> Self {
        let win: BtmanWindow = glib::Object::builder()
            .property("application", application)
            .build();

        win.setup();
        win
    }

    fn setup_settings(&self) {
        let settings = Settings::new("io.github.antraxbr666.Btman");
        self.imp()
            .settings
            .set(settings)
            .expect("settings not setup");
    }

    fn setup(&self) {
        let (sender, receiver) = async_channel::unbounded::<Message>();

        if let Err(err) = self.pre_setup(sender.clone()) {
            println!("ERROR: cannot start presetup, something got REALLY fucked");
            println!("error is: {:?}", err);

            let clone = self.clone();
            let message = StartupErrorMessage::new();

            message.set_transient_for(Some(&clone));
            message.set_modal(true);

            message.connect_destroy(move |_| {
                WidgetExt::activate_action(&clone, "app.quit", None)
                    .expect("cannot exit app on message close");
            });

            message.set_visible(true);
            return;
        }

        // Create battery level indicator and add it to the Bluetooth group
        let battery = crate::battery_indicator::BatteryLevelIndicator::new();
        self.imp().bluetooth_group.add(&battery);
        *self.imp().battery_level_indicator.borrow_mut() = Some(battery);

        let self_clone = self.clone();

        glib::MainContext::default().spawn_local(async move {
            while let Ok(msg) = receiver.recv().await {
                let clone = self_clone.clone();

                match msg {
                    Message::SwitchActive(_active, address, is_current) => {
                        let _ = is_current;
                        let listbox = clone.imp().main_listbox.get();
                        let mut listbox_index = 0;

                        while let Some(row) = listbox.row_at_index(listbox_index) {
                            let action_row = row.downcast::<DeviceActionRow>().expect("cannot downcast to action row.");
                            if action_row.get_bluer_address() == address {
                                // just update row state if needed
                            } else if address == bluer::Address::any() {
                                // clear all
                            }

                            listbox_index += 1;
                        }
                    }
                    Message::SwitchActiveSpinner(_spinning) => {}
                    Message::SwitchRssi(_device_name, _rssi) => {}
                    Message::AddRow(device) => {
                        let row = add_child_row(device);

                        if let Ok(ok_row) = row {
                            let main_listbox = clone.imp().main_listbox.get();
                            main_listbox.append(&ok_row);
                            main_listbox.invalidate_sort();
                        }
                    }
                    Message::RemoveDevice(name, address) => {
                        let listbox = clone.imp().main_listbox.get();
                        let mut index = 0;

                        while let Some(row) = listbox.row_at_index(index) {
                            let action_row = row.downcast::<DeviceActionRow>().expect("cannot downcast to action row.");

                            if action_row.title() == name && action_row.get_bluer_address() == address {
                                listbox.remove(&action_row);
                            }
                            index += 1;
                        }

                        listbox.invalidate_sort();
                    }
                    Message::SwitchAdapterPowered(powered) => {
                        let powered_switch_row = clone.imp().powered_switch_row.get();
                        powered_switch_row.set_active(powered);
                    }
                    Message::SwitchAdapterDiscoverable(discoverable) => {
                        let discoverable_switch_row = clone.imp().discoverable_switch_row.get();
                        discoverable_switch_row.set_active(discoverable);
                    }
                    Message::PopupError(string, priority) => {
                        let toast_overlay = clone.imp().toast_overlay.get();
                        let toast = adw::Toast::new("");

                        toast.set_priority(priority);

                        let title_holder = match string {
                            s if s.to_lowercase().contains("page-timeout") => {
                                "Failed to connect to device, connection timed out"
                            }
                            s if s.to_lowercase().contains("already-connected") => {
                                "Device is already connected"
                            }
                            s if s.to_lowercase().contains("busy") => {
                                "Other operations pending, please try again in a bit"
                            }
                            s if s.to_lowercase().contains("limit") => {
                                "Reached limit, cannot connect to anymore devices"
                            }
                            s if s.to_lowercase().contains("connection-timeout") => {
                                "Failed to connect to device, connection timed out"
                            }
                            s if s.to_lowercase().contains("refused") => {
                                "Connection was refused by target device"
                            }
                            s if s.to_lowercase().contains("aborted-by-remote") => {
                                "Target device aborted connection"
                            }
                            s if s.to_lowercase().contains("aborted-by-local") => {
                                "Connection has been aborted"
                            }
                            s if s.to_lowercase().contains("canceled") => {
                                "Connection was canceled due to unforeseen circumstances"
                            }
                            s if s.to_lowercase().contains("unknown-error") => {
                                "Connection failed, no idea why"
                            }
                            s if s.to_lowercase().contains("not-powered") || s.to_lowercase().contains("resource not ready") => {
                                "Adapter is not powered"
                            }
                            s if s.to_lowercase().contains("not-supported") => {
                                "Connection failed, requested features are not supported"
                            }
                            s if s.to_lowercase().contains("refreshed") => {
                                "Refreshed devices list"
                            }
                            s if s.to_lowercase().contains("stopped searching for devices") => {
                                "Stopped searching for devices"
                            }
                            s if s.to_lowercase().contains("refresh-adapter-failed") => {
                                "Unable to refresh devices list after adapter change"
                            }
                            e => {
                                println!("unknown error: {}", e.clone());
                                "Unknown error occurred"
                            }
                        };

                        let mut title = String::new();
                        let boxholder = gtk::Box::new(gtk::Orientation::Horizontal, 8);

                        toast.set_timeout(3);
                        match priority {
                            adw::ToastPriority::High => {
                                title += "<span font_weight='bold'>";

                                let icon = gtk::Image::new();
                                icon.set_icon_name(Some("notification-symbolic"));
                                boxholder.append(&icon);
                            }
                            _ => {
                                title += "<span font_weight='regular'>";
                            }
                        }
                        let label = gtk::Label::new(Some(""));
                        boxholder.append(&label);

                        title += title_holder;
                        title += "</span>";

                        label.set_use_markup(true);
                        label.set_label(&title);

                        toast.set_custom_title(Some(&boxholder));

                        toast_overlay.add_toast(toast);
                    }
                    Message::UpdateListBoxImage() => {
                        let listbox_image_box = clone.imp().listbox_image_box.get();
                        let main_listbox = clone.imp().main_listbox.get();

                        let exists = main_listbox.row_at_index(0).is_some();

                        if exists {
                            listbox_image_box.set_visible(false);
                            main_listbox.set_visible(true);
                        } else {
                            listbox_image_box.set_visible(true);
                            main_listbox.set_visible(false);
                        }
                    }
                    Message::RequestPinCode(request) => {
                        let device: String;
                        let adapter: String;
                        device = devices_lut().lock().unwrap().as_ref().unwrap().get(&request.device).unwrap_or(&"Unknown Device".to_string()).to_string();
                        adapter = adapters_lut().lock().unwrap().as_ref().unwrap().get(&request.adapter).unwrap_or(&"Unknown Adapter".to_string()).to_string();
                        BTMAN_PROPS.lock().unwrap().displaying_dialog = true;

                        let body = device + " has requested pairing on " + adapter.as_str() + ", please enter the correct pin code.";
                        let popup = adw::MessageDialog::new(Some(&clone), Some("Pin Code Requested"), Some(body.as_str()));
                        let popup2 = popup.clone();

                        popup.set_destroy_with_parent(true);

                        popup.add_response("cancel", "Cancel");
                        popup.add_response("confirm", "Confirm");
                        popup.set_response_appearance("confirm", adw::ResponseAppearance::Suggested);
                        popup.set_default_response(Some("confirm"));
                        popup.set_close_response("cancel");

                        let entry = gtk::Entry::new();
                        entry.set_placeholder_text(Some("12345 or abcde"));
                        popup.set_extra_child(Some(&entry));
                        popup.set_response_enabled("confirm", false);

                        entry.connect_changed(move |entry| {
                            let is_empty = entry.text().is_empty();
                            popup.set_response_enabled("confirm", !is_empty);
                            if is_empty {
                                entry.add_css_class("error");
                            } else {
                                entry.remove_css_class("error");
                            }
                        });
                        entry.add_css_class("error");

                        let pin_code = Rc::new(RefCell::new(String::new()));
                        popup2.clone().choose(gio::Cancellable::NONE, move |response| {
                            match response.to_string() {
                                s if s.contains("confirm") => {
                                    *pin_code.borrow_mut() = entry.text().to_string();
                                }
                                _ => {
                                    *pin_code.borrow_mut() = String::new();
                                }
                            }
                            BTMAN_PROPS.lock().unwrap().displaying_dialog = false;
                            BTMAN_PROPS.lock().unwrap().pin_code = pin_code.borrow().clone();
                        });
                    }
                    Message::DisplayPinCode(request) => {
                        let pin_code = &request.pincode;
                        let device: String;
                        device = devices_lut().lock().unwrap().as_ref().unwrap().get(&request.device).unwrap_or(&"Unknown Device".to_string()).to_string();
                        BTMAN_PROPS.lock().unwrap().displaying_dialog = true;

                        let body = "Please enter this pin code on ".to_string() + device.as_str();
                        let popup = adw::MessageDialog::new(Some(&clone), None, Some(body.as_str()));

                        let label = gtk::Label::new(Some(pin_code.as_str()));

                        popup.set_extra_child(Some(&label));
                        popup.add_response("okay", "Okay");
                        popup.set_close_response("okay");

                        popup.clone().choose(
                            gio::Cancellable::NONE, move |_| {
                                BTMAN_PROPS.lock().unwrap().displaying_dialog = false;
                            });
                    }
                    Message::RequestPassKey(request) => {
                        let device: String;
                        let adapter: String;
                        device = devices_lut().lock().unwrap().as_ref().unwrap().get(&request.device).unwrap_or(&"Unknown Device".to_string()).to_string();
                        adapter = adapters_lut().lock().unwrap().as_ref().unwrap().get(&request.adapter).unwrap_or(&"Unknown Adapter".to_string()).to_string();
                        BTMAN_PROPS.lock().unwrap().displaying_dialog = true;

                        let body = device + " has requested pairing on " + adapter.as_str() + ", please enter the correct pass key.";
                        let popup = adw::MessageDialog::new(Some(&clone), Some("Pass Key Requested"), Some(body.as_str()));
                        let popup2 = popup.clone();

                        popup.set_close_response("cancel");
                        popup.set_destroy_with_parent(true);

                        popup.add_response("cancel", "Cancel");
                        popup.add_response("confirm", "Confirm");
                        popup.set_response_appearance("confirm", adw::ResponseAppearance::Suggested);
                        popup.set_default_response(Some("confirm"));

                        let entry = gtk::Entry::new();
                        entry.set_placeholder_text(Some("0-999999"));
                        entry.set_input_purpose(gtk::InputPurpose::Digits);
                        entry.set_max_length(6);

                        popup.set_extra_child(Some(&entry));
                        popup.set_response_enabled("confirm", false);

                        entry.connect_changed(clone!(move |entry| {
                            let is_empty = entry.text().is_empty();
                            popup.set_response_enabled("confirm", !is_empty);
                            if is_empty {
                                entry.add_css_class("error");
                            } else {
                                entry.remove_css_class("error");
                            }
                        }));
                        entry.add_css_class("error");

                        let pass_key = Rc::new(RefCell::new(String::new()));
                        popup2.clone().choose(gio::Cancellable::NONE, move |response| {
                            match response.to_string() {
                                s if s.contains("confirm") => {
                                    *pass_key.borrow_mut() = entry.text().to_string();
                                }
                                _ => {
                                    *pass_key.borrow_mut() = String::new();
                                }
                            }

                            BTMAN_PROPS.lock().unwrap().displaying_dialog = false;
                            BTMAN_PROPS.lock().unwrap().pass_key = pass_key.borrow().parse::<u32>().unwrap_or(0);
                        });
                    }
                    Message::DisplayPassKey(request) => {
                        let pin_code = &request.passkey;
                        let device: String;
                        device = devices_lut().lock().unwrap().as_ref().unwrap().get(&request.device).unwrap_or(&"Unknown Device".to_string()).to_string();
                        BTMAN_PROPS.lock().unwrap().displaying_dialog = true;

                        if clone.imp().display_pass_key_dialog.borrow().clone().is_some() {
                            let dialog = clone.imp().display_pass_key_dialog.borrow().clone().unwrap();
                            let label = dialog.extra_child().unwrap().downcast::<gtk::Label>().unwrap();
                            label.set_text(pin_code.to_string().as_str());
                        } else {
                            let body = "Please enter this pin code on ".to_string() + device.as_str();
                            let popup = adw::MessageDialog::new(Some(&clone), None, Some(body.as_str()));

                            let label = gtk::Label::new(Some(pin_code.to_string().as_str()));

                            popup.set_extra_child(Some(&label));
                            popup.add_response("okay", "Okay");
                            popup.set_close_response("okay");

                            popup.clone().choose(gio::Cancellable::NONE, move |_| {
                                BTMAN_PROPS.lock().unwrap().displaying_dialog = false;
                            });
                            *clone.imp().display_pass_key_dialog.borrow_mut() = Some(popup.clone());
                        }
                    }
                    Message::RequestConfirmation(request) => {
                        let device: String;
                        let adapter: String;
                        let passkey = &request.passkey.to_string();
                        BTMAN_PROPS.lock().unwrap().displaying_dialog = true;
                        device = devices_lut().lock().unwrap().as_ref().unwrap().get(&request.device).unwrap_or(&"Unknown Device".to_string()).to_string();
                        let mut holder = String::new();
                        for key in adapters_lut().lock().unwrap().as_ref().unwrap().keys() {
                            if let Some(pair) = adapters_lut().lock().unwrap().as_ref().unwrap().get_key_value(key) {
                                if pair.1 == &request.adapter {
                                    holder = pair.0.to_string();
                                }
                            }
                        }
                        if holder.is_empty() {
                            adapter = "Unknown Adapter".to_string();
                        } else {
                            adapter = holder;
                        }

                        let body = "Is this the right code for ".to_string() + device.as_str() + " on " + adapter.as_str();
                        let popup = adw::MessageDialog::new(Some(&clone), Some("Pairing Request"), None);
                        popup.set_body_use_markup(true);
                        popup.set_body(body.as_str());

                        popup.set_close_response("cancel");
                        popup.set_destroy_with_parent(true);

                        popup.add_response("cancel", "Cancel");
                        popup.add_response("allow", "Allow");
                        popup.set_response_appearance("allow", adw::ResponseAppearance::Suggested);
                        popup.set_default_response(Some("allow"));

                        let string = "<span font_weight='bold' font_size='32pt'>".to_string() + passkey + "</span>";
                        let label = gtk::Label::new(None);
                        label.set_use_markup(true);
                        label.set_label(string.as_str());

                        popup.set_extra_child(Some(&label));

                        let pass_key = Rc::new(RefCell::new(false));
                        popup.clone().choose(gio::Cancellable::NONE, move |response| {
                            match response.to_string() {
                                s if s.contains("allow") => {
                                    *pass_key.borrow_mut() = true;
                                }
                                _ => {
                                    *pass_key.borrow_mut() = false;
                                }
                            }

                            BTMAN_PROPS.lock().unwrap().displaying_dialog = false;
                            BTMAN_PROPS.lock().unwrap().confirm_authorization = *pass_key.borrow();
                        });
                    }
                    Message::RequestAuthorization(request) => {
                        let device: String;
                        let adapter: String;
                        device = devices_lut().lock().unwrap().as_ref().unwrap().get(&request.device).unwrap_or(&"Unknown Device".to_string()).to_string();
                        adapter = adapters_lut().lock().unwrap().as_ref().unwrap().get(&request.adapter).unwrap_or(&"Unknown Adapter".to_string()).to_string();
                        BTMAN_PROPS.lock().unwrap().displaying_dialog = true;

                        let body = "Is `".to_string() + device.as_str() + "` on `" + adapter.as_str() + "` allowed to pair?";
                        let popup = adw::MessageDialog::new(Some(&clone), Some("Pairing Request"), None);
                        popup.set_body_use_markup(true);
                        popup.set_body(body.as_str());

                        popup.set_close_response("cancel");
                        popup.set_destroy_with_parent(true);

                        popup.add_response("cancel", "Cancel");
                        popup.add_response("allow", "Allow");
                        popup.set_response_appearance("allow", adw::ResponseAppearance::Suggested);
                        popup.set_default_response(Some("allow"));

                        let pass_key = Rc::new(RefCell::new(false));
                        popup.clone().choose(gio::Cancellable::NONE, move |response| {
                            match response.to_string() {
                                s if s.contains("allow") => {
                                    *pass_key.borrow_mut() = true;
                                }
                                _ => {
                                    *pass_key.borrow_mut() = false;
                                }
                            }

                            BTMAN_PROPS.lock().unwrap().displaying_dialog = false;
                            BTMAN_PROPS.lock().unwrap().confirm_authorization = *pass_key.borrow();
                        });
                    }
                    Message::AuthorizeService(request) => {
                        let device: String;
                        let adapter: String;
                        BTMAN_PROPS.lock().unwrap().displaying_dialog = true;
                        device = devices_lut().lock().unwrap().as_ref().unwrap().get(&request.device).unwrap_or(&"Unknown Device".to_string()).to_string();
                        adapter = adapters_lut().lock().unwrap().as_ref().unwrap().iter()
                            .find_map(|(key, val)| if val == &request.adapter { Some(key) } else { None })
                            .unwrap_or(&"Unknown Adapter".to_string()).to_string();

                        let service_id = format!("{:?}", request.service);

                        let popup = adw::MessageDialog::new(Some(&clone), Some("Service Authorization Request"), None);

                        let body = "Is <span font_weight='bold' color='#78aeed'>`".to_string() + service_id.as_str() + "`</span> allowed to be authorized?\nRequest by <span font_weight='bold'>`" + device.as_str() + "`</span> on <span font_weight='bold'>`" + adapter.as_str() + "`</span>.";
                        let label = gtk::Label::new(Some(""));
                        label.set_use_markup(true);
                        label.set_label(body.as_str());
                        popup.set_extra_child(Some(&label));

                        popup.set_close_response("cancel");
                        popup.set_modal(true);
                        popup.set_destroy_with_parent(true);

                        popup.add_response("cancel", "Cancel");
                        popup.add_response("allow", "Allow");
                        popup.set_response_appearance("allow", adw::ResponseAppearance::Suggested);
                        popup.set_default_response(Some("allow"));

                        let pass_key = Rc::new(RefCell::new(false));
                        popup.clone().choose(gio::Cancellable::NONE, move |response| {
                            match response.to_string() {
                                s if s.contains("allow") => {
                                    *pass_key.borrow_mut() = true;
                                }
                                _ => {
                                    *pass_key.borrow_mut() = false;
                                }
                            }

                            BTMAN_PROPS.lock().unwrap().displaying_dialog = false;
                            BTMAN_PROPS.lock().unwrap().confirm_authorization = *pass_key.borrow();
                        });
                    }
                    Message::RequestYesNo(title, subtitle, confirm, response_type) => {
                        BTMAN_PROPS.lock().unwrap().displaying_dialog = true;

                        let popup = adw::MessageDialog::new(Some(&clone), Some(&title), None);

                        popup.set_close_response("cancel");
                        popup.set_modal(true);
                        popup.set_body_use_markup(true);
                        popup.set_body(&subtitle);
                        popup.set_destroy_with_parent(true);

                        popup.add_response("cancel", "Cancel");
                        popup.add_response(&confirm.to_lowercase(), &confirm);
                        popup.set_response_appearance(&confirm.to_lowercase(), response_type);
                        popup.set_default_response(Some(&confirm.to_lowercase()));

                        let pass_key = Rc::new(RefCell::new(false));
                        popup.clone().choose(gio::Cancellable::NONE, move |response| {
                            match response.to_string() {
                                s if s.contains(&confirm.to_lowercase()) => {
                                    *pass_key.borrow_mut() = true;
                                }
                                _ => {
                                    *pass_key.borrow_mut() = false;
                                }
                            }

                            BTMAN_PROPS.lock().unwrap().displaying_dialog = false;
                            BTMAN_PROPS.lock().unwrap().confirm_authorization = *pass_key.borrow();
                        });
                    }
                    Message::InvalidateSort() => {
                        let main_listbox = clone.imp().main_listbox.get();
                        main_listbox.invalidate_sort();
                    }
                    Message::RefreshDevicesList() => {
                        WidgetExt::activate_action(&clone, "win.refresh-devices", None).expect("cannot refresh devices list");
                    }
                    Message::UpdateBatteryLevel(level) => {
                        if let Some(battery_level_indicator) = clone.imp().battery_level_indicator.borrow().as_ref() {
                            battery_level_indicator.set_indicator_battery_level(level);
                        }
                    }
                }
            }
        });

        let main_listbox = self.imp().main_listbox.get();

        main_listbox.set_sort_func(|row_one, row_two| {
            let actionrow_one = row_one.clone().downcast::<DeviceActionRow>().unwrap();
            let actionrow_two = row_two.clone().downcast::<DeviceActionRow>().unwrap();

            let title_one = actionrow_one.title().to_lowercase();
            let title_two = actionrow_two.title().to_lowercase();

            match (title_one.as_str(), title_two.as_str()) {
                ("unknown device", _) if title_two != "unknown device" => {
                    return gtk::Ordering::Larger
                }
                (_, "unknown device") if title_one != "unknown device" => {
                    return gtk::Ordering::Smaller
                }
                ("unknown device", "unknown device") => (),
                _ => (),
            }

            match title_one.cmp(&title_two) {
                std::cmp::Ordering::Less => return gtk::Ordering::Smaller,
                std::cmp::Ordering::Greater => return gtk::Ordering::Larger,
                _ => (),
            }

            return gtk::Ordering::Equal;
        });
        main_listbox.invalidate_sort();

        // refresh devices action
        let refresh_action = gio::SimpleAction::new("refresh-devices", None);
        let sender0 = sender.clone();
        refresh_action.connect_activate(move |_, _| {
            runtime().spawn(clone!(
                #[strong]
                sender0,
                async move {
                device::stop_searching().await;
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;

                let sender = sender0.clone();
                let adapter_name = BTMAN_PROPS.lock().unwrap().current_adapter.clone();

                let mut can_send = true;
                if let Err(err) = device::get_devices_continuous(sender.clone(), adapter_name).await {
                    let string = err.message;

                    can_send = false;

                    sender
                        .send(Message::PopupError(string, adw::ToastPriority::High))
                        .await.expect("cannot send message");
                    sender
                        .send(Message::UpdateListBoxImage())
                        .await.expect("cannot send message");
                }
                println!("can send: {}", can_send);
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                if can_send {
                    sender
                        .send(Message::PopupError(
                            "br-adapter-refreshed".to_string(),
                            adw::ToastPriority::Normal,
                        ))
                        .await.expect("can't send message");
                }
            }));
        });
        self.add_action(&refresh_action);
        refresh_action.activate(None);

        // powered switch
        let powered_switch_row = self.imp().powered_switch_row.get();
        let sender5 = sender.clone();
        powered_switch_row.connect_activated(move |_| {
            let sender_clone = sender5.clone();
            let adapter_name = BTMAN_PROPS.lock().unwrap().current_adapter.clone();

            runtime().spawn(clone!(
                #[strong]
                sender_clone,
                async move {
                    if let Err(err) =
                        bluetooth_settings::set_adapter_powered(adapter_name.clone(), sender_clone.clone()).await
                    {
                        let string = err.message;
                        sender_clone
                            .send(Message::PopupError(string, adw::ToastPriority::High))
                            .await.expect("cannot send message");
                        if let Ok(session) = bluer::Session::new().await {
                            if let Ok(adapter) = session.adapter(&adapter_name) {
                                if let Ok(powered) = adapter.is_powered().await {
                                    sender_clone
                                        .send(Message::SwitchAdapterPowered(powered))
                                        .await.expect("cannot send message");
                                }
                            }
                        }
                    }
                }
            ));
        });

        // discoverable switch
        let discoverable_switch_row = self.imp().discoverable_switch_row.get();
        let sender6 = sender.clone();
        discoverable_switch_row.connect_activated(move |_| {
            let sender_clone = sender6.clone();
            let adapter_name = BTMAN_PROPS.lock().unwrap().current_adapter.clone();

            runtime().spawn(async move {
                if let Err(err) =
                    bluetooth_settings::set_adapter_discoverable(adapter_name, sender_clone.clone()).await
                {
                    let string = "Adapter ".to_string() + &err.message;
                    sender_clone
                        .send(Message::PopupError(string, adw::ToastPriority::High))
                        .await.expect("cannot send message");
                    sender_clone
                        .send(Message::SwitchAdapterDiscoverable(false))
                        .await.expect("cannot send message");
                }
            });
        });
    }

    fn save_settings(&self) -> Result<(), glib::BoolError> {
        let size = (
            self.size(gtk::Orientation::Horizontal),
            self.size(gtk::Orientation::Vertical),
        );
        let settings = self
            .imp()
            .settings
            .get()
            .expect("cannot get settings, setup improperly?");

        settings.set_int("window-width", size.0)?;
        settings.set_int("window-height", size.1)?;
        settings.set_boolean("window-maximized", self.is_maximized())?;

        Ok(())
    }

    fn preload_settings(&self) {
        let settings = self
            .imp()
            .settings
            .get()
            .expect("cannot get settings, setup improperly?");

        let width = settings.int("window-width");
        let height = settings.int("window-height");
        let maximized = settings.boolean("window-maximized");

        self.set_default_size(width, height);
        self.set_maximized(maximized);
    }

    #[tokio::main]
    async fn pre_setup(&self, sender: Sender<Message>) -> bluer::Result<()> {
        let settings = self.imp().settings.get().unwrap();

        BTMAN_PROPS.lock().unwrap().sender = Some(sender.clone());
        *devices_lut().lock().unwrap() = Some(HashMap::new());
        let name = settings.string("current-adapter-name").to_string();
        let session = bluer::Session::new().await?;

        if name.is_empty() {
            let adapter = session.default_adapter().await?;
            BTMAN_PROPS.lock().unwrap().current_adapter = adapter.name().to_string();
            BTMAN_PROPS.lock().unwrap().name = adapter.name().to_string();

            let current_adapter = adapter.name().to_string();
            settings
                .set_string("current-adapter-name", current_adapter.as_str())
                .expect("cannot set default adapter at start");
            settings
                .set_string("original-adapter-name", current_adapter.as_str())
                .expect("cannot set original adapter at start");
        } else {
            BTMAN_PROPS.lock().unwrap().current_adapter = name.clone();
        }

        let mut lut = HashMap::new();

        let adapter = session.adapter(BTMAN_PROPS.lock().unwrap().current_adapter.clone().as_str())?;
        let alias = adapter.alias().await?;
        println!("startup alias is: {}\n", alias);

        lut.insert(alias.to_string(), BTMAN_PROPS.lock().unwrap().current_adapter.to_string());
        *adapters_lut().lock().unwrap() = Some(lut);

        let clone = sender.clone();
        std::thread::spawn(move || {
            register_bluetooth_agent(sender.clone()).expect("cannot register bluetooth agent");
        });

        let _ = clone;

        Ok(())
    }
}

/// Creates a new DeviceActionRow from a device
#[tokio::main]
async fn add_child_row(device: bluer::Device) -> bluer::Result<DeviceActionRow> {
    let child_row = DeviceActionRow::new();

    let mut name = device.alias().await?;
    let address = device.address();

    child_row.set_bluer_address(address);

    if let Ok(_bad_title) = bluer::Address::from_str(name.clone().replace('-', ":").as_str()) {
        name = "Unknown Device".to_string();
        child_row.set_title("Unknown Device");
    } else {
        child_row.set_title(name.clone().as_str());
    }

    let props = BTMAN_PROPS.lock().unwrap();
    child_row.set_activatable(true);

    {
        let mut lut = devices_lut().lock().unwrap().take().unwrap();
        lut.insert(address, name.clone());
        *devices_lut().lock().unwrap() = Some(lut);
    }

    let sender = props.sender.clone().unwrap();
    sender
        .send(Message::InvalidateSort())
        .await.expect("cannot send message");

    // on click — pair/connect/disconnect
    child_row.connect_activated(move |row| {
        BTMAN_PROPS.lock().unwrap().current_index = row.index();
        BTMAN_PROPS.lock().unwrap().address = row.get_bluer_address();

        let address = row.get_bluer_address();
        let adapter_name = BTMAN_PROPS.lock().unwrap().current_adapter.clone();
        let sender_clone = sender.clone();

        runtime().spawn(async move {
            if let Err(err) =
                device::set_device_active(address, sender_clone.clone(), adapter_name).await
            {
                let string = err.message;

                sender_clone
                    .send(Message::PopupError(string, adw::ToastPriority::High))
                    .await.expect("cannot send message");
            }
        });
    });

    Ok(child_row)
}
