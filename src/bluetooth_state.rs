use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use bluer::Address;

/// Device address → name mapping - shared between window.rs and bluetooth/device.rs
pub fn devices_lut() -> &'static Mutex<Option<HashMap<Address, String>>> {
    static INSTANCE: OnceLock<Mutex<Option<HashMap<Address, String>>>> = OnceLock::new();
    INSTANCE.get_or_init(|| Mutex::new(None))
}

/// Adapter alias → name mapping - shared between window.rs and bluetooth/bluetooth_settings.rs
pub fn adapters_lut() -> &'static Mutex<Option<HashMap<String, String>>> {
    static INSTANCE: OnceLock<Mutex<Option<HashMap<String, String>>>> = OnceLock::new();
    INSTANCE.get_or_init(|| Mutex::new(None))
}
