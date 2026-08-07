---
name: btman-ui
description: Use when editing btman's UI/UX — buttons, pairing behavior, device lists, dialogs, or any GTK/libadwaita widget in this project. Enforce the project's visual consistency and interaction conventions so every change matches. Also use proactively before touching src/window.rs, src/widgets/*.rs, src/bluetooth/device.rs, or src/bluetooth/agent.rs.
---

# btman UI & Interaction Conventions

Codified standards for this project so the UI stays visually consistent and
behaviors stay predictable. ALWAYS follow these; do not invent your own.

## Stack
- GTK4 + libadwaita, pure Rust builder pattern (NO Blueprint, NO .ui, NO GResource).
- Build/run: `cargo build --release` / `cargo run`. After any code change run
  auto-commit: `/home/antrax/.config/opencode/skills/auto-commit/auto-commit.sh`.
- Build must be clean (zero warnings) in both `debug` and `release` before committing.

## Button visual standard (CRITICAL — user repeatedly complained)
All ACTION buttons in the app must share the SAME look: a filled accent pill.
Achieve this with the libadwaita `suggested-action` CSS class. This applies to:
- **Pair** button (Devices in-range list rows)
- **Connect / Disconnect** button (Paired Devices list rows) — ALWAYS `suggested-action`,
  regardless of connected state (do NOT remove the class when connected).
- **Forget** button (Paired Devices list rows) — a text button with `suggested-action`.
- **Remove** confirm button in the confirmation dialog — the suggested/filled style.

Rules:
- Do NOT use `flat`, icon-only buttons for actions that connect/pair/forget devices.
- Do NOT mix gray/plain buttons with blue pills — that was the inconsistency the user kept flagging.
- Buttons are built via `gtk::Button::new()`, with a child `gtk::Box(Horizontal, 6)`
  containing an invisible `gtk::Spinner` (16x16) + a `gtk::Label`, then `add_suffix`.
- Re-entry guard: `connect_clicked` returns early if `spinner.is_visible()` (prevents double-click).
- While an action runs: spinner shown, label hidden, keep the button enabled (do NOT set insensitive).
- Cancel/secondary dialog button stays a plain/normal button (no `suggested-action`).

## Pairing behavior
- Pairing a device = `set_device_active` (src/bluetooth/device.rs:15): pair then
  double-connect with a ~200ms settle after pair (Just-Works). On error it sends
  `SwitchActiveSpinner(false,address)` then `PopupError`.
- After a device becomes paired it must move from the Devices (in-range) list to the
  Paired Devices list WITHOUT a restart. This is driven by the BlueZ
  `DeviceProperty::Paired(true)` event handler in device.rs. If a device pair does not
  show up in Paired Devices until restart, that handler (or the AddPairedRow path) is
  broken — fix it, do not tell the user to restart.
- Agent is registered with `set_trust=true` (agent.rs `register_agent(&session, true, true, ...)`).

## Device lists
- Two lists: **Paired Devices** (top) and **Devices** (in-range, below).
- Row subtitle format: `Signal: {rssi_to_percent}%` and, in Paired rows only,
  ` · Battery: {level}%` appended when the device reports battery (level >= 0).
  `rssi_to_percent(rssi) = (rssi.clamp(-100, -50) + 100) * 2`.
- 'Unknown Device' rows are HIDDEN from the Devices (in-range) list.
- When the adapter is powered off, both lists are cleared.

## Concurrency / shared state (crash-avoidance)
- `devices_lut()` is `&'static Mutex<Option<HashMap<bluer::Address,String>>>`.
  NEVER use the old racy `lock().unwrap().take().unwrap(); ... ; *lock()=Some(...)` pattern —
  it exposed `None` and poisoned the mutex (crash). Use single-lock in-place mutation with
  poison tolerance: `devices_lut().lock().unwrap_or_else(|p| p.into_inner())`, and work on
  `guard.as_mut()` / `as_ref()` without ever leaving the Option as `None`.
- All async DBus work must run off the GTK main thread (spawn via `crate::window::runtime().spawn`
  or `std::thread::spawn`); never `#[tokio::main]`-block the UI thread for DBus calls.

## BTMAN_PROPS
Global `OnceLock<Mutex<Props>>` in src/singletons.rs with fields:
name, current_adapter, sender: Option<Sender<Message>>, address, displaying_dialog,
pin_code, pass_key, confirm_authorization. Read/write with `lock().unwrap_or_else(|p| p.into_inner())`.
