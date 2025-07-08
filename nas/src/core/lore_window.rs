use hudhook::*;
use imgui::{Condition, WindowFlags};
use crate::commands::game_common::GameCommon;
use std::sync::Arc;

pub struct LoreWindow {
    pub window_visible: bool,
    game_common: Option<Arc<GameCommon>>,
    base_position: [f32; 2],
    current_width: u32,
    current_height: u32,
}

// Possible base_position:
// 420, 200
// 320, 40

impl LoreWindow {
    pub fn new() -> Self {
        Self {
            window_visible: true,
            game_common: None,
            base_position: [320.0, 40.0],
            current_width: 1920,
            current_height: 1080,
        }
    }

    pub fn toggle_visibility(&mut self) {
        self.window_visible = !self.window_visible;
    }

    pub fn set_game_common(&mut self, game_common: Arc<GameCommon>) {
        self.game_common = Some(game_common);
    }

    fn update_dimensions(&mut self) {
        if let Some(game_common) = &self.game_common {
            if let (Ok(width), Ok(height)) = (game_common.get_window_width(), game_common.get_window_height()) {
                if width != self.current_width || height != self.current_height {
                    self.current_width = width;
                    self.current_height = height;
                    tracing::debug!("Window dimensions updated to: {}x{}", width, height);
                }
            }
        }
    }

    fn get_adjusted_position(&self) -> [f32; 2] {
        let width_ratio = self.current_width as f32 / 1920.0;
        let height_ratio = self.current_height as f32 / 1080.0;

        [
            self.base_position[0] * width_ratio,
            self.base_position[1] * height_ratio,
        ]
    }
}

impl ImguiRenderLoop for LoreWindow {
    fn render(&mut self, ui: &mut imgui::Ui) {
        if self.window_visible {
            self.update_dimensions();
            let position = self.get_adjusted_position();
            
            ui.window("Lore Info")
                .size([260.0, 96.0], Condition::Always)
                .position(position, Condition::Always)
                .flags(WindowFlags::NO_MOVE 
                    | WindowFlags::NO_RESIZE 
                    | WindowFlags::NO_COLLAPSE
                    | WindowFlags::NO_TITLE_BAR)
                .build(|| {
                    ui.text(format!("Window size: {}x{}", self.current_width, self.current_height));
                    ui.text("Next lore suggestion: 0");
                });
        }
    }
}
