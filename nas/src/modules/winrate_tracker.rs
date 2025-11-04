/*
    Win/Loss tracking overlay hooks template.

    Hook the following function to detect wins and losses:
    fn __constructor__@23326 (ui.win.EndGameScene, h3d.scene.Scene, bool, String, h2d.Object) -> void (27 regs, 169 ops)

    Hook the following function to figure out that the mode is 3v3:
    fn getTeamPlayerCount@7356 (GameState) -> i32 (12 regs, 30 ops)

    Hook the following function to obtain `GameState` for the function above:
    fn set_victory@7444 (GameState, ent.Player) -> ent.Player

    TODO: Use GameState::getPlayersByTeam and the following function to convert result to String@13:
    fn toString@126 (hl.types.ArrayObj) -> String (14 regs, 51 ops)
*/

use crate::modules::base::InjectionManager;
use crate::modules::mem_alloc::{MemoryAllocator, DataType};
use crate::modules::hashlink::*;
use iced_x86::code_asm::*;
use std::error::Error;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

const MAX_WINRATE_MEMORY_REGION_SIZE: usize = 0x2000;

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
#[repr(i32)]
enum EndGameKind {
    None = 0,
    Defeat,
    Victory, // defaultVictory
    Fame,
    Helheim,
    Faith,
    Lore,
    Mealsquirrel,
    Odinsword,
    Money,
    Owltitan,
    Yggdrasil,
}

pub struct WinrateTracker {
    pid: u32,
    enabled: bool,
    file_path: PathBuf,

    address_setvictory: usize,
    address_getteamplayercount: usize,

    // Those address are taken from the following function:
    // fn __constructor__@23326 (ui.win.EndGameScene, h3d.scene.Scene, bool, String, h2d.Object) -> void (27 regs, 169 ops)
    address_defeat: usize,
    address_defaultvictory: usize,
    address_escapebifrostvictory: usize,
    address_famevictory: usize,
    address_helheimvictory: usize,
    address_faithvictory: usize,
    address_lorevictory: usize,
    address_mealsquirrelvictory: usize,
    address_odinswordvictory: usize,
    address_moneyvictory: usize,
    address_owltitanvictory: usize,
    address_yggdrasilvictory: usize,

    injection_manager: InjectionManager,
    memory_allocator: MemoryAllocator,

    var_ptr_gamestate: usize,
    var_ptr_teamplayercount: usize,
    var_ptr_endgamekind: usize,

    var_ptr_callback_addr: usize,
    var_ptr_self_instance: usize,
}

impl WinrateTracker {
    extern "win64" fn update_winrate_callback(instance_ptr: *mut WinrateTracker, endgame_kind: EndGameKind) {
        let wrt = unsafe { &mut *instance_ptr };
        if endgame_kind == EndGameKind::None {
            return;
        }

        #[derive(Serialize, Deserialize, Default)]
        struct WinrateData {
            total_3v3: u32,
            wins: u32,
            losses: u32,
            by_victory: HashMap<String, u32>,
        }

        fn classify(kind: EndGameKind) -> (bool, Option<&'static str>) {
            if kind == EndGameKind::Defeat {
                return (false, None);
            }
            let reason = match kind {
                k if k == EndGameKind::Victory => Some("defaultVictory"),
                k if k == EndGameKind::Fame => Some("fameVictory"),
                k if k == EndGameKind::Helheim => Some("helheimVictory"),
                k if k == EndGameKind::Faith => Some("faithVictory"),
                k if k == EndGameKind::Lore => Some("loreVictory"),
                k if k == EndGameKind::Mealsquirrel => Some("mealSquirrelVictory"),
                k if k == EndGameKind::Odinsword => Some("odinSwordVictory"),
                k if k == EndGameKind::Money => Some("moneyVictory"),
                k if k == EndGameKind::Owltitan => Some("owlTitanVictory"),
                k if k == EndGameKind::Yggdrasil => Some("yggdrasilVictory"),
                _ => None,
            };
            (true, reason)
        }

        let (is_win, reason) = classify(endgame_kind);

        if let Some(parent) = wrt.file_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let mut data: WinrateData = match fs::read_to_string(&wrt.file_path) {
            Ok(content) if !content.trim().is_empty() => serde_json::from_str(&content).unwrap_or_default(),
            _ => WinrateData::default(),
        };

        data.total_3v3 = data.total_3v3.saturating_add(1);
        if is_win {
            data.wins = data.wins.saturating_add(1);
            if let Some(key) = reason {
                let entry = data.by_victory.entry(key.to_string()).or_insert(0);
                *entry = entry.saturating_add(1);
            }
        } else {
            data.losses = data.losses.saturating_add(1);
        }

        if let Ok(json) = serde_json::to_string_pretty(&data) {
            let _ = fs::write(&wrt.file_path, json);
        }

        let _ = wrt.memory_allocator.write_var("EndGameKind", EndGameKind::None as i32);
    }

