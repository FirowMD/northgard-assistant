use crate::modules::base::InjectionManager;
use crate::modules::mem_alloc::*;
use crate::modules::hashlink::*;
use iced_x86::code_asm::*;
use std::error::Error;

pub struct AutoLockin {
    pid: u32, // Keep pid for now since this struct has complex memory management
    clan_current: Option<usize>,
    clan_array: Option<Vec<String>>,
    color_current: Option<usize>,
    color_array: Option<Vec<String>>,
    
    clan_enabled: bool,
    color_enabled: bool,
    clan_name: Option<String>,
    
    // Function addresses
    address_allocstring: usize,
    address_parseint: usize,
    address_changemyclan: usize,
    address_changemycolor: usize,
    address_canready: usize,
    address_canready_end: usize,
    address_clanunlockedbydlc: usize,
    
    // Variable pointers
    var_ptr_lobbymanager: usize,
    var_ptr_clan: usize,
    var_ptr_arrayclans: Vec<usize>,
    var_ptr_arraycolors: Vec<usize>,
    var_ptr_lockedin: usize,
    var_ptr_color: usize,
    var_ptr_color_int: usize,

    injection_manager: InjectionManager,
    mem_allocator: MemoryAllocator,
}

impl AutoLockin {
    pub fn new(pid: u32) -> Result<Self, Box<dyn Error>> {
        let mut memory_allocator = MemoryAllocator::new(pid, 0x1000)?;
        let var_ptr_clan_tmp = memory_allocator.allocate_var("ClanString", DataType::Pointer)?;
        let var_ptr_color_tmp = memory_allocator.allocate_var("ColorString", DataType::Pointer)?;
        let var_ptr_color_int_tmp = memory_allocator.allocate_var("ColorInt", DataType::Pointer)?;
        let var_ptr_lobbymanager_tmp = memory_allocator.allocate_var("gamesys.LobbyManager", DataType::Pointer)?;
        let var_ptr_lockedin_tmp = memory_allocator.allocate_var("gamesys.LobbyManager.LockedIn", DataType::Pointer)?;

        let mut injection_manager = InjectionManager::new(pid);
        injection_manager.add_injection("canready".to_string());
        injection_manager.add_injection("canready_end".to_string());

        let mut auto_lockin = Self {
            pid,
            clan_current: None,
            clan_array: None,
            color_current: None,
            color_array: None,
            clan_enabled: false,
            color_enabled: false,
            clan_name: None,
            address_allocstring: 0,
            address_parseint: 0,
            address_changemyclan: 0,
            address_changemycolor: 0,
            address_canready: 0,
            address_canready_end: 0,
            address_clanunlockedbydlc: 0,
            var_ptr_lobbymanager: var_ptr_lobbymanager_tmp,
            var_ptr_clan: var_ptr_clan_tmp,
            var_ptr_lockedin: var_ptr_lockedin_tmp,
            var_ptr_arrayclans: vec![],
            var_ptr_arraycolors: vec![],
            var_ptr_color: var_ptr_color_tmp,
            var_ptr_color_int: var_ptr_color_int_tmp,
            injection_manager,
            mem_allocator: memory_allocator,
        };

        auto_lockin.init_auto_lockin()?;
        auto_lockin.init_clans()?;
        auto_lockin.init_colors()?;

        Ok(auto_lockin)
    }

    pub fn init_auto_lockin(&mut self) -> Result<(), Box<dyn Error>> {
        const OFFSET_CANREADY_END: usize = 182;

        let guard = Hashlink::instance(self.pid).lock().unwrap();
        if let Some(hashlink) = guard.as_ref() {
            self.address_allocstring = hashlink.get_function_address("__alloc__", Some(0))?;
            self.address_parseint = hashlink.get_function_address("parseInt", Some(0))?;
            self.address_changemyclan = hashlink.get_function_address("changeMyClan", Some(0))?;
            self.address_changemycolor = hashlink.get_function_address("changeMyColor", Some(0))?;
            self.address_canready = hashlink.get_function_address("canReady", Some(0))?;
            self.address_canready_end = self.address_canready + OFFSET_CANREADY_END;
            self.address_clanunlockedbydlc = hashlink.get_function_address("clanUnlockedByDLC", Some(0))?;
        } else {
            return Err("Hashlink not initialized".into());
        }

        Ok(())
    }
    
