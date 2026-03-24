use eframe::egui::{self, Align, Color32, Layout, RichText};

use crate::config::HotkeyBind;
use crate::input::{hotkey_label, key_label};

fn bind_row_inner<T, F>(
    ui: &mut egui::Ui,
    enabled: &mut bool,
    label: &str,
    bind: &mut Option<T>,
    fmt: F,
) -> bool
where
    F: Fn(&T) -> String,
{
    let mut capture = false;
    ui.horizontal(|ui| {
        ui.checkbox(enabled, label);
        if *enabled {
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                let text = bind.as_ref().map(&fmt).unwrap_or_else(|| "not set".to_string());
                ui.label(RichText::new(text).monospace().color(if bind.is_some() {
                    Color32::GREEN
                } else {
                    Color32::from_rgb(180, 100, 100)
                }));
                if ui.small_button("Capture").clicked() {
                    capture = true;
                }
                if bind.is_some() && ui.small_button("Clear").clicked() {
                    *bind = None;
                }
            });
        }
    });
    capture
}

pub fn bind_row(
    ui: &mut egui::Ui,
    enabled: &mut bool,
    label: &str,
    bind: &mut Option<u16>,
) -> bool {
    bind_row_inner(ui, enabled, label, bind, |&code| key_label(code))
}

pub fn hotkey_bind_row(
    ui: &mut egui::Ui,
    enabled: &mut bool,
    bind: &mut Option<HotkeyBind>,
) -> bool {
    bind_row_inner(ui, enabled, "Start/Stop hotkey", bind, hotkey_label)
}