    pub fn new(pid: u32) -> Result<Self, Box<dyn Error>> {
        let mut memory_allocator = MemoryAllocator::new(pid, MAX_WINRATE_MEMORY_REGION_SIZE)?;

        let var_ptr_gamestate = memory_allocator.allocate_var("GameState", DataType::Pointer)?;
        let var_ptr_teamplayercount = memory_allocator.allocate_var("TeamPlayerCount", DataType::I32)?;
        let var_ptr_endgamekind = memory_allocator.allocate_var("EndGameKind", DataType::I32)?;

        let var_ptr_callback_addr = memory_allocator.allocate_var("WinrateTrackerCallback", DataType::Pointer)?;
        let var_ptr_self_instance = memory_allocator.allocate_var("WinrateTrackerInstance", DataType::Pointer)?;
        
        memory_allocator.write_var("GameState", 0usize)?;
        memory_allocator.write_var("TeamPlayerCount", 0i32)?;
        memory_allocator.write_var("EndGameKind", 0i32)?;

        let mut injection_manager = InjectionManager::new(pid);
        injection_manager.add_injection("set_victory".to_string());
        injection_manager.add_injection("get_teamplayercount".to_string());
        injection_manager.add_injection("defeat".to_string());
        injection_manager.add_injection("default_victory".to_string());
        injection_manager.add_injection("escape_bifrost_victory".to_string());
        injection_manager.add_injection("fame_victory".to_string());
        injection_manager.add_injection("helheim_victory".to_string());
        injection_manager.add_injection("faith_victory".to_string());
        injection_manager.add_injection("lore_victory".to_string());
        injection_manager.add_injection("mealsquirrel_victory".to_string());
        injection_manager.add_injection("odinsword_victory".to_string());
        injection_manager.add_injection("money_victory".to_string());
        injection_manager.add_injection("owltitan_victory".to_string());
        injection_manager.add_injection("yggdrasil_victory".to_string());

        let file_path: PathBuf = if let Ok(pd) = std::env::var("PROGRAMDATA") {
            PathBuf::from(pd).join("northgard-assistant").join("winrate.json")
        } else {
            let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
            base.join("northgard-assistant").join("winrate.json")
        };

        let mut winrate_tracker = Self {
            pid,
            enabled: false,
            file_path,

            address_setvictory: 0,
            address_getteamplayercount: 0,

            address_defeat: 0,
            address_defaultvictory: 0,
            address_escapebifrostvictory: 0,
            address_famevictory: 0,
            address_helheimvictory: 0,
            address_faithvictory: 0,
            address_lorevictory: 0,
            address_mealsquirrelvictory: 0,
            address_odinswordvictory: 0,
            address_moneyvictory: 0,
            address_owltitanvictory: 0,
            address_yggdrasilvictory: 0,

            injection_manager,
            memory_allocator,

            var_ptr_gamestate,
            var_ptr_teamplayercount,
            var_ptr_endgamekind,

            var_ptr_callback_addr,
            var_ptr_self_instance,
        };
        
        winrate_tracker.init()?;
        winrate_tracker.setup_callback_infrastructure()?;

        Ok(winrate_tracker)
    }

    pub fn init(&mut self) -> Result<(), Box<dyn Error>> {
        let guard = Hashlink::instance(self.pid).lock().unwrap();
        if let Some(hashlink) = guard.as_ref() {
            self.address_setvictory = hashlink.get_function_address("set_victory", Some(0))?;
            self.address_getteamplayercount = hashlink.get_function_address("getTeamPlayerCount", Some(0))?;
            self.address_defeat = hashlink.get_function_address("defeat", Some(0))?;
            self.address_defaultvictory = hashlink.get_function_address("defaultVictory", Some(0))?;
            self.address_escapebifrostvictory = hashlink.get_function_address("escapeBifrostVictory", Some(0))?;
            self.address_famevictory = hashlink.get_function_address("fameVictory", Some(0))?;
            self.address_helheimvictory = hashlink.get_function_address("helheimVictory", Some(0))?;
            self.address_faithvictory = hashlink.get_function_address("faithVictory", Some(0))?;
            self.address_lorevictory = hashlink.get_function_address("loreVictory", Some(0))?;
            self.address_mealsquirrelvictory = hashlink.get_function_address("mealSquirrelVictory", Some(0))?;
            self.address_odinswordvictory = hashlink.get_function_address("odinSwordVictory", Some(0))?;
            self.address_moneyvictory = hashlink.get_function_address("moneyVictory", Some(0))?;
            self.address_owltitanvictory = hashlink.get_function_address("owlTitanVictory", Some(0))?;
            self.address_yggdrasilvictory = hashlink.get_function_address("yggdrasilVictory", Some(0))?;
        }
 
        Ok(())
    }

    fn setup_callback_infrastructure(&mut self) -> Result<(), Box<dyn Error>> {
        let callback_addr = Self::update_winrate_callback as *const () as usize;
        self.memory_allocator.write_var("WinrateTrackerCallback", callback_addr)?;

        let self_ptr = self as *const Self as usize;
        self.memory_allocator.write_var("WinrateTrackerInstance", self_ptr)?;

        Ok(())
    }