    pub fn init_clans(&mut self) -> Result<(), Box<dyn Error>> {
        let clans = vec!["Bear", "Boar", "Dragon", "Eagle", "Pack", "Goat", "Hippogriff", "Horse", "Kraken", "Lion", "Lynx", "Owl", "Ox", "Rat", "Raven", "Snake", "Squirrel", "Stag", "Stoat", "Turtle", "Wolf"];

        self.clan_array = Some(clans.iter().map(|s| s.to_string()).collect());

        self.var_ptr_arrayclans.clear();

        for clan in clans {
            self.var_ptr_arrayclans.push(self.mem_allocator.allocate_wide_string(clan, clan)?);
        }

        Ok(())
    }
    
    pub fn init_colors(&mut self) -> Result<(), Box<dyn Error>> {
        let colors = vec!["0", "1", "2", "3", "4", "5", "6", "7"];

        self.color_array = Some(colors.iter().map(|s| s.to_string()).collect());

        self.var_ptr_arraycolors.clear();

        for color in colors {
            self.var_ptr_arraycolors.push(self.mem_allocator.allocate_wide_string(color, color)?);
        }

        Ok(())
    }

    pub fn change_color(&mut self, color: &str) -> Result<(), Box<dyn Error>> {
        let color_id = match color {
            "Red" => 0,
            "Blue" => 1,
            "Yellow" => 2,
            "Green" => 3,
            "Purple" => 4,
            "Brown" => 5,
            "Orange" => 6,
            "Navy" => 7,
            _ => 0, // `Red` is default color
        };

        self.color_current = Some(color_id);

        Ok(())
    }

    pub fn change_clan(&mut self, clan: &str) -> Result<(), Box<dyn Error>> {
        // `Garm` clan is external name for `Pack` clan
        // So we need to convert `Pack` to `Garm`
        let clan_id = match clan {
            "Bear" => 0,
            "Boar" => 1,
            "Dragon" => 2,
            "Eagle" => 3,
            "Garm" => 4,
            "Goat" => 5,
            "Hippogriff" => 6,
            "Horse" => 7,
            "Kraken" => 8,
            "Lion" => 9,
            "Lynx" => 10,
            "Owl" => 11,
            "Ox" => 12,
            "Rat" => 13,
            "Raven" => 14,
            "Snake" => 15,
            "Squirrel" => 16,
            "Stag" => 17,
            "Stoat" => 18,
            "Turtle" => 19,
            "Wolf" => 20,
            _ => 5, // `Goat` is default clan
        };

        self.clan_current = Some(clan_id);

        Ok(())
    }

    pub fn auto_lockin_apply_clan(&mut self, enable: bool, clan_str: &str) -> Result<(), Box<dyn Error>> {
        self.clan_enabled = enable;
        self.change_clan(clan_str)?;
        self.clan_name = Some(clan_str.to_string());
        self.auto_lockin_apply()?;
        Ok(())
    }

    pub fn auto_lockin_apply_color(&mut self, enable: bool, color_str: &str) -> Result<(), Box<dyn Error>> {
        self.color_enabled = enable;
        self.change_color(color_str)?;
        self.auto_lockin_apply()?;
        Ok(())
    }

