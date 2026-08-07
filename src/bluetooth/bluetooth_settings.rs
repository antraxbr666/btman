use async_channel::Sender;
use crate::message::Message;

pub async fn set_adapter_powered(adapter_name: String, sender: Sender<Message>) -> bluer::Result<()> {
    let adapter = bluer::Session::new().await?.adapter(adapter_name.as_str())?;

    let current = adapter.is_powered().await?;
    adapter.set_powered(!current).await?;

    let powered = adapter.is_powered().await?;

    if powered {
        sender.send(Message::RefreshDevicesList()).await.expect("cannot send message");
        sender.send(Message::PopupError("br-adapter-refreshed".to_string(), adw::ToastPriority::Normal)).await.expect("cannot send message");
    }
    else {
        sender.send(Message::SwitchActive(false, bluer::Address::any(), true)).await.expect("cannot send message");
    }

    sender.send(Message::SwitchAdapterPowered(powered)).await.expect("cannot send message");
    Ok(())
}

pub async fn set_adapter_discoverable(adapter_name: String, sender: Sender<Message>) -> bluer::Result<()> {
    let adapter = bluer::Session::new().await?.adapter(adapter_name.as_str())?;

    let current = adapter.is_discoverable().await?;
    adapter.set_discoverable(!current).await?;

    tokio::time::sleep(std::time::Duration::from_secs_f32(0.5)).await;
    let discoverable = adapter.is_discoverable().await?;
    sender.send(Message::SwitchAdapterDiscoverable(discoverable)).await.expect("cannot send message");

    Ok(())
}
