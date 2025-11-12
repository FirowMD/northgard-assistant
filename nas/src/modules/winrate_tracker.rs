use crate::modules::base::InjectionManager;
use crate::modules::mem_alloc::{MemoryAllocator, DataType};
use crate::modules::hashlink::*;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use iced_x86::code_asm::*;
use std::error::Error;
use std::path::PathBuf;

const MAX_WINRATE_MEMORY_REGION_SIZE: usize = 0x2000;

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
#[repr(i32)]
pub enum EndGameKind {
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

impl EndGameKind {
    #[inline]
    fn from_u32(value: u32) -> EndGameKind {
        match value {
            0 => EndGameKind::None,
            1 => EndGameKind::Defeat,
            2 => EndGameKind::Victory,
            3 => EndGameKind::Fame,
            4 => EndGameKind::Helheim,
            5 => EndGameKind::Faith,
            6 => EndGameKind::Lore,
            7 => EndGameKind::Mealsquirrel,
            8 => EndGameKind::Odinsword,
            9 => EndGameKind::Money,
            10 => EndGameKind::Owltitan,
            11 => EndGameKind::Yggdrasil,
            _ => EndGameKind::None,
        }
    }
}

pub struct EndGameEvent {
    pub kind: EndGameKind,
}

pub struct WinrateTracker {
    pid: u32,
    enabled: bool,
    file_path: PathBuf,

    address_ui_win_EndGame_init: usize,
    address_getteamplayercount: usize,

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
    var_ptr_callback: usize, // `update_winrate_callback` address
}

static ENDGAME_PENDING: AtomicBool = AtomicBool::new(false);
static ENDGAME_KIND_ATOMIC: AtomicU32 = AtomicU32::new(0);

extern "C" fn update_winrate_callback(endgame_kind: u32) {
    ENDGAME_KIND_ATOMIC.store(endgame_kind, Ordering::Relaxed);
    ENDGAME_PENDING.store(true, Ordering::Release);
}

pub fn take_pending_endgame() -> Option<EndGameEvent> {
    if ENDGAME_PENDING.swap(false, Ordering::Acquire) {
        let val = ENDGAME_KIND_ATOMIC.load(Ordering::Relaxed);
        Some(EndGameEvent { kind: EndGameKind::from_u32(val) })
    } else {
        None
    }
}

impl WinrateTracker {
    fn classify(kind: EndGameKind) -> (bool, Option<&'static str>) {
        if kind == EndGameKind::Defeat { return (false, None); }
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

    pub fn new(pid: u32) -> Result<Self, Box<dyn Error>> {
        let mut memory_allocator = MemoryAllocator::new(pid, MAX_WINRATE_MEMORY_REGION_SIZE)?;

        let var_ptr_gamestate = memory_allocator.allocate_var("GameState", DataType::Pointer)?;
        let var_ptr_teamplayercount = memory_allocator.allocate_var("TeamPlayerCount", DataType::I32)?;
        let var_ptr_endgamekind = memory_allocator.allocate_var("EndGameKind", DataType::I32)?;

        let var_ptr_callback = memory_allocator.allocate_var("UpdateWinrateCallback", DataType::Pointer)?;
        
        memory_allocator.write_var("GameState", 0usize)?;
        memory_allocator.write_var("TeamPlayerCount", 0i32)?;
        memory_allocator.write_var("EndGameKind", 0i32)?;

        memory_allocator.write_var("UpdateWinrateCallback", update_winrate_callback as usize)?;

        let mut injection_manager = InjectionManager::new(pid);
        injection_manager.add_injection("ui_win_EndGame_init".to_string());
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
            PathBuf::from(pd).join("northgard-tracker").join("winrate.json")
        } else {
            let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
            base.join("northgard-tracker").join("winrate.json")
        };

        let mut winrate_tracker = Self {
            pid,
            enabled: false,
            file_path,

            address_ui_win_EndGame_init: 0,
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
            var_ptr_callback,
        };
        
        winrate_tracker.init()?;
        Ok(winrate_tracker)
    }

