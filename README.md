# btman 🚀
![Version](https://img.shields.io/badge/version-0.6.22-blue)
A minimal GTK4/libadwaita Bluetooth pairing manager for Wayland 💙

Originally forked from [overskride](https://github.com/kaii-lb/overskride), then scoped down to device pairing/management only.

## 📦 Prerequisites for building
- gtk4 and libadwaita (development packages)
- rust & cargo
- bluez (installed by default on all distros)

## 🔨 Compiling
```bash
git clone https://github.com/antraxbr666/overskride && cd overskride
meson setup build && cd build
meson compile
./src/btman
```

## ✨ Features
- 📱 Dynamically enumerate and list all devices
- 🔐 Pair/unpair devices (passkey confirmation)
- 📶 Battery level display
- ⚡ Powered/discoverable toggles
