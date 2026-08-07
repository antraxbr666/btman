# Maintainer: antraX <antraxbr666@proton.me>
# Based on the original overskride PKGBUILD from kaii

pkgname=btman
pkgver=0.6.47
pkgrel=1
pkgdesc="A minimal GTK4/libadwaita Bluetooth pairing manager for Wayland"
arch=('x86_64')
url="https://github.com/antraxbr666/btman"
license=('GPL3')
depends=('gtk4' 'libadwaita' 'dbus' 'bluez' 'glib2')
makedepends=('rust' 'cargo' 'pkg-config' 'gcc' 'glib2' 'gtk3')
source=("btman::git+ssh://git@github.com/antraxbr666/btman.git#tag=v${pkgver}")
install='btman.install'
sha256sums=('SKIP')

build() {
  cd "$srcdir/btman"
  cargo build --release --locked
}

package() {
  cd "$srcdir/btman"

  install -Dm755 target/release/btman "$pkgdir/usr/bin/btman"

  install -Dm644 data/io.github.antraxbr666.Btman.desktop.in \
    "$pkgdir/usr/share/applications/io.github.antraxbr666.Btman.desktop"
  install -Dm644 data/io.github.antraxbr666.Btman.appdata.xml.in \
    "$pkgdir/usr/share/metainfo/io.github.antraxbr666.Btman.appdata.xml"
  install -Dm644 data/io.github.antraxbr666.Btman.gschema.xml \
    "$pkgdir/usr/share/glib-2.0/schemas/io.github.antraxbr666.Btman.gschema.xml"
  glib-compile-schemas "$pkgdir/usr/share/glib-2.0/schemas"

  install -Dm644 data/icons/hicolor/scalable/apps/io.github.antraxbr666.Btman.svg \
    "$pkgdir/usr/share/icons/hicolor/scalable/apps/io.github.antraxbr666.Btman.svg"
  install -Dm644 data/icons/hicolor/symbolic/apps/io.github.antraxbr666.Btman.svg \
    "$pkgdir/usr/share/icons/hicolor/symbolic/apps/io.github.antraxbr666.Btman.svg"
  install -Dm644 data/icons/hicolor/symbolic/apps/io.github.antraxbr666.Btman-symbolic.svg \
    "$pkgdir/usr/share/icons/hicolor/symbolic/apps/io.github.antraxbr666.Btman-symbolic.svg"

  gtk-update-icon-cache -q -f -t "$pkgdir/usr/share/icons/hicolor"

  install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
}
