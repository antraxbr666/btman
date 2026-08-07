#[allow(dead_code)]
pub enum Message {
    /// Changes the connected switch's active to `bool` if the `is_current` is true, and sets the corresponding device action row's connected state
    SwitchActive(bool, bluer::Address, bool),
    /// Changes the connected switch's spinner spinning state to `bool` for the device at `bluer::Address`
    SwitchActiveSpinner(bool, bluer::Address),
    /// Changes the supplied device's RSSI to the supplied value
    SwitchRssi(bluer::Address, i32),
    /// Removes the device matching the supplied name
    RemoveDevice(String, bluer::Address),
    /// Adds a new device from the properties of [device](bluer::Device)
    AddRow(bluer::Device),
    /// Adds a new paired device with already-resolved name, address and connected state
    AddPairedRow(String, bluer::Address, bool),
    /// Changes the adapter's powered state to `bool`
    SwitchAdapterPowered(bool),
    /// Changes the adapter's discoverable state to `bool`
    SwitchAdapterDiscoverable(bool),
    /// Displays an error (or a message) of [message](String) with a [priority](adw::ToastPriority) as a [toast](adw::Toast)
    PopupError(String, adw::ToastPriority),
    /// Checks if there are devices and changes the "no bluetooth devices found" image accordingly
    UpdateListBoxImage(),
    /// Requests a pairing pincode using [request](bluer::agent::RequestPinCode) as input
    RequestPinCode(bluer::agent::RequestPinCode),
    /// Displays a pairing pincode using [request](bluer::agent::DisplayPinCode) as input
    DisplayPinCode(bluer::agent::DisplayPinCode),
    /// Requests a pairing passkey using [request](bluer::agent::RequestPasskey) as input
    RequestPassKey(bluer::agent::RequestPasskey),
    /// Displays a pairing passkey using [request](bluer::agent::RequestPasskey) as input
    DisplayPassKey(bluer::agent::DisplayPasskey),
    /// Requests pairing confirmation using [request](bluer::agent::RequestConfirmation) as input
    RequestConfirmation(bluer::agent::RequestConfirmation),
    /// Requests pairing authorization using [request](bluer::agent::RequestAuthorization) as input
    RequestAuthorization(bluer::agent::RequestAuthorization),
    /// Requests service authorization using [request](bluer::agent::AuthorizeService) as input
    AuthorizeService(bluer::agent::AuthorizeService),
    /// Gets a `yes/no` answer from a dialog
    #[allow(dead_code)]
    RequestYesNo(String, String, String, adw::ResponseAppearance),
    /// Invalidates the device list's sorting, forcing it to resort the devices according to various factors
    InvalidateSort(),
    /// Forcefully refreshes the device list
    RefreshDevicesList(),
    /// Updates the battery level of the currently selected device
    UpdateBatteryLevel(i8),
}
