//! Developer aid: render one screen, save it as raw RGBA, and quit.
//!
//! `DESK_SNAPSHOT=/path/out.rgba DESK_SCREEN=board cargo run`
//! The file starts with a `width height\n` header followed by RGBA bytes.

use eframe::egui;

pub struct Snapshot {
    pub out: std::path::PathBuf,
    pub screen: String,
    frames: u32,
    requested: bool,
}

impl Snapshot {
    pub fn from_env() -> Option<Self> {
        let out = std::env::var_os("DESK_SNAPSHOT")?;
        Some(Self {
            out: out.into(),
            screen: std::env::var("DESK_SCREEN").unwrap_or_default(),
            frames: 0,
            requested: false,
        })
    }

    /// Call once per frame; asks for a screenshot after the layout settles.
    pub fn tick(&mut self, ctx: &egui::Context) {
        self.frames += 1;
        ctx.request_repaint();
        if self.requested || self.frames < 6 {
            return;
        }
        self.requested = true;
        let out = self.out.clone();
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(
            egui::ScreenshotCallback::new(move |image| {
                let mut bytes = format!("{} {}\n", image.size[0], image.size[1]).into_bytes();
                for pixel in &image.pixels {
                    bytes.extend_from_slice(&pixel.to_array());
                }
                let _ = std::fs::write(&out, bytes);
                std::process::exit(0);
            }),
        ));
    }
}
