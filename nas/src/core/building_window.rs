use hudhook::*;
use imgui::{Condition, WindowFlags};

pub struct BuildingWindow {
    window_visible: bool,
    window_pos: [f32; 2],
    window_size: [f32; 2],
}

impl BuildingWindow {
    pub fn new() -> Self {
        Self {
            window_visible: false,
            window_pos: [1453.0, 111.0],
            window_size: [200.0, 513.0],
        }
    }

    pub fn toggle_visibility(&mut self) {
        self.window_visible = !self.window_visible;
    }
}

impl ImguiRenderLoop for BuildingWindow {
    fn render(&mut self, ui: &mut imgui::Ui) {
        if self.window_visible {
            let _window = ui.window("Building Info")
                .size(self.window_size, Condition::FirstUseEver)
                .position(self.window_pos, Condition::FirstUseEver)
                .flags(WindowFlags::NO_MOVE 
                    | WindowFlags::NO_RESIZE 
                    | WindowFlags::NO_COLLAPSE
                    | WindowFlags::NO_TITLE_BAR)
                .build(|| {
                    ui.text("Building Info");
                });
        }
    }
}
