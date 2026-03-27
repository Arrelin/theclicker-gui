use eframe::egui::{self, Color32, RichText};
use input_linux::sys::{BTN_LEFT, EV_KEY, input_event};
use std::io::{BufRead, BufReader};
use std::os::unix::io::AsRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use theclicker::InputDevice;

use crate::config::{Config, HotkeyBind};
use crate::input::{clean_name, key_label, load_devices, modifier_bit};
use crate::tray::{ClickerTray, TrayAction};
use crate::types::{Action, KeyTarget, Screen};
use crate::widgets::{bind_row, hotkey_bind_row};

pub struct App {
    pub config: Config,
    screen: Screen,
    devices: Vec<(String, String)>,
    child: Option<std::process::Child>,
    key_rx: Option<mpsc::Receiver<(u16, u8)>>,
    key_target: Option<KeyTarget>,
    find_rx: Option<mpsc::Receiver<String>>,
    find_cancel: Option<Arc<AtomicBool>>,
    key_cancel: Option<Arc<AtomicBool>>,
    hotkey_rx: Option<mpsc::Receiver<()>>,
    hotkey_cancel: Option<Arc<AtomicBool>>,
    hotkey_active: Option<HotkeyBind>,
    status: String,
    tray: Option<ksni::blocking::Handle<ClickerTray>>,
    tray_rx: mpsc::Receiver<TrayAction>,
}

impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        tray: Option<ksni::blocking::Handle<ClickerTray>>,
        tray_rx: mpsc::Receiver<TrayAction>,
    ) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.remove("NotoEmoji-Regular");
        for family in fonts.families.values_mut() {
            family.retain(|f| f != "NotoEmoji-Regular");
        }
        cc.egui_ctx.set_fonts(fonts);
        let config = cc
            .storage
            .and_then(|s| eframe::get_value(s, eframe::APP_KEY))
            .unwrap_or_default();
        Self {
            config,
            screen: Screen::Config,
            devices: load_devices(),
            child: None,
            key_rx: None,
            key_target: None,
            find_rx: None,
            find_cancel: None,
            key_cancel: None,
            hotkey_rx: None,
            hotkey_cancel: None,
            hotkey_active: None,
            status: String::new(),
            tray,
            tray_rx,
        }
    }

}

impl Drop for App {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl App {
    fn update_tray<F: FnOnce(&mut ClickerTray)>(&self, f: F) {
        if let Some(tray) = &self.tray {
            tray.update(f);
        }
    }

    fn reset_tray_state(&self) {
        self.update_tray(|t| {
            t.running = false;
            t.locked = false;
            t.clicking = false;
        });
    }

    fn can_launch(&self) -> bool {
        !self.config.device_name.is_empty() && !self.config.missing_binds()
    }

