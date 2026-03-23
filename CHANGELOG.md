# Changelog

## [0.1.4]

### Added

- Global Start/Stop hotkey: bind any key combination (including Ctrl, Alt, Shift, Super modifiers) to toggle the clicker from anywhere

## [0.1.3]

### Added

- Console logging with configurable log level via `--log-level` flag:
  ```bash
  theclicker-gui --log-level info
  theclicker-gui --log-level debug
  theclicker-gui --log-level trace  # includes raw theclicker stdout output
  ```
  Available levels: `error`, `warn` (default), `info`, `debug`, `trace`

## [0.1.2]

### Added

- System tray: Start/Stop actions via context menu
- System tray: left click raises the application window
