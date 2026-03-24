use eframe::egui;
use std::sync::mpsc;

pub enum TrayAction {
    Start,
    Stop,
    Quit,
}

pub struct ClickerTray {
    pub running: bool,
    pub locked: bool,
    pub clicking: bool,
    pub tx: mpsc::Sender<TrayAction>,
    pub ctx: egui::Context,
}

impl ksni::Tray for ClickerTray {
    fn id(&self) -> String {
        "theclicker-gui".into()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        self.ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        self.ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
            egui::UserAttentionType::Informational,
        ));
        self.ctx.request_repaint();
    }

    fn title(&self) -> String {
        "TheClicker".into()
    }

    fn icon_name(&self) -> String {
        if !self.running {
            "input-mouse"
        } else if self.locked {
            "changes-prevent"
        } else if self.clicking {
            "media-record"
        } else {
            "changes-allow"
        }
        .into()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        let description = if !self.running {
            "Not running"
        } else if self.locked {
            "Locked"
        } else if self.clicking {
            "Clicking"
        } else {
            "Idle"
        };
        ksni::ToolTip {
            title: "TheClicker".into(),
            description: description.into(),
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        vec![
            StandardItem {
                label: "Start".into(),
                enabled: !self.running,
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayAction::Start);
                    this.ctx.request_repaint();
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Stop".into(),
                enabled: self.running,
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayAction::Stop);
                    this.ctx.request_repaint();
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit".into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.tx.send(TrayAction::Quit);
                    this.ctx.request_repaint();
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}
