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
use gtk::{gio, glib};

use crate::agent::register_bluetooth_agent;
use crate::application::BtmanApplication;
use crate::device_action_row::DeviceActionRow;
use crate::paired_device_row::PairedDeviceRow;
use crate::message::Message;
use crate::startup_error_message::StartupErrorMessage;
use crate::{bluetooth_settings, device};
use crate::singletons::BtmanProperties;

use async_channel::Sender;
use std::cell::{OnceCell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::{Arc, Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::runtime::Runtime;
use lazy_static::lazy_static;

use crate::bluetooth_state::{devices_lut, adapters_lut};

lazy_static! {
    pub static ref BTMAN_PROPS: Mutex<BtmanProperties> = Mutex::new(BtmanProperties::new());
}

mod imp {
    use super::*;

    #[derive(Debug, Default)]
    pub struct BtmanWindow {
        pub main_listbox: RefCell<Option<gtk::ListBox>>,
        pub paired_listbox: RefCell<Option<gtk::ListBox>>,
        pub powered_switch_row: RefCell<Option<adw::SwitchRow>>,
        pub discoverable_switch_row: RefCell<Option<adw::SwitchRow>>,
        pub toast_overlay: RefCell<Option<adw::ToastOverlay>>,
        pub listbox_image_box: RefCell<Option<gtk::Box>>,
        pub paired_image_box: RefCell<Option<gtk::Box>>,
        pub window_title: RefCell<Option<adw::WindowTitle>>,
        pub bluetooth_group: RefCell<Option<adw::PreferencesGroup>>,

        pub settings: OnceCell<Settings>,
        pub display_pass_key_dialog: RefCell<Option<adw::MessageDialog>>,
        pub powered_sync_guard: Arc<AtomicBool>,
        pub discoverable_sync_guard: Arc<AtomicBool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for BtmanWindow {
        const NAME: &'static str = "BtmanWindow";
        type Type = super::BtmanWindow;
        type ParentType = adw::ApplicationWindow;
    }

    impl ObjectImpl for BtmanWindow {
        fn constructed(&self) {
            self.parent_constructed();

            // Build UI
            let toast_overlay = adw::ToastOverlay::new();
            let toolbar_view = adw::ToolbarView::new();

            let header_bar = adw::HeaderBar::new();
            let window_title = adw::WindowTitle::new("btman", "Bluetooth Manager");
            window_title.set_halign(gtk::Align::Center);
            header_bar.set_title_widget(Some(&window_title));

            let menu_button = gtk::MenuButton::builder()
                .icon_name("open-menu-symbolic")
                .tooltip_text("Main Menu")
                .build();
            // menu_model will be set from the application
            header_bar.pack_end(&menu_button);
            toolbar_view.add_top_bar(&header_bar);

            let main_box = gtk::Box::new(gtk::Orientation::Vertical, 12);
            main_box.set_halign(gtk::Align::Fill);
            main_box.set_valign(gtk::Align::Fill);
            main_box.set_margin_start(12);
            main_box.set_margin_end(12);
            main_box.set_margin_top(6);
            main_box.set_margin_bottom(12);

            // Bluetooth group
            let bluetooth_group = adw::PreferencesGroup::builder()
                .title("Bluetooth")
                .description("Adapter status")
                .build();

            let powered_switch_row = adw::SwitchRow::builder()
                .title("Enabled")
                .build();
            bluetooth_group.add(&powered_switch_row);

            let discoverable_switch_row = adw::SwitchRow::builder()
                .title("Discoverable")
                .subtitle("Visible to other devices")
                .build();
            bluetooth_group.add(&discoverable_switch_row);

            main_box.append(&bluetooth_group);

            // Devices group
            let devices_group = adw::PreferencesGroup::builder()
                .title("Devices")
                .description("All devices in range")
                .build();

            // Empty state image
            let listbox_image_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
            listbox_image_box.set_visible(true);
            listbox_image_box.set_vexpand(true);
            listbox_image_box.set_valign(gtk::Align::Center);
            listbox_image_box.set_halign(gtk::Align::Center);

            let empty_box = gtk::Box::new(gtk::Orientation::Vertical, 12);
            empty_box.set_valign(gtk::Align::Center);
            empty_box.set_halign(gtk::Align::Center);

            let empty_icon = gtk::Image::from_icon_name("bluetooth-disabled-symbolic");
            empty_icon.set_pixel_size(48);
            empty_icon.set_opacity(0.35);
            empty_box.append(&empty_icon);

            let empty_label = gtk::Label::new(Some("No devices in range"));
            empty_label.set_opacity(0.4);
            empty_label.set_wrap(true);
            empty_label.set_justify(gtk::Justification::Center);
            empty_box.append(&empty_label);

            listbox_image_box.append(&empty_box);
            devices_group.add(&listbox_image_box);

            // Device list
            let scrolled = gtk::ScrolledWindow::builder()
                .propagate_natural_height(true)
                .kinetic_scrolling(true)
                .overlay_scrolling(true)
                .build();
            scrolled.add_css_class("flat");

            let main_listbox = gtk::ListBox::builder()
                .margin_top(6)
                .margin_bottom(6)
                .margin_start(6)
                .margin_end(6)
                .valign(gtk::Align::Fill)
                .visible(false)
                .build();
            main_listbox.add_css_class("boxed-list");
            main_listbox.add_css_class("separators");

            scrolled.set_child(Some(&main_listbox));
            devices_group.add(&scrolled);

            // Paired Devices group
            let paired_group = adw::PreferencesGroup::builder()
                .title("Paired Devices")
                .description("Devices paired to this computer")
                .build();

            // Empty state image
            let paired_image_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
            paired_image_box.set_visible(true);
            paired_image_box.set_vexpand(true);
            paired_image_box.set_valign(gtk::Align::Center);
            paired_image_box.set_halign(gtk::Align::Center);

            let paired_empty_box = gtk::Box::new(gtk::Orientation::Vertical, 12);
            paired_empty_box.set_valign(gtk::Align::Center);
            paired_empty_box.set_halign(gtk::Align::Center);

            let paired_empty_icon = gtk::Image::from_icon_name("network-wireless-symbolic");
            paired_empty_icon.set_pixel_size(48);
            paired_empty_icon.set_opacity(0.35);
            paired_empty_box.append(&paired_empty_icon);

            let paired_empty_label = gtk::Label::new(Some("No paired devices"));
            paired_empty_label.set_opacity(0.4);
            paired_empty_label.set_wrap(true);
            paired_empty_label.set_justify(gtk::Justification::Center);
            paired_empty_box.append(&paired_empty_label);

            paired_image_box.append(&paired_empty_box);
            paired_group.add(&paired_image_box);

            // Paired device list
            let paired_scrolled = gtk::ScrolledWindow::builder()
                .propagate_natural_height(true)
                .kinetic_scrolling(true)
                .overlay_scrolling(true)
                .build();
            paired_scrolled.add_css_class("flat");

            let paired_listbox = gtk::ListBox::builder()
                .margin_top(6)
                .margin_bottom(6)
                .margin_start(6)
                .margin_end(6)
                .valign(gtk::Align::Fill)
                .visible(false)
                .build();
            paired_listbox.add_css_class("boxed-list");
            paired_listbox.add_css_class("separators");

            paired_scrolled.set_child(Some(&paired_listbox));
            paired_group.add(&paired_scrolled);

            main_box.append(&paired_group);

            // Devices group
            main_box.append(&devices_group);

            toolbar_view.set_content(Some(&main_box));
            toast_overlay.set_child(Some(&toolbar_view));

            self.obj().set_content(Some(&toast_overlay));

            // Store references
            *self.main_listbox.borrow_mut() = Some(main_listbox);
            *self.paired_listbox.borrow_mut() = Some(paired_listbox);
            *self.powered_switch_row.borrow_mut() = Some(powered_switch_row);
            *self.discoverable_switch_row.borrow_mut() = Some(discoverable_switch_row);
            *self.toast_overlay.borrow_mut() = Some(toast_overlay);
            *self.listbox_image_box.borrow_mut() = Some(listbox_image_box);
            *self.paired_image_box.borrow_mut() = Some(paired_image_box);
            *self.window_title.borrow_mut() = Some(window_title);
            *self.bluetooth_group.borrow_mut() = Some(bluetooth_group);

            // Setup settings
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

glib::wrapper! {
    pub struct BtmanWindow(ObjectSubclass<imp::BtmanWindow>)
        @extends gtk::Widget, adw::ApplicationWindow, BtmanApplication,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager, gtk::ApplicationWindow, gtk::Grid, gtk::Window;
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
                gtk::prelude::WidgetExt::activate_action(&clone, "app.quit", None)
                    .expect("cannot exit app on message close");
            });

            message.set_visible(true);
            return;
        }

        let self_clone = self.clone();

        glib::MainContext::default().spawn_local(async move {
            while let Ok(msg) = receiver.recv().await {
                let clone = self_clone.clone();

                match msg {
                    Message::SwitchActive(_active, address, is_current) => {
                        let _ = is_current;

                        let paired_listbox = clone.imp().paired_listbox.borrow();
                        let paired_listbox = paired_listbox.as_ref().unwrap();
                        let mut paired_index = 0;
                        while let Some(row) = paired_listbox.row_at_index(paired_index) {
                            let action_row = row
                                .downcast::<PairedDeviceRow>()
                                .expect("cannot downcast to paired row.");
                            if action_row.get_bluer_address() == address {
                                action_row.set_connected(_active);
                            }
                            paired_index += 1;
                        }
                    }
                    Message::SwitchActiveSpinner(spinning, address) => {
                        let main_listbox = clone.imp().main_listbox.borrow();
                        let main_listbox = main_listbox.as_ref().unwrap();
                        let mut main_index = 0;
                        while let Some(row) = main_listbox.row_at_index(main_index) {
                            let action_row = row
                                .downcast::<DeviceActionRow>()
                                .expect("cannot downcast to action row.");
                            if action_row.get_bluer_address() == address {
                                action_row.set_spinning(spinning);
                                break;
                            }
                            main_index += 1;
                        }

                        let paired_listbox = clone.imp().paired_listbox.borrow();
                        let paired_listbox = paired_listbox.as_ref().unwrap();
                        let mut spinning_index = 0;
                        while let Some(row) = paired_listbox.row_at_index(spinning_index) {
                            let action_row = row
                                .downcast::<PairedDeviceRow>()
                                .expect("cannot downcast to paired row.");
                            if action_row.get_bluer_address() == address {
                                action_row.set_spinning(spinning);
                                break;
                            }
                            spinning_index += 1;
                        }
                    }
                    Message::SwitchRssi(address, rssi) => {
                        let main_listbox = clone.imp().main_listbox.borrow();
                        let main_listbox = main_listbox.as_ref().unwrap();
                        let mut main_index = 0;
                        while let Some(row) = main_listbox.row_at_index(main_index) {
                            let action_row = row
                                .downcast::<DeviceActionRow>()
                                .expect("cannot downcast to action row.");
                            if action_row.get_bluer_address() == address {
                                action_row.set_rssi(rssi);
                            }
                            main_index += 1;
                        }

                        let paired_listbox = clone.imp().paired_listbox.borrow();
                        let paired_listbox = paired_listbox.as_ref().unwrap();
                        let mut paired_index = 0;
                        while let Some(row) = paired_listbox.row_at_index(paired_index) {
                            let action_row = row
                                .downcast::<PairedDeviceRow>()
                                .expect("cannot downcast to paired row.");
                            if action_row.get_bluer_address() == address {
                                action_row.set_rssi(rssi);
                            }
                            paired_index += 1;
                        }
                    }
                    Message::AddRow(device) => {
                        let row = add_child_row(device);

                        if let Ok(ok_row) = row {
                            let main_listbox = clone.imp().main_listbox.borrow();
                            let main_listbox = main_listbox.as_ref().unwrap();
                            main_listbox.append(&ok_row);
                            main_listbox.invalidate_sort();
                        }
                    }
                    Message::AddPairedRow(name, address, connected) => {
                        let paired_listbox = clone.imp().paired_listbox.borrow();
                        let paired_listbox = paired_listbox.as_ref().unwrap();

                        let mut exists = false;
                        let mut index = 0;
                        while let Some(row) = paired_listbox.row_at_index(index) {
                            let action_row = row
                                .downcast::<PairedDeviceRow>()
                                .expect("cannot downcast to paired row.");
                            if action_row.get_bluer_address() == address {
                                exists = true;
                                break;
                            }
                            index += 1;
                        }

                        if !exists {
                            let ok_row = add_paired_row(&name, address, connected);
                            paired_listbox.append(&ok_row);

                            let paired_image_box = clone.imp().paired_image_box.borrow();
                            let paired_image_box = paired_image_box.as_ref().unwrap();
                            paired_image_box.set_visible(false);
                            paired_listbox.set_visible(true);
                        }
                    }
                    Message::RemoveDevice(name, address) => {
                        let listbox = clone.imp().main_listbox.borrow();
                        let listbox = listbox.as_ref().unwrap();
                        let mut index = 0;

                        while let Some(row) = listbox.row_at_index(index) {
                            let action_row = row.downcast::<DeviceActionRow>().expect("cannot downcast to action row.");

                            if action_row.title() == name && action_row.get_bluer_address() == address {
                                listbox.remove(&action_row);
                            }
                            index += 1;
                        }

                        listbox.invalidate_sort();

                        let paired_listbox = clone.imp().paired_listbox.borrow();
                        let paired_listbox = paired_listbox.as_ref().unwrap();
                        let mut paired_index = 0;
                        while let Some(row) = paired_listbox.row_at_index(paired_index) {
                            let paired_row = row.downcast::<PairedDeviceRow>().expect("cannot downcast to paired row.");
                            if paired_row.get_bluer_address() == address {
                                paired_listbox.remove(&paired_row);
                                break;
                            }
                            paired_index += 1;
                        }

                        if paired_listbox.row_at_index(0).is_none() {
                            let paired_image_box = clone.imp().paired_image_box.borrow();
                            let paired_image_box = paired_image_box.as_ref().unwrap();
                            paired_image_box.set_visible(true);
                            paired_listbox.set_visible(false);
                        }
                    }
                    Message::SwitchAdapterPowered(powered) => {
                        let imp = clone.imp();
                        imp.powered_sync_guard.store(true, Ordering::SeqCst);
                        let powered_switch_row = imp.powered_switch_row.borrow();
                        let powered_switch_row = powered_switch_row.as_ref().unwrap();
                        powered_switch_row.set_active(powered);
                        imp.powered_sync_guard.store(false, Ordering::SeqCst);

                        if !powered {
                            clear_device_lists(&clone);

                            *devices_lut().lock().unwrap() = Some(HashMap::new());
                        } else {
                            let sender =
                                BTMAN_PROPS.lock().unwrap().sender.clone().unwrap();
                            let adapter_name = BTMAN_PROPS
                                .lock()
                                .unwrap()
                                .current_adapter
                                .clone();
                            runtime().spawn(async move {
                                let _ = device::get_paired_devices(sender, adapter_name).await;
                            });
                        }
                    }
                    Message::SwitchAdapterDiscoverable(discoverable) => {
                        let imp = clone.imp();
                        imp.discoverable_sync_guard.store(true, Ordering::SeqCst);
                        let discoverable_switch_row = imp.discoverable_switch_row.borrow();
                        let discoverable_switch_row = discoverable_switch_row.as_ref().unwrap();
                        discoverable_switch_row.set_active(discoverable);
                        imp.discoverable_sync_guard.store(false, Ordering::SeqCst);
                    }
                    Message::PopupError(string, priority) => {
                        let toast_overlay = clone.imp().toast_overlay.borrow();
                        let toast_overlay = toast_overlay.as_ref().unwrap();
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
                        let listbox_image_box = clone.imp().listbox_image_box.borrow();
                        let listbox_image_box = listbox_image_box.as_ref().unwrap();
                        let main_listbox = clone.imp().main_listbox.borrow();
                        let main_listbox = main_listbox.as_ref().unwrap();

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
                        let main_listbox = clone.imp().main_listbox.borrow();
                        let main_listbox = main_listbox.as_ref().unwrap();
                        main_listbox.invalidate_sort();
                    }
                    Message::RefreshDevicesList() => {
                        gtk::prelude::WidgetExt::activate_action(&clone, "win.refresh-devices", None).expect("cannot refresh devices list");
                    }
                    Message::UpdateBattery(address, level) => {
                        let paired_listbox = clone.imp().paired_listbox.borrow();
                        let paired_listbox = paired_listbox.as_ref().unwrap();
                        let mut battery_index = 0;
                        while let Some(row) = paired_listbox.row_at_index(battery_index) {
                            let action_row = row
                                .downcast::<PairedDeviceRow>()
                                .expect("cannot downcast to paired row.");
                            if action_row.get_bluer_address() == address {
                                action_row.set_battery(level);
                                break;
                            }
                            battery_index += 1;
                        }
                    }
                }
            }
        });

        let main_listbox = self.imp().main_listbox.borrow();
        let main_listbox = main_listbox.as_ref().unwrap();

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
        let powered_switch_row = self.imp().powered_switch_row.borrow();
        let powered_switch_row = powered_switch_row.as_ref().unwrap();
        let sender5 = sender.clone();
        let powered_guard = self.imp().powered_sync_guard.clone();
        powered_switch_row.connect_notify(Some("active"), move |_, _| {
            if powered_guard.load(Ordering::SeqCst) {
                return;
            }
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
        let discoverable_switch_row = self.imp().discoverable_switch_row.borrow();
        let discoverable_switch_row = discoverable_switch_row.as_ref().unwrap();
        let sender6 = sender.clone();
        let discoverable_guard = self.imp().discoverable_sync_guard.clone();
        discoverable_switch_row.connect_notify(Some("active"), move |_, _| {
            if discoverable_guard.load(Ordering::SeqCst) {
                return;
            }
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

        if let Ok(powered) = adapter.is_powered().await {
            sender.send(Message::SwitchAdapterPowered(powered)).await.expect("cannot send message");
        }
        if let Ok(discoverable) = adapter.is_discoverable().await {
            sender.send(Message::SwitchAdapterDiscoverable(discoverable)).await.expect("cannot send message");
        }

        lut.insert(alias.to_string(), BTMAN_PROPS.lock().unwrap().current_adapter.to_string());
        *adapters_lut().lock().unwrap() = Some(lut);

        let clone = sender.clone();
        std::thread::spawn(move || {
            register_bluetooth_agent(clone.clone()).expect("cannot register bluetooth agent");
        });

        let adapter_name = BTMAN_PROPS.lock().unwrap().current_adapter.clone();
        runtime().spawn(async move {
            let _ = device::get_paired_devices(sender.clone(), adapter_name).await;
        });

        Ok(())
    }
}

/// Creates a new DeviceActionRow from a device
#[tokio::main]
async fn add_child_row(device: bluer::Device) -> bluer::Result<DeviceActionRow> {
    let device_sender = BTMAN_PROPS.lock().unwrap().sender.clone().unwrap();
    let child_row = DeviceActionRow::new(device_sender.clone());

    let mut name = device.alias().await?;
    let address = device.address();

    child_row.set_bluer_address(address);

    if let Ok(_bad_title) = bluer::Address::from_str(name.clone().replace('-', ":").as_str()) {
        name = "Unknown Device".to_string();
        child_row.set_title("Unknown Device");
    } else {
        child_row.set_title(name.clone().as_str());
    }

    {
        let mut lut = devices_lut().lock().unwrap().take().unwrap();
        lut.insert(address, name.clone());
        *devices_lut().lock().unwrap() = Some(lut);
    }

    device_sender
        .send(Message::InvalidateSort())
        .await.expect("cannot send message");

    Ok(child_row)
}

/// Creates a new PairedDeviceRow from an already-resolved name, address and connected state
fn add_paired_row(name: &str, address: bluer::Address, connected: bool) -> PairedDeviceRow {
    let mut name = name.to_string();

    if let Ok(_bad_title) = bluer::Address::from_str(name.clone().replace('-', ":").as_str()) {
        name = "Unknown Device".to_string();
    }

    let sender = BTMAN_PROPS.lock().unwrap().sender.clone().unwrap();
    PairedDeviceRow::new(&name, address, sender, connected)
}

/// Removes every device from the in-range and paired listboxes and restores
/// their empty states.
fn clear_device_lists(win: &BtmanWindow) {
    let imp = win.imp();

    let main_listbox = imp.main_listbox.borrow();
    let main_listbox = main_listbox.as_ref().unwrap();
    while let Some(row) = main_listbox.row_at_index(0) {
        main_listbox.remove(&row);
    }

    let paired_listbox = imp.paired_listbox.borrow();
    let paired_listbox = paired_listbox.as_ref().unwrap();
    while let Some(row) = paired_listbox.row_at_index(0) {
        paired_listbox.remove(&row);
    }

    let listbox_image_box = imp.listbox_image_box.borrow();
    let listbox_image_box = listbox_image_box.as_ref().unwrap();
    listbox_image_box.set_visible(true);
    main_listbox.set_visible(false);

    let paired_image_box = imp.paired_image_box.borrow();
    let paired_image_box = paired_image_box.as_ref().unwrap();
    paired_image_box.set_visible(true);
    paired_listbox.set_visible(false);
}
