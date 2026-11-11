# Pixelscale

A Tailscale panel in the [Glowgrid](https://github.com/maxmaster55/pixal_design_system) pixel style:
dark, square, hard-edged, lit only by its palette.

Click the tray icon (or bind a key to `pixelscale toggle`) and a panel drops from the corner
of the screen. It shows your tailnet and lets you change it without a terminal.

- **One switch** turns Tailscale on and off.
- **Devices**: online first, with address, last-seen time and quick actions: copy the address,
  open SSH in your terminal, ping (latency and whether the path is direct or relayed), send a file.
- **Exit node**: pick one of your devices or none, and toggle local-network access.
- **Taildrop**: drop files onto a device or the drop area to send them; save or discard received files.
- **Settings**: switch between accounts, and toggle accept-routes, Tailscale DNS and shields-up.
- **Three themes**: Ember, Forge and Frost.

It talks to the Tailscale daemon directly over its local socket, so it needs `tailscaled`
running but not the `tailscale` command.

## Requirements

- Linux with Tailscale installed and running.
- GTK 4.12 or newer.
- [gtk4-layer-shell](https://github.com/wmww/gtk4-layer-shell) 1.0 or newer.

| Desktop | How it opens |
| --- | --- |
| niri, Sway, Hyprland, river, KDE Plasma (Wayland) | Panel attached to a screen corner; closes on Esc or a click elsewhere |
| GNOME, X11 desktops | Ordinary window |

To let Pixelscale change settings without `sudo`, make yourself the Tailscale operator once:

```sh
sudo tailscale set --operator=$USER
```

Without this it can still show status, devices and ping them.

## Install

### Release tarball

Needs a distro at least as new as Ubuntu 24.04 (Debian 13, Fedora 40, Arch).

```sh
# Debian / Ubuntu
sudo apt install libgtk-4-1 libgtk4-layer-shell0
# Fedora
sudo dnf install gtk4 gtk4-layer-shell
# Arch
sudo pacman -S gtk4 gtk4-layer-shell
```

Download the latest `pixelscale-*-x86_64-linux.tar.gz` from the
[releases page](https://github.com/maxmaster55/pixelscale/releases), then:

```sh
tar -xzf pixelscale-*-x86_64-linux.tar.gz
cd pixelscale-*-x86_64-linux
./install.sh
```

This installs into `~/.local` (binary, launcher entry, the four fonts) and makes Pixelscale
start with your session. `./install.sh uninstall` removes it.

### From source

```sh
# build dependencies: a Rust toolchain, pkg-config, GTK 4 and gtk4-layer-shell headers
#   Debian / Ubuntu: sudo apt install cargo pkg-config libgtk-4-dev libgtk4-layer-shell-dev
#   Fedora:          sudo dnf install cargo pkgconf gtk4-devel gtk4-layer-shell-devel
#   Arch:            sudo pacman -S rust pkgconf gtk4 gtk4-layer-shell
git clone https://github.com/maxmaster55/pixelscale
cd pixelscale
cargo build --release
./install.sh
```

### Nix

```sh
nix run github:maxmaster55/pixelscale
```

In a flake-based NixOS or Home Manager config, add the input and install the package:

```nix
inputs.pixelscale.url = "github:maxmaster55/pixelscale";
# ...
home.packages = [ inputs.pixelscale.packages.${pkgs.system}.default ];
```

## Use

```
pixelscale            show or hide the panel (starts the background process if needed)
pixelscale show       show it
pixelscale hide       hide it
pixelscale quit       stop the background process
pixelscale status     print the Tailscale state as one line of JSON, without opening anything
```

One background process owns the window and the tray icon; running the command again talks
to it, so a bar button or key binding opens the panel instantly.

Options, read when the background process starts:

```
--window      open as an ordinary window
--panel       open as a panel attached to a screen corner
--background  start without showing anything (what autostart uses)
--no-tray     do not show a tray icon
```

`pixelscale status` is meant for status bars:

```json
{"exit_node":null,"online":3,"running":true,"state":"Running","text":"Connected, 3 online"}
```

### Tray icon

A 3×3 lattice of cells. A lit "T" means connected; a coloured corner cell means an exit node
is in use (theme colour) or that you need to log in (red). Left click toggles the panel; the
menu has open, on/off and quit. On GNOME the tray needs the AppIndicator extension.

## Settings

`~/.config/pixelscale/config.toml`. Every key is optional; these are the defaults.

```toml
theme = "ember"        # "ember", "forge" or "frost"
tray = true            # show the tray icon
mode = "auto"          # "auto", "panel" or "window"

# Terminal used for SSH; the ssh command is appended to it. Empty means $TERMINAL, then the
# first installed of kitty, foot, alacritty, wezterm, gnome-terminal, konsole, xterm.
terminal = []
# The command run in that terminal. {host} becomes the device's MagicDNS name.
ssh = ["ssh", "{host}"]          # for Tailscale SSH: ["tailscale", "ssh", "{host}"]

[panel]
anchor = "top-right"   # "top-right", "top-left", "bottom-right", "bottom-left"
margin_x = 12          # gap from the screen edges, in pixels
margin_y = 8
width = 440
height = 600

[taildrop]
directory = ""         # where received files are saved; empty means your Downloads folder
auto_save = false      # save received files as they arrive
```

On a Glowgrid desktop, Pixelscale follows the desktop's theme (`~/.local/state/glowgrid/theme`)
and ignores `theme`.

## Fonts

The look relies on four open fonts, bundled in `assets/fonts` and installed by the install
script and the Nix package: Pixelify Sans, Silkscreen, VT323 and IBM Plex Sans (all under the
SIL Open Font License). Without them it falls back to your system's sans and monospace fonts.

## Limits

- Taildrop sending reads one file at a time and does not send folders.
- Receiving files needs the operator permission above.
- The panel's position is a screen corner with margins; it does not follow the tray icon.
- Subnet routes, advertising an exit node, and Tailscale Serve/Funnel are not covered. Use the
  `tailscale` command for those.

## Licence

GPL-3.0-or-later. See [LICENSE](LICENSE). Fonts are under their own licences in `assets/fonts`.

Pixelscale is an independent project and is not affiliated with Tailscale Inc.
