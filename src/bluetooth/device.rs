use async_channel::Sender;
use bluer::{AdapterEvent, AdapterProperty, DeviceEvent, DeviceProperty};
use futures::{pin_mut, stream::SelectAll, StreamExt};
use std::sync::{Mutex, OnceLock};
use tokio_util::sync::CancellationToken;

use crate::{message::Message, bluetooth_state::devices_lut, agent::wait_for_dialog_exit, battery::cancel_battery_check};
use crate::window::BTMAN_PROPS;

fn cancellation_token() -> &'static Mutex<Option<CancellationToken>> {
    static INSTANCE: OnceLock<Mutex<Option<CancellationToken>>> = OnceLock::new();
    INSTANCE.get_or_init(|| Mutex::new(None))
}

pub async fn set_device_active(address: bluer::Address, sender: Sender<Message>, adapter_name: String) -> bluer::Result<()> {
    let address_string = address.clone().to_string();
    let adapter_string = adapter_name.clone();

    let adapter = bluer::Session::new().await?.adapter(adapter_name.as_str())?;
    let device = adapter.device(address)?;

    sender.send(Message::SwitchActiveSpinner(true)).await.expect("cannot set spinner to show.");

    let state = device.is_connected().await?;

    if state {
        device.disconnect().await?;
    }
    else if !device.is_paired().await? {
        device.pair().await?;
        device.connect().await?;
        device.connect().await?;
    }
    else {
        device.connect().await?;
    }

    let updated_state = device.is_connected().await?;

    println!("set state {} for device {}\n", updated_state, device.address());
    sender.send(Message::SwitchActiveSpinner(false)).await.expect("cannot set spinner to show.");
    sender.send(Message::SwitchActive(updated_state, address, true)).await.expect("cannot send message");
    sender.send(Message::InvalidateSort()).await.expect("cannot set device name.");

    let sender_clone = sender.clone();
    std::thread::spawn(move || {
        let clone = sender_clone.clone();
        *cancel_battery_check().lock().unwrap() = true;
        crate::battery::get_battery_for_device(address_string, adapter_string, clone);
    });

    Ok(())
}

#[allow(dead_code)]
pub async fn remove_device(address: bluer::Address, sender: Sender<Message>, adapter_name: String) -> bluer::Result<()> {
    let adapter = bluer::Session::new().await?.adapter(adapter_name.as_str())?;
    let device = adapter.device(address)?;

    let title = "Remove Device?".to_string();
    let subtitle = "Are you sure you want to remove <span font_weight='bold' color='#78aeed'>`".to_string() + &device.alias().await? + "`</span>?";
    let confirm = "Remove".to_string();

    BTMAN_PROPS.lock().unwrap().displaying_dialog = true;

    sender.send(Message::RequestYesNo(title, subtitle, confirm, adw::ResponseAppearance::Destructive)).await.expect("can't send message");

    wait_for_dialog_exit().await;

    let confirmed = BTMAN_PROPS.lock().unwrap().confirm_authorization;

    if confirmed {
        println!("removing device...");
        let name = device.alias().await?;
        adapter.remove_device(address).await?;
        {
            let mut lut = devices_lut().lock().unwrap().take().unwrap();
            if lut.contains_key(&address) {
                lut.remove(&address);
                *devices_lut().lock().unwrap() = Some(lut);
            }
        }

        sender.send(Message::RemoveDevice(name, address)).await.expect("can't send message");
        sender.send(Message::UpdateListBoxImage()).await.expect("can't send message");
    }

    Ok(())
}

pub async fn stop_searching() {
    if let Some(token) = cancellation_token().lock().unwrap().take() {
        token.cancel();
    }
}

/// Enumerates all paired devices known to the adapter and sends `AddPairedRow`
/// for each one. Used to populate the "Paired Devices" list on startup and
/// whenever the adapter is powered back on.
pub async fn get_paired_devices(sender: Sender<Message>, adapter_name: String) -> bluer::Result<()> {
    let session = bluer::Session::new().await?;
    let adapter = session.adapter(adapter_name.as_str())?;

    let addresses = adapter.device_addresses().await?;

    for addr in addresses {
        if let Ok(device) = adapter.device(addr) {
            if let Ok(true) = device.is_paired().await {
                sender.send(Message::AddPairedRow(device)).await.expect("cannot send message");
            }
        }
    }

    Ok(())
}

