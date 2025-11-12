use crate::modules::libmem_injection::LibmemInjection;
use crate::modules::mem_alloc::*;
use crate::modules::hashlink::*;
use crate::utils::libmem_ex::get_target_process;
use libmem::Process;
use std::error::Error;
use std::sync::Mutex;
use std::collections::HashMap;

/// Shared context for all commands to reduce duplication
pub struct CommandContext {
    pub pid: u32,
    pub mem_allocator: MemoryAllocator,
}

impl CommandContext {
    pub fn new(pid: u32) -> Result<Self, Box<dyn Error>> {
        let mem_allocator = MemoryAllocator::new(pid, 0x1000)?;
        
        Ok(Self {
            pid,
            mem_allocator,
        })
    }

    /// Helper to get function address from hashlink
    pub fn get_function_address(&self, function_name: &str, index: Option<usize>) -> Result<usize, Box<dyn Error>> {
        let hashlink = Hashlink::instance(self.pid);
        let guard = hashlink.lock().unwrap();
        if let Some(hashlink) = guard.as_ref() {
            hashlink.get_function_address(function_name, index)
        } else {
            Err("Hashlink not initialized".into())
        }
    }

    /// Helper to allocate variables
    pub fn allocate_var(&mut self, name: &str, data_type: DataType) -> Result<usize, Box<dyn Error>> {
        self.mem_allocator.allocate_var(name, data_type)
    }

    /// Helper to allocate wide strings
    pub fn allocate_wide_string(&mut self, name: &str, value: &str) -> Result<usize, Box<dyn Error>> {
        self.mem_allocator.allocate_wide_string(name, value)
    }
}

/// Base trait that all commands should implement
pub trait Command {
    /// Initialize the command with the given context
    fn init(&mut self, ctx: &mut CommandContext) -> Result<(), Box<dyn Error>>;
    
    /// Apply or remove the command's functionality
    fn apply(&mut self, enable: bool) -> Result<(), Box<dyn Error>>;
    
    /// Check if the command is currently enabled
    fn is_enabled(&self) -> bool;
    
    /// Get the command's name for logging
    fn name(&self) -> &'static str;
}

/// Injection manager to handle common injection patterns
pub struct InjectionManager {
    injections: HashMap<String, Mutex<Option<LibmemInjection>>>,
    pid: u32,
    process: Process,
}

impl InjectionManager {
    pub fn new(pid: u32) -> Self {
        let process = get_target_process(pid).expect("Failed to get process with libmem");
        Self {
            injections: HashMap::new(),
            pid,
            process,
        }
    }

    /// Add a new injection point
    pub fn add_injection(&mut self, name: String) {
        self.injections.insert(name, Mutex::new(None));
    }

    /// Apply injection at a specific address
    pub fn apply_injection(&self, name: &str, address: usize, code: &mut iced_x86::code_asm::CodeAssembler) -> Result<(), Box<dyn Error>> {
        if let Some(injection_mutex) = self.injections.get(name) {
            let mut injection = injection_mutex.lock().unwrap();
            
            // Remove existing injection
            if let Some(inj) = injection.as_ref() {
                inj.undo()?;
                tracing::info!("Removed injection: {} at 0x{:X}", name, address);
            }

            // Apply new injection
            *injection = Some(LibmemInjection::new(self.pid, address, code, &self.process)?);
            tracing::info!("Applied injection: {} at 0x{:X}", name, address);
        }
        Ok(())
    }

    /// Remove a specific injection
    pub fn remove_injection(&self, name: &str) -> Result<(), Box<dyn Error>> {
        if let Some(injection_mutex) = self.injections.get(name) {
            let mut injection = injection_mutex.lock().unwrap();
            if let Some(inj) = injection.as_ref() {
                inj.undo()?;
                *injection = None;
                tracing::info!("Removed injection: {}", name);
            }
        }
        Ok(())
    }

    /// Remove all injections
    pub fn remove_all(&self) -> Result<(), Box<dyn Error>> {
        for (name, injection_mutex) in &self.injections {
            let mut injection = injection_mutex.lock().unwrap();
            if let Some(inj) = injection.as_ref() {
                inj.undo()?;
                *injection = None;
                tracing::info!("Removed injection: {}", name);
            }
        }
        Ok(())
    }
}