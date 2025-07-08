use hudhook::*;
use imgui::{Condition, WindowFlags};

pub struct WarbandWindow {
    window_visible: bool,
}

impl WarbandWindow {
    pub fn new() -> Self {
        Self {
            window_visible: false,
        }
    }

    pub fn toggle_visibility(&mut self) {
        self.window_visible = !self.window_visible;
    }
}

impl ImguiRenderLoop for WarbandWindow {
    fn render(&mut self, ui: &mut imgui::Ui) {
        if self.window_visible {
            ui.window("Warband Info")
                .size([260.0, 48.0], Condition::Always)
                .position([1380.0, 40.0], Condition::Always)
                .flags(WindowFlags::NO_MOVE 
                    | WindowFlags::NO_RESIZE 
                    | WindowFlags::NO_COLLAPSE
                    | WindowFlags::NO_TITLE_BAR)
                .build(|| {
                    ui.text("You can hire: 0");
                });
        }
    }
}
