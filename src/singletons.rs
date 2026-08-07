use async_channel::Sender;
use bluer::Address;
use crate::message::Message;

pub struct BtmanProperties {
    pub name: String,
    pub current_adapter: String,
    pub sender: Option<Sender<Message>>,
    pub address: Address,
    pub displaying_dialog: bool,
    pub pin_code: String,
    pub pass_key: u32,
    pub confirm_authorization: bool,
}

impl BtmanProperties {
    pub(crate) fn new() -> Self {
        let empty_string = "".to_string();
        BtmanProperties {
            name: empty_string.to_string(),
            current_adapter: empty_string.to_string(),
            sender: None,
            address: Address::any(),
            displaying_dialog: false,
            pin_code: empty_string.to_string(),
            pass_key: 0,
            confirm_authorization: false,
        }
    }
}
