use crate::commands::base::{Command, InjectionManager};
use iced_x86::code_asm::*;
use std::error::Error;

pub struct AutoAccept {
    address_setcheckedjoin: usize,
    enabled: bool,
    injection_manager: InjectionManager,
}

impl AutoAccept {
    pub fn new() -> Self {
        Self {
            address_setcheckedjoin: 0,
            enabled: false,
            injection_manager: InjectionManager::new(0), // Will be updated in init
        }
    }

    pub fn auto_accept_apply(&mut self, enable: bool) -> Result<(), Box<dyn Error>> {
        self.apply(enable)
    }
}

impl Command for AutoAccept {
    fn init(&mut self, ctx: &mut crate::commands::base::CommandContext) -> Result<(), Box<dyn Error>> {
        const OFFSET_SETCHECKEDJOIN: usize = 12;
        
        // Update injection manager with correct PID
        self.injection_manager = InjectionManager::new(ctx.pid);
        self.injection_manager.add_injection("setCheckedJoin".to_string());

        // Get function address using context helper
        self.address_setcheckedjoin = ctx.get_function_address("setCheckedJoin", Some(0))?;
        self.address_setcheckedjoin += OFFSET_SETCHECKEDJOIN;

        Ok(())
    }

    fn apply(&mut self, enable: bool) -> Result<(), Box<dyn Error>> {
        if enable {
            let mut code = CodeAssembler::new(64)?;
            code.mov(dl, 1)?;
            
            self.injection_manager.apply_injection(
                "setCheckedJoin", 
                self.address_setcheckedjoin, 
                &mut code
            )?;
        } else {
            self.injection_manager.remove_injection("setCheckedJoin")?;
        }

        self.enabled = enable;
        Ok(())
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn name(&self) -> &'static str {
        "AutoAccept"
    }
}
