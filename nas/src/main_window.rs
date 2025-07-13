use std::fs::File;
use std::sync::Mutex;
use std::sync::Arc;

use hudhook::*;
use imgui::Condition;
use imgui::Key;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};
use crate::commands::auto_accept::AutoAccept;
use crate::commands::base::{Command, CommandContext};
use crate::commands::lobby_members::LobbyMembers;
use crate::commands::auto_lockin::AutoLockin;
use crate::commands::game_common::GameCommon;
use crate::commands::build_guide::{BuildGuideManager};
use crate::core::building_window::BuildingWindow;
use crate::core::lore_window::LoreWindow;
use crate::core::warband_window::WarbandWindow;


pub fn setup_tracing() {
    hudhook::alloc_console().unwrap();
    hudhook::enable_console_colors();
    std::env::set_var("RUST_LOG", "trace");

    let log_file = hudhook::util::get_dll_path()
        .map(|mut path| {
            path.set_extension("log");
            path
        })
        .and_then(|path| File::create(path).ok())
        .unwrap();

    tracing_subscriber::registry()
        .with(
            fmt::layer().event_format(
                fmt::format()
                    .with_level(true)
                    .with_thread_ids(true)
                    .with_file(true)
                    .with_line_number(true)
                    .with_thread_names(true),
            ),
        )
        .with(
            fmt::layer()
                .with_thread_ids(true)
                .with_file(true)
                .with_line_number(true)
                .with_thread_names(true)
                .with_writer(Mutex::new(log_file))
                .with_ansi(false)
                .boxed(),
        )
        .with(EnvFilter::from_default_env())
        .init();

    tracing::info!("Tracing initialized");
}

#[allow(dead_code)]
pub struct MainWindow {
    /// GUI
    checkbox_auto_accept: bool,
    window_visible: bool,
    selected_clan: Option<usize>,

    /// General
    pid: u32,
    command_context: Option<CommandContext>,
    auto_accept: Option<AutoAccept>,
    auto_lockin: Option<AutoLockin>,
    game_common: Option<Arc<GameCommon>>,
    lobby_members: Option<LobbyMembers>,
    lobby_members_enabled: bool,
    selected_color: Option<usize>,
    building_window: Option<BuildingWindow>,
    lore_window: Option<LoreWindow>,
    warband_window: Option<WarbandWindow>,
    build_guide_manager: Option<BuildGuideManager>,
    selected_guide: Option<String>,
}

impl MainWindow {
    pub fn new() -> Self {
        Self {
            checkbox_auto_accept: false,
            window_visible: true,
            selected_clan: None,
            pid: 0,
            command_context: None,
            auto_accept: None,
            auto_lockin: None,
            game_common: None,
            lobby_members: None,
            lobby_members_enabled: false,
            selected_color: None,
            building_window: None,
            lore_window: None,
            warband_window: None,
            build_guide_manager: None,
            selected_guide: None,
        }
    }

    fn update_pid(&mut self) {
        self.pid = std::process::id();
    }
}

const FONT_DATA: &[u8] = include_bytes!("../assets/Microsoft Yahei.ttf");

