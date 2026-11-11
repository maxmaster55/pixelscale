#!/bin/sh
# Build dependencies for CI on Ubuntu: GTK 4 and gtk4-layer-shell.
# gtk4-layer-shell is taken from the distro when it has it, else built from source.
set -eu
sudo apt-get update
sudo apt-get install -y libgtk-4-dev pkg-config
if ! sudo apt-get install -y libgtk4-layer-shell-dev; then
    sudo apt-get install -y meson ninja-build libwayland-dev wayland-protocols git
    git clone --depth 1 --branch v1.3.0 https://github.com/wmww/gtk4-layer-shell /tmp/gtk4-layer-shell
    meson setup /tmp/gtk4-layer-shell/build /tmp/gtk4-layer-shell --prefix=/usr \
        -Dexamples=false -Ddocs=false -Dtests=false -Dintrospection=false -Dvapi=false
    sudo ninja -C /tmp/gtk4-layer-shell/build install
    sudo ldconfig
fi
pkg-config --modversion gtk4 gtk4-layer-shell-0
