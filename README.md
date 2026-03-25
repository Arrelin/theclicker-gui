# theclicker-gui

A graphical Linux autoclicker for X11 and Wayland, powered by [theclicker](https://crates.io/crates/theclicker).

## Requirements

- Linux (X11 or Wayland)
- Access to `/dev/input/` and `/dev/uinput` — typically requires the user to be in the `input` and `uinput` groups:
  ```bash
  sudo usermod -aG input,uinput $USER
  ```

## Installation

```bash
cargo install theclicker-gui
```

## Features

- Select input device from a list or detect it automatically by clicking ("Find Mouse")
- Configure bindings for left, middle, and right click autoclicker
- Lock/Unlock binding to pause clicking without stopping
- Hold mode — hold the bind to click, release to stop
- Grab mode — captures the input device so bindings don't pass through to the system
- Configurable cooldown (ms) and press-release gap (ms)
- System tray icon (SNI) showing current state: idle / locked / clicking
- Settings are persisted across restarts
- Global Start/Stop hotkey (keyboard binding to toggle the clicker from anywhere)

## Usage

Launch the GUI:
```bash
theclicker-gui
# or directly
~/.cargo/bin/theclicker-gui
```

1. Select your input device from the dropdown or press **Find Mouse** and click with your mouse
2. Enable and configure bindings in the **Bindings** section
3. Adjust cooldown and other settings
4. Press **Start**

## Notes

- Minimum cooldown is 25 ms (~40 clicks/sec), which is the Linux kernel limit for uinput events
- Grab mode may softlock input if your compositor does not recognize the virtual device created by theclicker
- The system tray icon requires a compositor or panel that supports the StatusNotifierItem (SNI) protocol (KDE Plasma, waybar, etc.)