impl ImguiRenderLoop for MainWindow {
    fn initialize<'a>(
        &'a mut self,
        ctx: &mut imgui::Context,
        _render_context: &'a mut dyn RenderContext,
    ) {
        self.update_pid();

        if self.build_guide_manager.is_none() {
            match BuildGuideManager::new() {
                Ok(manager) => self.build_guide_manager = Some(manager),
                Err(e) => tracing::error!("Failed to initialize build guide manager: {}", e),
            }
        }

        /*
        1252 Latin 1
        1250 Latin 2: Eastern Europe
        1251 Cyrillic
        1253 Greek
        1254 Turkish
        936 Chinese: Simplified chars--PRC and Singapore
        */
        const GLYPH_RANGES: &[u32] = &[
            0x0020, 0x00FF,  // Basic Latin + Latin Supplement
            0x0100, 0x017F,  // Latin Extended-A
            0x0180, 0x024F,  // Latin Extended-B
            0x0370, 0x03FF,  // Greek and Coptic
            0x0400, 0x04FF,  // Cyrillic
            0x0500, 0x052F,  // Cyrillic Supplement
            0x0590, 0x05FF,  // Hebrew
            0x0600, 0x06FF,  // Arabic
            0x4E00, 0x9FFF,  // CJK Ideographs
            0x2000, 0x206F,  // General Punctuation
            0x3000, 0x30FF,  // CJK Symbols and Punctuation, Hiragana, Katakana
            0xFF00, 0xFFEF,  // Half-width and Full-width Forms
            0x1F600, 0x1F64F, // Emoticons (emoji range)
            0x1F300, 0x1F5FF, // Miscellaneous Symbols and Pictographs
            0x0000,           // terminator (important)
        ];

        let mut font_config = imgui::FontConfig::default();
        font_config.glyph_ranges = imgui::FontGlyphRanges::from_slice(GLYPH_RANGES);
        ctx.fonts().add_font(&[
            imgui::FontSource::TtfData {
                data: FONT_DATA,
                size_pixels: 16.0,
                config: Some(font_config),
            }
        ]);
        
        ctx.fonts().build_rgba32_texture();

        // Initialize command context
        self.command_context = Some(CommandContext::new(self.pid).unwrap());
        
        // Initialize AutoAccept with new pattern
        let mut auto_accept = AutoAccept::new();
        if let Some(ctx) = &mut self.command_context {
            auto_accept.init(ctx).unwrap();
        }
        self.auto_accept = Some(auto_accept);
        
        // Keep AutoLockin with old pattern for now
        self.auto_lockin = Some(AutoLockin::new(self.pid).unwrap());
        match GameCommon::new(self.pid) {
            Ok(mut game_common) => {
                if let Err(e) = game_common.game_common_apply(true) {
                    tracing::error!("Failed to apply game common: {}", e);
                } else {
                    tracing::info!("Successfully initialized game common");
                    
                    // Wrap game_common in Arc for shared ownership
                    let game_common = Arc::new(game_common);
                    
                    // Create lore window with shared game_common
                    let mut lore_window = LoreWindow::new();
                    lore_window.set_game_common(Arc::clone(&game_common));
                    lore_window.window_visible = false;
                    self.lore_window = Some(lore_window);
                    
                    // Store the same instance
                    self.game_common = Some(game_common);
                }
            }
            Err(e) => {
                tracing::error!("Failed to create game common: {}", e);
                self.lore_window = Some(LoreWindow::new());
            }
        }
        self.lobby_members = Some(LobbyMembers::new(self.pid).unwrap());
        self.building_window = Some(BuildingWindow::new());
        self.warband_window = Some(WarbandWindow::new());
    }

    fn render(&mut self, ui: &mut imgui::Ui) {
        if ui.is_key_down(Key::LeftCtrl) && 
           ui.is_key_down(Key::LeftShift) && 
           ui.is_key_pressed(Key::V) 
        {
            self.window_visible = !self.window_visible;
            tracing::info!("Window visibility toggled: {}", self.window_visible);
        }

        if self.window_visible {
            ui.window("Northgard Assistant")
                .size([300.0, 400.0], Condition::FirstUseEver)
                .position([16.0, 16.0], Condition::FirstUseEver)
                .build(|| {
                    let prev_state = self.checkbox_auto_accept;
                    if ui.checkbox("Enable Auto-Accept", &mut self.checkbox_auto_accept) {
                        if self.pid == 0 {
                            tracing::error!("Northgard process not found");
                            self.checkbox_auto_accept = false;
                            return;
                        }

                        if let Some(auto_accept) = &mut self.auto_accept {
                            if let Err(e) = auto_accept.auto_accept_apply(self.checkbox_auto_accept) {
                                tracing::error!("Auto-accept failed: {}", e);
                                self.checkbox_auto_accept = prev_state;
                            } else {
                                tracing::info!("Auto-accept {}", if self.checkbox_auto_accept { "enabled" } else { "disabled" });
                            }
                        }
                    }

                    ui.separator();

                    if let Some(auto_lockin) = &mut self.auto_lockin {
                        if let Some(clans) = auto_lockin.get_clans_game() {
                            let combo_text = self.selected_clan
                                .map_or("Lock in clan", |idx| clans[idx].as_str());
                                
                            let clans: Vec<String> = std::iter::once("Disabled".to_string())
                                .chain(clans.iter().cloned())
                                .collect();

                            if let Some(token) = ui.begin_combo("##clan_combo", combo_text) {
                                for (idx, clan) in clans.iter().enumerate() {
                                    let is_selected = if idx == 0 {
                                        self.selected_clan.is_none()
                                    } else {
                                        Some(idx - 1) == self.selected_clan
                                    };

                                    if ui.selectable_config(clan).selected(is_selected).build() {
                                        if idx == 0 {
                                            // Disable auto-lockin
                                            if let Some(prev_clan_idx) = self.selected_clan {
                                                let prev_clan = &clans[prev_clan_idx + 1];
                                                if let Err(e) = auto_lockin.auto_lockin_apply_clan(false, prev_clan) {
                                                    tracing::error!("Failed to disable auto-lockin: {}", e);
                                                }
                                            }
                                            self.selected_clan = None;
                                        } else {
                                            // Disable auto-lockin
                                            if let Some(prev_clan_idx) = self.selected_clan {
                                                let prev_clan = &clans[prev_clan_idx + 1];
                                                if let Err(e) = auto_lockin.auto_lockin_apply_clan(false, prev_clan) {
                                                    tracing::error!("Failed to disable auto-lockin: {}", e);
                                                }
                                            }
                                            // Enable auto-lockin with selected clan
                                            self.selected_clan = Some(idx - 1);
                                            if let Err(e) = auto_lockin.auto_lockin_apply_clan(true, clan) {
                                                tracing::error!("Failed to change clan: {}", e);
                                            }
                                        }
                                        break;
                                    }
                                }
                                token.end();
                            }
                        }
                    }

                    if let Some(auto_lockin) = &mut self.auto_lockin {
                        if let Some(colors) = auto_lockin.get_colors_game() {
                            let combo_text = self.selected_color
                                .map_or("Auto-select color", |idx| colors[idx]);
                                
                            let colors: Vec<String> = std::iter::once("Disabled".to_string())
                                .chain(colors.iter().map(|s| s.to_string()))
                                .collect();

                            if let Some(token) = ui.begin_combo("##color_combo", combo_text) {
                                for (idx, color) in colors.iter().enumerate() {
                                    let is_selected = if idx == 0 {
                                        self.selected_color.is_none()
                                    } else {
                                        Some(idx - 1) == self.selected_color
                                    };

                                    if ui.selectable_config(color).selected(is_selected).build() {
                                        if idx == 0 {
                                            // Disable auto-color
                                            if let Some(prev_color_idx) = self.selected_color {
                                                let prev_color = &colors[prev_color_idx + 1];
                                                if let Err(e) = auto_lockin.auto_lockin_apply_color(false, prev_color) {
                                                    tracing::error!("Failed to disable auto-color: {}", e);
                                                }
                                            }
                                            self.selected_color = None;
                                        } else {
                                            // Disable previous color
                                            if let Some(prev_color_idx) = self.selected_color {
                                                let prev_color = &colors[prev_color_idx + 1];
                                                if let Err(e) = auto_lockin.auto_lockin_apply_color(false, prev_color) {
                                                    tracing::error!("Failed to disable auto-color: {}", e);
                                                }
                                            }
                                            // Enable auto-color with selected color
                                            self.selected_color = Some(idx - 1);
                                            if let Err(e) = auto_lockin.auto_lockin_apply_color(true, color) {
                                                tracing::error!("Failed to change color: {}", e);
                                            }
                                        }
                                        break;
                                    }
                                }
                                token.end();
                            }
                        }
                    }

                    if let Some(lobby) = &self.lobby_members {
                        ui.separator();
                        
                        if ui.collapsing_header("Lobby Members", imgui::TreeNodeFlags::empty()) {
                            if ui.checkbox("Enable Lobby Members", &mut self.lobby_members_enabled) {
                                if let Err(e) = lobby.lobby_members_apply(self.lobby_members_enabled) {
                                    tracing::error!("Failed to toggle lobby members: {:?}", e);
                                    self.lobby_members_enabled = !self.lobby_members_enabled;
                                }
                            }

                            ui.child_window("members_list")
                                .size([0.0, 200.0])
                                .border(true)
                                .build(|| {
                                    if self.lobby_members_enabled {
                                        if let Some(lobby) = &self.lobby_members {
                                            let members = lobby.get_members();
                                            tracing::debug!("Current members: {:?}", members);
                                            if members.is_empty() {
                                                ui.text_disabled("No members in lobby");
                                            } else {
                                                for member in members {
                                                    ui.text_wrapped(&member);
                                                }
                                            }
                                        }
                                    } else {
                                        ui.text_disabled("Enable lobby members to see the list");
                                    }
                                });
                        }
                    }

                    if let Some(manager) = &self.build_guide_manager {
                        let guide_names = manager.get_guide_names();
                        let combo_text = self.selected_guide
                            .as_deref()
                            .unwrap_or("Select build guide");
                            
                        if let Some(token) = ui.begin_combo("Build Guide", combo_text) {
                            for guide_name in &guide_names {
                                let is_selected = Some(guide_name) == self.selected_guide.as_ref();
                                
                                if ui.selectable_config(guide_name)
                                    .selected(is_selected)
                                    .build() 
                                {
                                    self.selected_guide = Some(guide_name.clone());
                                    
                                    // If a guide is selected, update clan and lores
                                    if let Some(guide) = manager.get_guide(guide_name) {
                                        if let Some(auto_lockin) = &mut self.auto_lockin {
                                            // Set clan
                                            if let Some(clans) = auto_lockin.get_clans_game() {
                                                if let Some(clan_idx) = clans.iter().position(|c| c == &guide.clan) {
                                                    self.selected_clan = Some(clan_idx);
                                                    if let Err(e) = auto_lockin.auto_lockin_apply_clan(true, &guide.clan) {
                                                        tracing::error!("Failed to set clan: {}", e);
                                                    }
                                                }
                                            }
                                            
                                            // TODO: Add lore order display/tracking
                                        }
                                    }
                                }
                            }
                            token.end();
                        }
                        
                        // Display current guide info if selected
                        if let Some(guide_name) = &self.selected_guide {
                            if let Some(guide) = manager.get_guide(guide_name) {
                                ui.separator();
                                
                                if ui.collapsing_header("Guide Info", imgui::TreeNodeFlags::empty()) {
                                    // Display clan
                                    ui.text(format!("Clan: {}", guide.clan));
                                    
                                    // Display lore order
                                    if let Some(token) = ui.tree_node("Lore Order") {
                                        for (i, lore) in guide.lore_order.iter().enumerate() {
                                            ui.text(format!("{}. {}", i + 1, lore));
                                        }
                                        token.end();
                                    }
                                    
                                    // Display groups
                                    if let Some(token) = ui.tree_node("Groups") {
                                        for (year, group) in &guide.groups {
                                            if let Some(year_token) = ui.tree_node(year) {
                                                // Buildings
                                                if !group.buildings.is_empty() {
                                                    ui.text("Buildings:");
                                                    for building in &group.buildings {
                                                        ui.bullet_text(building);
                                                    }
                                                }
                                                
                                                // Units
                                                if !group.units.is_empty() {
                                                    ui.text("Units:");
                                                    for unit in &group.units {
                                                        ui.bullet_text(unit);
                                                    }
                                                }
                                                
                                                // Description
                                                if !group.description.is_empty() {
                                                    ui.text_wrapped(&group.description);
                                                }
                                                
                                                year_token.end();
                                            }
                                        }
                                        token.end();
                                    }
                                }
                            }
                        }
                    }
                });
        }

        if let Some(lore) = &mut self.lore_window {
            lore.render(ui);
        }

        if let Some(building) = &mut self.building_window {
            building.render(ui);
        }

        if let Some(warband) = &mut self.warband_window {
            warband.render(ui);
        }
    }
}