    pub fn auto_lockin_apply(&mut self) -> Result<(), Box<dyn Error>> {
        // Remove existing injections
        self.injection_manager.remove_injection("canready")?;
        self.injection_manager.remove_injection("canready_end")?;

        // Apply canready injection
        let mut code = CodeAssembler::new(64)?;
        code.push(rax)?;
        code.push(rcx)?;
        code.push(rdx)?;

        // Save `gamesys.LobbyManager` to `var_ptr_lobbymanager`
        code.mov(rax, self.var_ptr_lobbymanager as u64)?;
        code.mov(qword_ptr(rax), rcx)?;

        code.pop(rdx)?;
        code.pop(rcx)?;
        code.pop(rax)?;

        self.injection_manager.apply_injection("canready", self.address_canready, &mut code)?;

        // Apply canready_end injection
        let mut code_end = CodeAssembler::new(64)?;
        let mut label_end = code_end.create_label();

        code_end.pushfq()?;
        code_end.push(rax)?;
        code_end.push(rcx)?;
        code_end.push(rdx)?;

        // Compare `var_ptr_lockedin` and `var_ptr_lobbymanager`
        code_end.mov(rax, self.var_ptr_lockedin as u64)?;
        code_end.mov(rcx, qword_ptr(rax))?;
        code_end.mov(rax, self.var_ptr_lobbymanager as u64)?;
        code_end.mov(rdx, qword_ptr(rax))?;
        code_end.cmp(rcx, rdx)?;
        code_end.je(label_end)?;

            if self.clan_enabled {
                let clan_addr = self.var_ptr_arrayclans[self.clan_current.unwrap()] as u64;
                let clan_len = self.clan_array.as_ref()
                    .unwrap()[self.clan_current.unwrap()].len() as u64;

                // Call `allocString`
                code_end.mov(rcx, clan_addr)?;
                code_end.mov(rdx, clan_len)?;
                code_end.mov(rax, self.address_allocstring as u64)?;
                code_end.call(rax)?;

                // Save `allocString` result to `var_ptr_clan`
                code_end.mov(rcx, self.var_ptr_clan as u64)?;
                code_end.mov(qword_ptr(rcx), rax)?;

                // Do not check if base clan
                if !self.get_base_clans().contains(&self.clan_name.as_ref().unwrap().as_str()) {
                    // Call `clanUnlockedByDLC`
                    code_end.mov(rax, self.var_ptr_clan as u64)?;
                    code_end.mov(rcx, qword_ptr(rax))?;
                    code_end.mov(rax, self.address_clanunlockedbydlc as u64)?;
                    code_end.call(rax)?;

                    // Compare result of `clanUnlockedByDLC` and 0
                    code_end.cmp(rax, 0)?;
                    code_end.je(label_end)?;
                }

                // Call `changeMyClan`
                code_end.mov(rax, self.var_ptr_clan as u64)?;
                code_end.mov(rdx, qword_ptr(rax))?;
                code_end.mov(rax, self.var_ptr_lobbymanager as u64)?;
                code_end.mov(rcx, qword_ptr(rax))?;
                code_end.mov(rax, self.address_changemyclan as u64)?;
                code_end.call(rax)?;
            }

            if self.color_enabled {
                let color_addr = self.var_ptr_arraycolors[self.color_current.unwrap()] as u64;
                let color_len = self.color_array.as_ref()
                    .unwrap()[self.color_current.unwrap()].len() as u64;
                
                // Call `allocString`
                code_end.mov(rcx, color_addr)?;
                code_end.mov(rdx, color_len)?;
                code_end.mov(rax, self.address_allocstring as u64)?;
                code_end.call(rax)?;

                // Save `allocString` result to `var_ptr_color`
                code_end.mov(rcx, self.var_ptr_color as u64)?;
                code_end.mov(qword_ptr(rcx), rax)?;

                // Call `parseInt`
                code_end.mov(rax, self.var_ptr_color as u64)?;
                code_end.mov(rcx, qword_ptr(rax))?;
                code_end.mov(rax, self.address_parseint as u64)?;
                code_end.call(rax)?;

                // Save `parseInt` result to `var_ptr_color_int`
                code_end.mov(rcx, self.var_ptr_color_int as u64)?;
                code_end.mov(qword_ptr(rcx), rax)?;

                // Call `changeMyColor`
                code_end.mov(rax, self.var_ptr_color_int as u64)?;
                code_end.mov(rdx, qword_ptr(rax))?;
                code_end.mov(rax, self.var_ptr_lobbymanager as u64)?;
                code_end.mov(rcx, qword_ptr(rax))?;
                code_end.mov(rax, self.address_changemycolor as u64)?;
                code_end.call(rax)?;
            }

        code_end.set_label(&mut label_end)?;
        code_end.pop(rdx)?;
        code_end.pop(rcx)?;
        code_end.pop(rax)?;
        code_end.popfq()?;

        self.injection_manager.apply_injection("canready_end", self.address_canready_end, &mut code_end)?;

        Ok(())
    }

    pub fn get_base_clans(&self) -> Vec<&str> {
        vec!["Stag", "Goat", "Wolf", "Raven", "Bear", "Boar"]
    }

    pub fn get_clans(&self) -> Option<&Vec<String>> {
        self.clan_array.as_ref()
    }

    // same as `get_clans`, but `Pack` is `Garm`
    pub fn get_clans_game(&self) -> Option<Vec<String>> {
        Some(self.get_clans()?
            .iter()
            .map(|s| if s == "Pack" { "Garm" } else { s })
            .map(String::from)
            .collect())
    }

    pub fn get_colors_game(&self) -> Option<Vec<&str>> {
        Some(vec!["Red", "Blue", "Yellow", "Green", "Purple", "Brown", "Orange", "Navy"])
    }

}