    fn start_hotkey_monitor(&mut self, bind: HotkeyBind) {
        let (tx, rx) = mpsc::channel::<()>();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_clone = cancel.clone();
        std::thread::spawn(move || {
            let devices = InputDevice::devices();
            let mut pollfds: Vec<libc::pollfd> = devices
                .iter()
                .map(|d| libc::pollfd {
                    fd: d.handler.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                })
                .collect();
            let mut events: [input_event; 1] = unsafe { std::mem::zeroed() };
            let mut current_mods: u8 = 0;
            loop {
                if cancel_clone.load(Ordering::Relaxed) {
                    return;
                }
                let ret = unsafe {
                    libc::poll(pollfds.as_mut_ptr(), pollfds.len() as libc::nfds_t, 100)
                };
                if ret < 0 {
                    return;
                }
                for (i, pfd) in pollfds.iter_mut().enumerate() {
                    if pfd.revents & (libc::POLLERR | libc::POLLHUP) != 0 {
                        return;
                    }
                    if pfd.revents & libc::POLLIN != 0 {
                        pfd.revents = 0;
                        if let Ok(len) = devices[i].read(&mut events) {
                            for event in &events[..len] {
                                if event.type_ == EV_KEY as u16 {
                                    let bit = modifier_bit(event.code);
                                    if bit != 0 {
                                        if event.value == 1 {
                                            current_mods |= bit;
                                        } else if event.value == 0 {
                                            current_mods &= !bit;
                                        }
                                    } else if event.value == 1
                                        && event.code == bind.key
                                        && current_mods == bind.mods
                                    {
                                        if tx.send(()).is_err() {
                                            return;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
        self.hotkey_rx = Some(rx);
        self.hotkey_cancel = Some(cancel);
    }

    fn handle_action(&mut self, action: Action) {
        match action {
            Action::Refresh => {
                self.devices = load_devices();
            }
            Action::FindMouse => {
                let (tx, rx) = mpsc::channel();
                let stop = Arc::new(AtomicBool::new(false));
                let stop_clone = stop.clone();
                let devices = InputDevice::devices();
                std::thread::spawn(move || {
                    let named: Vec<_> = devices
                        .into_iter()
                        .map(|device| {
                            let display = clean_name(&device.name).to_string();
                            let base_name = display
                                .strip_suffix(&format!("-{}", device.filename))
                                .unwrap_or(&display)
                                .to_string();
                            (device, base_name)
                        })
                        .collect();
                    let mut pollfds: Vec<libc::pollfd> = named
                        .iter()
                        .map(|(d, _)| libc::pollfd {
                            fd: d.handler.as_raw_fd(),
                            events: libc::POLLIN,
                            revents: 0,
                        })
                        .collect();
                    let mut events: [input_event; 1] = unsafe { std::mem::zeroed() };
                    loop {
                        if stop_clone.load(Ordering::Relaxed) {
                            return;
                        }
                        let ret = unsafe {
                            libc::poll(pollfds.as_mut_ptr(), pollfds.len() as libc::nfds_t, 100)
                        };
                        if ret < 0 {
                            return;
                        }
                        for (i, pfd) in pollfds.iter_mut().enumerate() {
                            if pfd.revents & (libc::POLLERR | libc::POLLHUP) != 0 {
                        return;
                    }
                    if pfd.revents & libc::POLLIN != 0 {
                                pfd.revents = 0;
                                if let Ok(len) = named[i].0.read(&mut events) {
                                    for event in &events[..len] {
                                        if event.type_ == EV_KEY as u16
                                            && event.code == BTN_LEFT as u16
                                            && event.value == 1
                                        {
                                            let _ = tx.send(named[i].1.clone());
                                            return;
                                        }
                                    }
                                }
                            }
                        }
                    }
                });
                self.find_rx = Some(rx);
                self.find_cancel = Some(stop);
                self.screen = Screen::FindMouse;
            }
            Action::StartCapture(target) => {
                if target != KeyTarget::HotkeyStartStop && self.config.device_name.is_empty() {
                    self.status = "Select a device first".to_string();
                    return;
                }
                let (tx, rx) = mpsc::channel();
                let stop = Arc::new(AtomicBool::new(false));
                let stop_clone = stop.clone();
                if target == KeyTarget::HotkeyStartStop {
                    std::thread::spawn(move || {
                        let devices = InputDevice::devices();
                        for d in &devices {
                            d.empty_read_buffer();
                        }
                        let mut pollfds: Vec<libc::pollfd> = devices
                            .iter()
                            .map(|d| libc::pollfd {
                                fd: d.handler.as_raw_fd(),
                                events: libc::POLLIN,
                                revents: 0,
                            })
                            .collect();
                        let mut events: [input_event; 1] = unsafe { std::mem::zeroed() };
                        let mut current_mods: u8 = 0;
                        loop {
                            if stop_clone.load(Ordering::Relaxed) {
                                return;
                            }
                            let ret = unsafe {
                                libc::poll(
                                    pollfds.as_mut_ptr(),
                                    pollfds.len() as libc::nfds_t,
                                    100,
                                )
                            };
                            if ret < 0 {
                                return;
                            }
                            for (i, pfd) in pollfds.iter_mut().enumerate() {
                                if pfd.revents & (libc::POLLERR | libc::POLLHUP) != 0 {
                        return;
                    }
                    if pfd.revents & libc::POLLIN != 0 {
                                    pfd.revents = 0;
                                    if let Ok(len) = devices[i].read(&mut events) {
                                        for event in &events[..len] {
                                            if event.type_ == EV_KEY as u16 {
                                                let bit = modifier_bit(event.code);
                                                if bit != 0 {
                                                    if event.value == 1 {
                                                        current_mods |= bit;
                                                    } else if event.value == 0 {
                                                        current_mods &= !bit;
                                                    }
                                                } else if event.value == 1 {
                                                    let _ = tx.send((event.code, current_mods));
                                                    return;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    });
                } else {
                    let name = clean_name(&self.config.device_name).to_string();
                    std::thread::spawn(move || {
                        let Some(device) = InputDevice::find_device(&name) else {
                            return;
                        };
                        device.empty_read_buffer();
                        let mut pollfd = libc::pollfd {
                            fd: device.handler.as_raw_fd(),
                            events: libc::POLLIN,
                            revents: 0,
                        };
                        let mut events: [input_event; 1] = unsafe { std::mem::zeroed() };
                        loop {
                            if stop_clone.load(Ordering::Relaxed) {
                                return;
                            }
                            let ret = unsafe { libc::poll(&mut pollfd, 1, 100) };
                            if ret < 0 {
                                return;
                            }
                            if pollfd.revents & (libc::POLLERR | libc::POLLHUP) != 0 {
                                return;
                            }
                            if pollfd.revents & libc::POLLIN != 0 {
                                pollfd.revents = 0;
                                let Ok(len) = device.read(&mut events) else {
                                    return;
                                };
                                for event in &events[..len] {
                                    if event.type_ == EV_KEY as u16 && event.value == 1 {
                                        let _ = tx.send((event.code, 0u8));
                                        return;
                                    }
                                }
                            }
                        }
                    });
                }
                self.key_rx = Some(rx);
                self.key_cancel = Some(stop);
                self.key_target = Some(target);
                self.screen = Screen::KeyCapture;
                self.status.clear();
            }
            Action::Launch => {
                let cfg = &self.config;
                let Ok(exe) = std::env::current_exe() else {
                    self.status = "Failed to resolve current executable".to_string();
                    return;
                };
                let mut cmd = std::process::Command::new(exe);
                cmd.arg("--backend");
                cmd.arg("run");
                cmd.arg(format!("-d{}", clean_name(&cfg.device_name)));
                cmd.arg(format!("-c{}", cfg.cooldown));
                cmd.arg(format!("-C{}", cfg.cooldown_press_release));
                if cfg.enable_left {
                    if let Some(b) = cfg.left_bind {
                        cmd.arg(format!("-l{b}"));
                    }
                }
                if cfg.enable_middle {
                    if let Some(b) = cfg.middle_bind {
                        cmd.arg(format!("-m{b}"));
                    }
                }
                if cfg.enable_right {
                    if let Some(b) = cfg.right_bind {
                        cmd.arg(format!("-r{b}"));
                    }
                }
                if cfg.enable_lock_unlock {
                    if let Some(b) = cfg.lock_unlock_bind {
                        cmd.arg(format!("-T{b}"));
                    }
                }
                if cfg.hold {
                    cmd.arg("-H");
                }
                if cfg.grab {
                    cmd.arg("--grab");
                }
                cmd.stdout(std::process::Stdio::piped());
                cmd.stderr(std::process::Stdio::null());
                log::debug!("Launching theclicker with args: {:?}", cmd.get_args().collect::<Vec<_>>());
                match cmd.spawn() {
                    Ok(mut child) => {
                        log::info!("theclicker started (pid {})", child.id());
                        if let Some(stdout) = child.stdout.take() {
                            let tray = self.tray.clone();
                            std::thread::spawn(move || {
                                let reader = BufReader::new(stdout);
                                let (mut prev_locked, mut prev_clicking) = (false, false);
                                for line in reader.lines() {
                                    let Ok(line) = line else { break };
                                    log::trace!("theclicker: {line}");
                                    if line.starts_with("Active:") {
                                        let locked = line.contains("LOCKED");
                                        let clicking = line.contains("left")
                                            || line.contains("right")
                                            || line.contains("middle");
                                        if locked != prev_locked || clicking != prev_clicking {
                                            prev_locked = locked;
                                            prev_clicking = clicking;
                                            if let Some(ref t) = tray {
                                                t.update(|t| {
                                                    t.locked = locked;
                                                    t.clicking = clicking;
                                                });
                                            }
                                        }
                                    }
                                }
                                if let Some(ref t) = tray {
                                    t.update(|t| {
                                        t.running = false;
                                        t.locked = false;
                                        t.clicking = false;
                                    });
                                }
                            });
                        }
                        self.update_tray(|t| t.running = true);
                        self.child = Some(child);
                        self.screen = Screen::Running;
                        self.status = "Running".to_string();
                    }
                    Err(e) => {
                        log::error!("Failed to start theclicker: {e}");
                        self.status = format!("Failed to start: {e}");
                    }
                }
            }
            Action::Stop => {
                if let Some(mut child) = self.child.take() {
                    log::info!("Stopping theclicker (pid {})", child.id());
                    let _ = child.kill();
                    let _ = child.wait();
                }
                self.reset_tray_state();
                self.screen = Screen::Config;
                self.status = "Stopped".to_string();
            }
        }
    }
}

impl eframe::App for App {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.config);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(ta) = self.tray_rx.try_recv() {
            match ta {
                TrayAction::Start => self.handle_action(Action::Launch),
                TrayAction::Stop => self.handle_action(Action::Stop),
                TrayAction::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            }
        }

        if let Some(rx) = &self.hotkey_rx {
            if rx.try_recv().is_ok() {
                match self.screen {
                    Screen::Running => self.handle_action(Action::Stop),
                    Screen::Config if self.can_launch() => self.handle_action(Action::Launch),
                    _ => {}
                }
            }
        }

        if self.screen == Screen::FindMouse {
            if let Some(rx) = &self.find_rx {
                if let Ok(base_name) = rx.try_recv() {
                    self.config.device_name = base_name;
                    self.find_rx = None;
                    self.find_cancel = None;
                    self.screen = Screen::Config;
                }
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }

        if self.screen == Screen::KeyCapture {
            if let Some(rx) = &self.key_rx {
                if let Ok((code, mods)) = rx.try_recv() {
                    match self.key_target {
                        Some(KeyTarget::LockUnlock) => self.config.lock_unlock_bind = Some(code),
                        Some(KeyTarget::Left) => self.config.left_bind = Some(code),
                        Some(KeyTarget::Middle) => self.config.middle_bind = Some(code),
                        Some(KeyTarget::Right) => self.config.right_bind = Some(code),
                        Some(KeyTarget::HotkeyStartStop) => {
                            self.config.hotkey_bind = Some(HotkeyBind { key: code, mods });
                        }
                        None => {}
                    }
                    self.key_rx = None;
                    self.key_cancel = None;
                    self.key_target = None;
                    self.screen = Screen::Config;
                }
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }

        if self.screen == Screen::Running {
            if let Some(child) = &mut self.child {
                if let Ok(Some(_)) = child.try_wait() {
                    self.child = None;
                    self.screen = Screen::Config;
                    self.status = "Process exited".to_string();
                }
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }

        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
        });

        let mut action: Option<Action> = None;

        egui::CentralPanel::default().show(ctx, |ui| match self.screen {
            Screen::Running => {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(16.0);
                        ui.horizontal(|ui| {
                            let (rect, _) = ui.allocate_exact_size(
                                egui::vec2(14.0, 14.0),
                                egui::Sense::hover(),
                            );
                            ui.painter().circle_filled(rect.center(), 6.0, Color32::GREEN);
                            ui.heading(RichText::new("TheClicker is Running").color(Color32::GREEN));
                        });
                        ui.add_space(16.0);
                    });

                    let cfg = &self.config;

                    ui.group(|ui| {
                        ui.label(RichText::new("Device").strong());
                        ui.add_space(2.0);
                        ui.label(RichText::new(&cfg.device_name).monospace());
                    });

                    ui.add_space(6.0);

                    ui.group(|ui| {
                        ui.label(RichText::new("Bindings").strong());
                        ui.add_space(2.0);
                        egui::Grid::new("running_bindings")
                            .num_columns(2)
                            .spacing([12.0, 4.0])
                            .show(ui, |ui| {
                                if cfg.enable_left {
                                    ui.label("Left click:");
                                    ui.label(RichText::new(cfg.left_bind.map(key_label).unwrap_or_else(|| "—".into())).monospace());
                                    ui.end_row();
                                }
                                if cfg.enable_middle {
                                    ui.label("Middle click:");
                                    ui.label(RichText::new(cfg.middle_bind.map(key_label).unwrap_or_else(|| "—".into())).monospace());
                                    ui.end_row();
                                }
                                if cfg.enable_right {
                                    ui.label("Right click:");
                                    ui.label(RichText::new(cfg.right_bind.map(key_label).unwrap_or_else(|| "—".into())).monospace());
                                    ui.end_row();
                                }
                                if cfg.enable_lock_unlock {
                                    ui.label("Lock/Unlock:");
                                    ui.label(RichText::new(cfg.lock_unlock_bind.map(key_label).unwrap_or_else(|| "—".into())).monospace());
                                    ui.end_row();
                                }
                            });
                    });

                    ui.add_space(6.0);

                    ui.group(|ui| {
                        ui.label(RichText::new("Settings").strong());
                        ui.add_space(2.0);
                        egui::Grid::new("running_settings")
                            .num_columns(2)
                            .spacing([12.0, 4.0])
                            .show(ui, |ui| {
                                ui.label("Cooldown:");
                                ui.label(RichText::new(format!("{} ms", cfg.cooldown)).monospace());
                                ui.end_row();
                                if cfg.cooldown_press_release > 0 {
                                    ui.label("Press-release gap:");
                                    ui.label(RichText::new(format!("{} ms", cfg.cooldown_press_release)).monospace());
                                    ui.end_row();
                                }
                                ui.label("Hold mode:");
                                ui.label(RichText::new(if cfg.hold { "on" } else { "off" }).monospace());
                                ui.end_row();
                                ui.label("Grab device:");
                                ui.label(RichText::new(if cfg.grab { "on" } else { "off" }).monospace());
                                ui.end_row();
                            });
                    });

                    ui.add_space(16.0);
                    ui.vertical_centered(|ui| {
                        if ui
                            .add(egui::Button::new(RichText::new("  Stop  ").size(16.0)))
                            .clicked()
                        {
                            action = Some(Action::Stop);
                        }
                    });
                });
            }
            Screen::KeyCapture => {
                ui.vertical_centered(|ui| {
                    ui.add_space(100.0);
                    ui.heading("Waiting for key press...");
                    ui.add_space(12.0);
                    let target_name = self.key_target.as_ref().map(KeyTarget::label).unwrap_or("");
                    ui.label(format!("Press the key for: {target_name}"));
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("Input is grabbed from your selected device")
                            .weak()
                            .italics(),
                    );
                    ui.add_space(24.0);
                    if ui.button("Cancel").clicked() {
                        if let Some(cancel) = self.key_cancel.take() {
                            cancel.store(true, Ordering::Relaxed);
                        }
                        self.key_rx = None;
                        self.key_target = None;
                        self.screen = Screen::Config;
                    }
                });
            }
            Screen::FindMouse => {
                ui.vertical_centered(|ui| {
                    ui.add_space(100.0);
                    ui.heading("Click left mouse button...");
                    ui.add_space(12.0);
                    ui.label(RichText::new("The device that produces the click will be selected").weak().italics());
                    ui.add_space(24.0);
                    if ui.button("Cancel").clicked() {
                        if let Some(cancel) = self.find_cancel.take() {
                            cancel.store(true, Ordering::Relaxed);
                        }
                        self.find_rx = None;
                        self.screen = Screen::Config;
                    }
                });
            }
            Screen::Config => {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.heading("Device");
                            if ui.small_button("↺ Refresh").clicked() {
                                action = Some(Action::Refresh);
                            }
                            if ui.small_button("Find Mouse").clicked() {
                                action = Some(Action::FindMouse);
                            }
                        });
                        ui.add_space(4.0);

                        let selected_label = if self.config.device_name.is_empty() {
                            "Select device..."
                        } else {
                            &self.config.device_name
                        };

                        egui::ComboBox::from_id_salt("device_select")
                            .selected_text(selected_label)
                            .width(ui.available_width() - 8.0)
                            .show_ui(ui, |ui| {
                                for (display, base_name) in &self.devices {
                                    ui.selectable_value(
                                        &mut self.config.device_name,
                                        base_name.clone(),
                                        display,
                                    );
                                }
                            });
                    });

                    ui.add_space(6.0);

                    ui.group(|ui| {
                        ui.heading("Bindings");
                        ui.add_space(4.0);

                        if bind_row(ui, &mut self.config.enable_lock_unlock, "Lock/Unlock", &mut self.config.lock_unlock_bind) {
                            action = Some(Action::StartCapture(KeyTarget::LockUnlock));
                        }
                        if bind_row(ui, &mut self.config.enable_left, "Left click", &mut self.config.left_bind) {
                            action = Some(Action::StartCapture(KeyTarget::Left));
                        }
                        if bind_row(ui, &mut self.config.enable_middle, "Middle click", &mut self.config.middle_bind) {
                            action = Some(Action::StartCapture(KeyTarget::Middle));
                        }
                        if bind_row(ui, &mut self.config.enable_right, "Right click", &mut self.config.right_bind) {
                            action = Some(Action::StartCapture(KeyTarget::Right));
                        }

                        ui.separator();

                        if hotkey_bind_row(ui, &mut self.config.enable_hotkey, &mut self.config.hotkey_bind) {
                            action = Some(Action::StartCapture(KeyTarget::HotkeyStartStop));
                        }
                    });

                    ui.add_space(6.0);

                    ui.group(|ui| {
                        ui.heading("Settings");
                        ui.add_space(4.0);

                        ui.checkbox(
                            &mut self.config.hold,
                            "Hold mode (hold key to click, release to stop)",
                        );

                        ui.horizontal(|ui| {
                            ui.checkbox(&mut self.config.grab, "Grab device");
                            if self.config.grab {
                                ui.label(
                                    RichText::new(
                                        "⚠ May softlock if compositor ignores TheClicker device",
                                    )
                                    .color(Color32::YELLOW)
                                    .small(),
                                );
                            }
                        });

                        if ui.checkbox(&mut self.config.no_min_delay, "My system can go faster").changed()
                            && !self.config.no_min_delay
                        {
                            self.config.cooldown = self.config.cooldown.max(25);
                        }

                        ui.add_space(4.0);

                        egui::Grid::new("settings_grid")
                            .num_columns(2)
                            .spacing([8.0, 4.0])
                            .show(ui, |ui| {
                                let cooldown_label = if self.config.no_min_delay {
                                    "Cooldown (ms):"
                                } else {
                                    "Cooldown (ms, min 25):"
                                };
                                ui.label(cooldown_label);
                                let mut s = self.config.cooldown.to_string();
                                if ui
                                    .add(egui::TextEdit::singleline(&mut s).desired_width(60.0))
                                    .changed()
                                {
                                    if let Ok(v) = s.parse::<u64>() {
                                        let min = if self.config.no_min_delay { 1 } else { 25 };
                                        self.config.cooldown = v.max(min);
                                    }
                                }
                                ui.end_row();

                                ui.label("Press-release gap (ms):");
                                let mut s = self.config.cooldown_press_release.to_string();
                                if ui
                                    .add(egui::TextEdit::singleline(&mut s).desired_width(60.0))
                                    .changed()
                                {
                                    if let Ok(v) = s.parse::<u64>() {
                                        self.config.cooldown_press_release = v;
                                    }
                                }
                                ui.end_row();
                            });
                    });

                    ui.add_space(10.0);

                    let missing_device = self.config.device_name.is_empty();
                    let missing_bind = self.config.missing_binds();

                    ui.vertical_centered(|ui| {
                        if ui
                            .add_enabled(
                                !missing_device && !missing_bind,
                                egui::Button::new(RichText::new("  Start  ").size(16.0)),
                            )
                            .clicked()
                        {
                            action = Some(Action::Launch);
                        }

                        if missing_device {
                            ui.label(
                                RichText::new("Select a device first")
                                    .color(Color32::from_rgb(220, 100, 100)),
                            );
                        } else if missing_bind {
                            ui.label(
                                RichText::new("Capture all enabled bindings first")
                                    .color(Color32::from_rgb(220, 100, 100)),
                            );
                        }

                        if !self.status.is_empty() {
                            ui.add_space(4.0);
                            ui.label(RichText::new(&self.status).weak());
                        }
                    });
                });
            }
        });

        if let Some(a) = action {
            self.handle_action(a);
        }

        let target = if self.config.enable_hotkey { self.config.hotkey_bind } else { None };
        if target != self.hotkey_active {
            if let Some(cancel) = self.hotkey_cancel.take() {
                cancel.store(true, Ordering::Relaxed);
            }
            self.hotkey_rx = None;
            self.hotkey_active = target;
            if let Some(bind) = target {
                self.start_hotkey_monitor(bind);
            }
        }

        if self.hotkey_rx.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
    }
}
