use crate::commands::basic::*;
use crate::commands::aob_injection::AobInjection;
use crate::commands::mem_alloc::*;
use crate::commands::hashlink::*;
use iced_x86::code_asm::*;
use std::error::Error;
use std::sync::Mutex;

#[allow(dead_code)]
pub struct GameCommon {
    pid: u32,

    // fn get_width@590 (hxd.Window) -> i32 (2 regs, 2 ops)
    address_getwidth: usize,
    // fn get_height@591 (hxd.Window) -> i32 (2 regs, 2 ops)
    address_getheight: usize,

    // i32
    var_ptr_winwidth: usize,
    // i32
    var_ptr_winheight: usize,

    injection_getwidth: Mutex<Option<AobInjection>>,
    injection_getheight: Mutex<Option<AobInjection>>,

    mem_allocator: MemoryAllocator,
}

impl GameCommon {
    pub fn new(pid: u32) -> Result<Self, Box<dyn Error>> {
        let mut memory_allocator = MemoryAllocator::new(pid, 0x1000)?;
        let var_ptr_windowwidth = memory_allocator.allocate_var("WindowWidth", DataType::U32)?;
        let var_ptr_windowheight = memory_allocator.allocate_var("WindowHeight", DataType::U32)?;

        let mut game_common = Self {
            pid,
            var_ptr_winwidth: var_ptr_windowwidth,
            var_ptr_winheight: var_ptr_windowheight,
            address_getwidth: 0,
            address_getheight: 0,
            injection_getwidth: Mutex::new(None),
            injection_getheight: Mutex::new(None),
            mem_allocator: memory_allocator,
        };

        game_common.init_game_common()?;

        Ok(game_common)
    }

    pub fn init_game_common(&mut self) -> Result<(), Box<dyn Error>> {
        const OFFSET_GETWIDTH_END: usize = 18;
        const OFFSET_GETHEIGHT_END: usize = 18;

        let guard = Hashlink::instance(self.pid).lock().unwrap();
        if let Some(hashlink) = guard.as_ref() {
            self.address_getwidth = hashlink.get_function_address("get_width", Some(2))? + OFFSET_GETWIDTH_END;
            self.address_getheight = hashlink.get_function_address("get_height", Some(1))? + OFFSET_GETHEIGHT_END;
        } else {
            return Err("Hashlink not initialized".into());
        }

        Ok(())
    }

    pub fn game_common_apply(&mut self, enable: bool) -> Result<(), Box<dyn Error>> {
        let mut injection_getwidth = self.injection_getwidth.lock().unwrap();
        let mut injection_getheight = self.injection_getheight.lock().unwrap();

        if enable {
            if injection_getwidth.is_none() {
                let mut code = CodeAssembler::new(64)?;
                code.pushfq()?;
                code.push(rax)?;
                code.push(rbx)?;
                code.mov(rbx, self.var_ptr_winwidth as u64)?;
                code.mov(dword_ptr(rbx), eax)?;
                code.pop(rbx)?;
                code.pop(rax)?;
                code.popfq()?;

                *injection_getwidth = Some(AobInjection::new(self.pid, self.address_getwidth, &mut code)?);
                tracing::info!("Successfully injected: getwidth at 0x{:X}", self.address_getwidth);
            }

            if injection_getheight.is_none() {
                let mut code = CodeAssembler::new(64)?;
                code.pushfq()?;
                code.push(rax)?;
                code.push(rbx)?;
                code.mov(rbx, self.var_ptr_winheight as u64)?;
                code.mov(dword_ptr(rbx), eax)?;
                code.pop(rbx)?;
                code.pop(rax)?;
                code.popfq()?;

                *injection_getheight = Some(AobInjection::new(self.pid, self.address_getheight, &mut code)?);
                tracing::info!("Successfully injected: getheight at 0x{:X}", self.address_getheight);
            }
        } else {
            if let Some(inj) = injection_getwidth.as_ref() {
                inj.undo()?;
                *injection_getwidth = None;
                tracing::info!("Successfully removed: getwidth at 0x{:X}", self.address_getwidth);
            }

            if let Some(inj) = injection_getheight.as_ref() {
                inj.undo()?;
                *injection_getheight = None;
                tracing::info!("Successfully removed: getheight at 0x{:X}", self.address_getheight);
            }
        }

        Ok(())
    }

    pub fn get_window_width(&self) -> Result<u32, Box<dyn Error>> {
        let winwidth = read_dword(self.pid, self.var_ptr_winwidth)? as u32;
        Ok(winwidth)
    }

    pub fn get_window_height(&self) -> Result<u32, Box<dyn Error>> {
        let winheight = read_dword(self.pid, self.var_ptr_winheight)? as u32;
        Ok(winheight)
    }
}