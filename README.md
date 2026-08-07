# btman 🚀
![Version](https://img.shields.io/badge/version-0.6.52-blue)
A minimal GTK4/libadwaita Bluetooth pairing manager for Wayland 💙

Originally forked from [overskride](https://github.com/kaii-lb/overskride), then scoped down to device pairing/management only.

## 📦 Prerequisites for building
- gtk4 and libadwaita (development packages)
- glib2 (for `glib-compile-schemas`, used by `build.rs`)
- rust & cargo
- bluez (installed by default on all distros)

## 🔨 Compiling (development)
```bash
git clone https://github.com/antraxbr666/btman && cd btman
cargo run
```

## 📦 Installing on Arch Linux (PKGBUILD)
```bash
git clone git@github.com:antraxbr666/btman.git && cd btman
makepkg -si
```
This builds `btman-x.y.z-1-x86_64.pkg.tar.zst` and installs it via `pacman -U`.
The version is tied to a matching git tag (`v<version>` = same version).

## ✨ Features
- 📱 Dynamically enumerate and list all devices
- 🔐 Pair/unpair devices (passkey confirmation)
- 📶 Battery level display
- ⚡ Powered/discoverable toggles
