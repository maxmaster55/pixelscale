#!/bin/sh
# Install Pixelscale for the current user from a release tarball or a source checkout.
#   ./install.sh            install into ~/.local
#   ./install.sh uninstall  remove it again
# Run it from the unpacked release folder (it contains a `pixelscale` binary), or from a
# source checkout after `cargo build --release`.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
prefix="${PREFIX:-$HOME/.local}"
id=io.github.maxmaster55.Pixelscale

if [ "${1:-}" = uninstall ]; then
    rm -f "$prefix/bin/pixelscale" \
          "$prefix/share/applications/$id.desktop" \
          "$prefix/share/icons/hicolor/scalable/apps/$id.svg" \
          "${XDG_CONFIG_HOME:-$HOME/.config}/autostart/$id.desktop"
    rm -rf "$prefix/share/fonts/pixelscale" "$prefix/lib/pixelscale"
    echo "Pixelscale removed. Your settings in ~/.config/pixelscale were left alone."
    exit 0
fi

if [ -x "$here/pixelscale" ]; then
    bin="$here/pixelscale"
elif [ -x "$here/target/release/pixelscale" ]; then
    bin="$here/target/release/pixelscale"
else
    echo "No pixelscale binary found next to this script." >&2
    echo "Download a release, or build one first: cargo build --release" >&2
    exit 1
fi

install -Dm755 "$bin" "$prefix/bin/pixelscale"
install -Dm644 "$here/data/$id.desktop" "$prefix/share/applications/$id.desktop"
install -Dm644 "$here/data/$id.svg" "$prefix/share/icons/hicolor/scalable/apps/$id.svg"
# release tarballs carry gtk4-layer-shell; the binary looks in ../lib/pixelscale first
if [ -d "$here/lib" ]; then
    mkdir -p "$prefix/lib/pixelscale"
    find "$here/lib" -name '*.so*' -exec install -m644 {} "$prefix/lib/pixelscale/" \;
fi
mkdir -p "$prefix/share/fonts/pixelscale"
find "$here/assets/fonts" -name '*.ttf' -exec install -m644 {} "$prefix/share/fonts/pixelscale/" \;
command -v fc-cache >/dev/null 2>&1 && fc-cache -f "$prefix/share/fonts/pixelscale" >/dev/null 2>&1 || true

# start with the session, hidden, so the tray icon and received-file notices work
auto="${XDG_CONFIG_HOME:-$HOME/.config}/autostart"
mkdir -p "$auto"
sed 's/^Exec=.*/Exec=pixelscale --background/' "$here/data/$id.desktop" > "$auto/$id.desktop"

echo "Pixelscale installed to $prefix/bin/pixelscale"
case ":$PATH:" in
    *":$prefix/bin:"*) ;;
    *) echo "Note: $prefix/bin is not on your PATH." ;;
esac
if ! "$prefix/bin/pixelscale" --version >/dev/null 2>&1; then
    echo "It did not start. It needs GTK 4.12 or newer:" >&2
    echo "  Debian/Ubuntu: sudo apt install libgtk-4-1" >&2
    echo "  Fedora:        sudo dnf install gtk4" >&2
    echo "  Arch:          sudo pacman -S gtk4" >&2
    echo "A source build also needs gtk4-layer-shell from your distro." >&2
    exit 1
fi
cat <<'MSG'

To let Pixelscale change Tailscale settings without sudo, run once:
    sudo tailscale set --operator=$USER

Start it now with:  pixelscale
MSG