    pub fn init(&mut self) -> Result<(), Box<dyn Error>> {

        let guard = Hashlink::instance(self.pid).lock().unwrap();
        if let Some(hashlink) = guard.as_ref() {
            // xor r11,r11 <- trampoline
            // mov [rbp-60],r11
            // mov rcx,r10
            // mov rdx,r11
            // sub rsp,20
            // call 76CA9F329430
            const INIT_OFFSET: usize = 1017;
            const INIT_INDEX: usize = 450; // magic!
            self.address_ui_win_EndGame_init = hashlink.get_function_address("init", Some(INIT_INDEX))? + INIT_OFFSET;

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

    fn create_endgame_code(&mut self, kind: EndGameKind) -> Result<CodeAssembler, Box<dyn Error>> {
        let mut code = CodeAssembler::new(64)?;
        let mut label_exit = code.create_label();

        code.pushfq()?;
        code.push(rax)?;
        code.push(rbx)?;
        code.push(rcx)?;
        code.push(rdx)?;
        code.push(r8)?;
        code.push(r9)?;
        code.push(r10)?;
        code.push(r11)?;

        // Dynamically align stack to 16 bytes and persist the adjustment amount
        code.mov(rax, rsp)?;
        code.and(rax, 8)?;            // rax = 8 if misaligned (rsp % 16 == 8), else 0
        code.sub(rsp, rax)?;          // apply alignment fix
        code.sub(rsp, 8)?;            // reserve 8 bytes to store the adjustment
        code.mov(qword_ptr(rsp), rax)?; // store the adjustment value on stack

        code.sub(rsp, 0x60)?;
        code.movups(oword_ptr(rsp), xmm0)?;
        code.movups(oword_ptr(rsp + 0x10), xmm1)?;
        code.movups(oword_ptr(rsp + 0x20), xmm2)?;
        code.movups(oword_ptr(rsp + 0x30), xmm3)?;
        code.movups(oword_ptr(rsp + 0x40), xmm4)?;
        code.movups(oword_ptr(rsp + 0x50), xmm5)?;

        code.mov(rbx, self.var_ptr_teamplayercount as u64)?;
        code.mov(eax, dword_ptr(rbx))?;

        code.cmp(eax, 3)?; // We need to track only 3v3 games
        code.jne(label_exit)?;

        code.mov(rbx, self.var_ptr_endgamekind as u64)?;
        code.mov(dword_ptr(rbx), kind as u32)?;

        code.mov(ecx, kind as i32)?;
        code.mov(rbx, self.var_ptr_callback as u64)?;
        code.mov(rax, qword_ptr(rbx))?;
        code.sub(rsp, 0x120)?;
        code.call(rax)?;
        code.add(rsp, 0x120)?;

        code.set_label(&mut label_exit)?;

        code.movups(xmm0, oword_ptr(rsp))?;
        code.movups(xmm1, oword_ptr(rsp + 0x10))?;
        code.movups(xmm2, oword_ptr(rsp + 0x20))?;
        code.movups(xmm3, oword_ptr(rsp + 0x30))?;
        code.movups(xmm4, oword_ptr(rsp + 0x40))?;
        code.movups(xmm5, oword_ptr(rsp + 0x50))?;
        code.add(rsp, 0x60)?;

        // Restore dynamic alignment
        code.mov(rax, qword_ptr(rsp))?; // load the previously stored adjustment
        code.add(rsp, 8)?;              // remove the storage slot
        code.add(rsp, rax)?;            // undo the alignment fix

        code.pop(r11)?;
        code.pop(r10)?;
        code.pop(r9)?;
        code.pop(r8)?;
        code.pop(rdx)?;
        code.pop(rcx)?;
        code.pop(rbx)?;
        code.pop(rax)?;
        code.popfq()?;
        
        Ok(code)
    }

    pub fn apply(&mut self, enable: bool) -> Result<(), Box<dyn Error>> {
        if enable {
            // Injection: ui_win_EndGame_init
            let mut code = CodeAssembler::new(64)?;
            
            code.pushfq()?;
            code.push(rax)?;
            code.push(rbx)?;
            code.push(rcx)?;
            code.push(rdx)?;
            code.push(r8)?;
            code.push(r9)?;
            code.push(r10)?;
            code.push(r11)?;

            // Dynamically align stack to 16 bytes and persist the adjustment amount
            code.mov(rax, rsp)?;
            code.and(rax, 8)?;            // rax = 8 if misaligned (rsp % 16 == 8), else 0
            code.sub(rsp, rax)?;          // apply alignment fix
            code.sub(rsp, 8)?;            // reserve 8 bytes to store the adjustment
            code.mov(qword_ptr(rsp), rax)?; // store the adjustment value on stack

            code.sub(rsp, 0x60)?;
            code.movups(oword_ptr(rsp), xmm0)?;
            code.movups(oword_ptr(rsp + 0x10), xmm1)?;
            code.movups(oword_ptr(rsp + 0x20), xmm2)?;
            code.movups(oword_ptr(rsp + 0x30), xmm3)?;
            code.movups(oword_ptr(rsp + 0x40), xmm4)?;
            code.movups(oword_ptr(rsp + 0x50), xmm5)?;

            code.mov(rcx, r10)?; // we are at center of the function, r10 is the first argument

            code.mov(rbx, self.var_ptr_gamestate as u64)?;
            code.mov(qword_ptr(rbx), rcx)?;
            code.mov(rax, self.address_getteamplayercount as u64)?;
            code.sub(rsp, 0x20)?;
            code.call(rax)?;
            code.add(rsp, 0x20)?;
            
            code.mov(rbx, self.var_ptr_teamplayercount as u64)?;
            code.mov(dword_ptr(rbx), eax)?;

            code.movups(xmm0, oword_ptr(rsp))?;
            code.movups(xmm1, oword_ptr(rsp + 0x10))?;
            code.movups(xmm2, oword_ptr(rsp + 0x20))?;
            code.movups(xmm3, oword_ptr(rsp + 0x30))?;
            code.movups(xmm4, oword_ptr(rsp + 0x40))?;
            code.movups(xmm5, oword_ptr(rsp + 0x50))?;
            code.add(rsp, 0x60)?;

            // Restore dynamic alignment
            code.mov(rax, qword_ptr(rsp))?; // load the previously stored adjustment
            code.add(rsp, 8)?;              // remove the storage slot
            code.add(rsp, rax)?;            // undo the alignment fix

            code.pop(r11)?;
            code.pop(r10)?;
            code.pop(r9)?;
            code.pop(r8)?;
            code.pop(rdx)?;
            code.pop(rcx)?;
            code.pop(rbx)?;
            code.pop(rax)?;
            code.popfq()?;

            self.injection_manager.apply_injection("ui_win_EndGame_init", self.address_ui_win_EndGame_init, &mut code)?;

            // Injection: EndGameScene
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
            self.injection_manager.remove_injection("ui_win_EndGame_init")?;
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

    pub fn free(&self) -> Result<(), Box<dyn Error>> {
        self.memory_allocator.free()
    }
}