pub async fn get_devices_continuous(sender: Sender<Message>, adapter_name: String) -> bluer::Result<()> {
    let session = bluer::Session::new().await?;
    let adapter = &session.adapter(adapter_name.as_str())?;

    let filter = bluer::DiscoveryFilter {
        transport: bluer::DiscoveryTransport::Auto,
        ..Default::default()
    };
    adapter.set_discovery_filter(filter).await?;

    let device_events = adapter.discover_devices().await?;
    pin_mut!(device_events);

    let mut all_change_events = SelectAll::new();

    let sender_clone = sender.clone();

    let token = CancellationToken::new();
    *cancellation_token().lock().unwrap() = Some(token.clone());

    while adapter.is_powered().await? {
        tokio::select! {
            Some(device_event) = device_events.next() => {
                match device_event {
                    AdapterEvent::DeviceAdded(addr) => {
                        if adapter.is_powered().await? {
                            let supposed_device = adapter.device(addr);

                            let devices_lut = devices_lut().lock().unwrap().as_ref().unwrap().clone();

                            if !devices_lut.contains_key(&addr) {
                                if let Ok(added_device) = supposed_device {
                                    if let Ok(paired) = added_device.is_paired().await {
                                        if paired {
                                            sender.send(Message::AddPairedRow(added_device)).await.expect("cannot send message {}");
                                        } else {
                                            sender.send(Message::AddRow(added_device)).await.expect("cannot send message {}");
                                        }
                                    } else {
                                        sender.send(Message::AddRow(added_device)).await.expect("cannot send message {}");
                                    }
                                    sender.send(Message::UpdateListBoxImage()).await.expect("cannot send message {}");

                                    let device = adapter.device(addr)?;
                                    let change_events = device.events().await?.map(move |evt| (addr, evt));
                                    all_change_events.push(change_events);
                                }
                                else {
                                    println!("device isn't present, something went wrong.");
                                }
                            }
                            else {
                                println!("device already exists, not adding again.");
                            }
                        }
                    }
                    AdapterEvent::DeviceRemoved(addr) => {
                        if adapter.is_powered().await? {
                            let mut lut = devices_lut().lock().unwrap().take().unwrap();

                            let device_name = if lut.contains_key(&addr) {
                                let name = lut.get(&addr).unwrap().clone();
                                lut.remove(&addr);
                                *devices_lut().lock().unwrap() = Some(lut);

                                name
                            }
                            else {
                                String::new()
                            };

                            sender_clone.send(Message::RemoveDevice(device_name.clone(), addr)).await.expect("cannot send message");
                            sender_clone.send(Message::UpdateListBoxImage()).await.expect("cannot send message");
                            println!("Device removed: {:?} {}\n", addr, device_name.clone());
                        }
                    },
                    AdapterEvent::PropertyChanged(AdapterProperty::Powered(powered)) => {
                        tokio::time::sleep(std::time::Duration::from_secs_f32(0.5)).await;
                        sender_clone.send(Message::SwitchAdapterPowered(powered)).await.expect("cannot send message {}");
                        println!("powered switch to {}", powered);
                    },
                    AdapterEvent::PropertyChanged(AdapterProperty::Discoverable(discoverable)) => {
                        tokio::time::sleep(std::time::Duration::from_secs_f32(0.5)).await;
                        sender_clone.send(Message::SwitchAdapterDiscoverable(discoverable)).await.expect("cannot send message {}");
                        println!("discoverable switch to {}", discoverable);
                    },
                    event => {
                        println!("unhandled adapter event: {:?}", event);
                    }
                }
            }
            Some((addr, DeviceEvent::PropertyChanged(property))) = all_change_events.next() => {
                let current_address = BTMAN_PROPS.lock().unwrap().address;
                match property {
                    DeviceProperty::Connected(connected) => {
                        tokio::time::sleep(std::time::Duration::from_secs_f32(0.5)).await;
                        sender_clone.send(Message::SwitchActive(connected, addr, addr == current_address)).await.expect("cannot send message");
                    },
                    DeviceProperty::Alias(_name) => {
                        sender_clone.send(Message::SwitchRssi(addr, 0)).await.ok();
                    },
                    DeviceProperty::Rssi(rssi) => {
                        sender_clone.send(Message::SwitchRssi(addr, rssi as i32)).await.expect("cannot send message");
                        sender_clone.send(Message::InvalidateSort()).await.expect("cannot send message");
                    },
                    event => {
                        println!("unhandled device event: {:?}", event);
                    },
                }
            }
            _ = async {
                let token = cancellation_token().lock().unwrap().as_ref().cloned();
                if let Some(token) = token {
                    token.cancelled().await;
                }
            } => {
                break;
            }
            else => break
        }
    }

    println!("exited loop");
    if cancellation_token().lock().unwrap().as_ref().is_some_and(|t| t.is_cancelled()) {
        Ok(())
    }
    else {
        Err(bluer::Error { kind: bluer::ErrorKind::Failed, message: "Stopped searching for devices".to_string() })
    }
}