    fn create_endgame_code(&mut self, kind: EndGameKind) -> Result<CodeAssembler, Box<dyn Error>> {
        let mut code = CodeAssembler::new(64)?;

        code.pushfq()?;
        code.push(rax)?;
        code.push(rbx)?;

        code.mov(rbx, self.var_ptr_endgamekind as u64)?;
        code.mov(dword_ptr(rbx), kind as u32)?;

        code.pop(rbx)?;
        code.pop(rax)?;
        code.popfq()?;
        
        Ok(code)
    }

    pub fn apply(&mut self, enable: bool) -> Result<(), Box<dyn Error>> {
        if enable {
            /*
            Injection: set_victory
                1. Save `GameState` value
                2. Call getTeamPlayerCount
            */
            let mut code = CodeAssembler::new(64)?;
            let mut label_exit = code.create_label();
            
            code.pushfq()?;
            code.push(rax)?;
            code.push(rbx)?;
            code.push(rcx)?;
            code.push(rdx)?;

            code.mov(rbx, self.var_ptr_gamestate as u64)?;
            code.mov(qword_ptr(rbx), rcx)?;
            code.mov(rax, self.address_getteamplayercount as u64)?;
            code.call(rax)?;
            
            code.mov(rbx, self.var_ptr_teamplayercount as u64)?;
            code.mov(dword_ptr(rbx), eax)?;
            
            code.cmp(eax, 3)?; // We need to track only 3v3 games
            code.jne(label_exit)?;

            code.mov(rbx, self.var_ptr_self_instance as u64)?;
            code.mov(rcx, qword_ptr(rbx))?;
            code.mov(rbx, self.var_ptr_endgamekind as u64)?;
            code.mov(edx, dword_ptr(rbx))?;
            code.mov(rbx, self.var_ptr_callback_addr as u64)?;
            code.mov(rax, qword_ptr(rbx))?;
            code.call(rax)?;

            code.set_label(&mut label_exit)?;
            code.pop(rdx)?;
            code.pop(rcx)?;
            code.pop(rbx)?;
            code.pop(rax)?;
            code.popfq()?;

            self.injection_manager.apply_injection("set_victory", self.address_setvictory, &mut code)?;

            /*
            Injection: EndGameScene
                1. Save result of game to `var_ptr_endgamekind`
            */
            let mut code = self.create_endgame_code(EndGameKind::Defeat)?;
            self.injection_manager.apply_injection("defeat", self.address_defeat, &mut code)?;

            let mut code = self.create_endgame_code(EndGameKind::Victory)?;
            self.injection_manager.apply_injection("default_victory", self.address_defaultvictory, &mut code)?;

            let mut code = self.create_endgame_code(EndGameKind::Fame)?;
            self.injection_manager.apply_injection("fame_victory", self.address_famevictory, &mut code)?;

            let mut code = self.create_endgame_code(EndGameKind::Helheim)?;
            self.injection_manager.apply_injection("helheim_victory", self.address_helheimvictory, &mut code)?;

            let mut code = self.create_endgame_code(EndGameKind::Faith)?;
            self.injection_manager.apply_injection("faith_victory", self.address_faithvictory, &mut code)?;

            let mut code = self.create_endgame_code(EndGameKind::Lore)?;
            self.injection_manager.apply_injection("lore_victory", self.address_lorevictory, &mut code)?;

            let mut code = self.create_endgame_code(EndGameKind::Mealsquirrel)?;
            self.injection_manager.apply_injection("mealsquirrel_victory", self.address_mealsquirrelvictory, &mut code)?;

            let mut code = self.create_endgame_code(EndGameKind::Odinsword)?;
            self.injection_manager.apply_injection("odinsword_victory", self.address_odinswordvictory, &mut code)?;

            let mut code = self.create_endgame_code(EndGameKind::Money)?;
            self.injection_manager.apply_injection("money_victory", self.address_moneyvictory, &mut code)?;

            let mut code = self.create_endgame_code(EndGameKind::Owltitan)?;
            self.injection_manager.apply_injection("owltitan_victory", self.address_owltitanvictory, &mut code)?;

            let mut code = self.create_endgame_code(EndGameKind::Yggdrasil)?;
            self.injection_manager.apply_injection("yggdrasil_victory", self.address_yggdrasilvictory, &mut code)?;
        } else {
            self.injection_manager.remove_injection("set_victory")?;
            self.injection_manager.remove_injection("get_teamplayercount")?;
            self.injection_manager.remove_injection("defeat")?;
            self.injection_manager.remove_injection("default_victory")?;
            self.injection_manager.remove_injection("escape_bifrost_victory")?;
            self.injection_manager.remove_injection("fame_victory")?;
            self.injection_manager.remove_injection("helheim_victory")?;
            self.injection_manager.remove_injection("faith_victory")?;
            self.injection_manager.remove_injection("lore_victory")?;
            self.injection_manager.remove_injection("mealsquirrel_victory")?;
            self.injection_manager.remove_injection("odinsword_victory")?;
            self.injection_manager.remove_injection("money_victory")?;
            self.injection_manager.remove_injection("owltitan_victory")?;
            self.injection_manager.remove_injection("yggdrasil_victory")?;
        }

        self.enabled = enable;
        Ok(())
    }
}