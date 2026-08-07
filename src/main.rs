/* main.rs
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

mod application;
mod config;
mod window;
mod bluetooth_state;
#[path = "bluetooth/message.rs"] mod message;
#[path = "bluetooth/bluetooth_settings.rs"] mod bluetooth_settings;
#[path = "bluetooth/device.rs"] mod device;
#[path = "bluetooth/agent.rs"] mod agent;
#[path = "bluetooth/battery.rs"] mod battery;
#[path = "widgets/device_action_row.rs"] mod device_action_row;
#[path = "widgets/paired_device_row.rs"] mod paired_device_row;
#[path = "widgets/startup_error_message.rs"] mod startup_error_message;
#[path = "widgets/battery_indicator.rs"] mod battery_indicator;
mod singletons;

use self::application::BtmanApplication;
use self::window::BtmanWindow;

use gtk::{gio, glib};
use gtk::prelude::*;
use gtk::gdk::Display;

fn main() -> glib::ExitCode {
    // Set GSETTINGS_SCHEMA_DIR for development builds
    if let Ok(mut path) = std::env::current_exe() {
        path.pop();
        if let Some(path_str) = path.to_str() {
            std::env::set_var("GSETTINGS_SCHEMA_DIR", path_str);
        }
    }

    let app = BtmanApplication::new("io.github.antraxbr666.Btman", &gio::ApplicationFlags::empty());

    app.connect_startup(|_| {
        load_css()
    });

    app.run()
}

fn load_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_data(include_str!("gtk/style.css"));

    gtk::style_context_add_provider_for_display(
        &Display::default().expect("could not connect to a display"),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
