use crate::utils::memory::{MemoryRegion, allocate_region_near};
use iced_x86::{code_asm::*, Decoder, DecoderOptions, Encoder, Code, Instruction};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_VM_WRITE, PROCESS_VM_OPERATION, PROCESS_VM_READ};
use windows::Win32::System::Diagnostics::Debug::{WriteProcessMemory, ReadProcessMemory};
use windows::Win32::Foundation::{HANDLE, BOOL, CloseHandle};
use windows::Win32::System::Memory::PAGE_EXECUTE_READWRITE;
use std::error::Error;
use std::ffi::c_void;

struct ProcessHandleWrapper(HANDLE);

impl Drop for ProcessHandleWrapper {
    fn drop(&mut self) {
        unsafe { let _ = CloseHandle(self.0); }
    }
}

#[allow(dead_code)]
pub struct CodeEntry {
    address: usize,
    size: usize,
    code: Vec<u8>,
}

#[allow(dead_code)]
pub struct AobInjection {
    pid: u32,
    address: usize,
    original_bytes: Vec<u8>,
    allocated_region: MemoryRegion,
    next_free_addr: usize,
    next_free_size: usize,
    entries: Vec<CodeEntry>,
}

impl AobInjection {
    pub fn new(pid: u32, address: usize, code: &mut CodeAssembler) -> Result<Self, Box<dyn Error>> {
        let handle_wrapper = ProcessHandleWrapper(unsafe {
            OpenProcess(
                PROCESS_VM_WRITE | PROCESS_VM_OPERATION | PROCESS_VM_READ,
                BOOL::from(false),
                pid,
            )
        }.map_err(|e| format!("Failed to open process: {:?}", e))?);

        // Save original bytes
        let original_bytes = Self::read_original_instructions(&handle_wrapper, address)?;
        let bytes_to_save = original_bytes.len();

        // Allocate memory for our code with RWX permissions
        let estimated_code_size = 1024 + bytes_to_save + 5; // Generous estimate + original + jmp
        let allocated = allocate_region_near(
            pid, 
            address, 
            estimated_code_size,
            PAGE_EXECUTE_READWRITE
        )?;
        let new_code_address = allocated.base_address;

        // Assemble the code at the correct address
        let custom_code = code.assemble(new_code_address as u64)?;

        // Create jump to our code
        let mut jmp_to_code = CodeAssembler::new(64)?;
        jmp_to_code.jmp(new_code_address as u64)?;
        let jmp_bytes = jmp_to_code.assemble(address as u64)?;

        // Write jump and original code
        Self::write_with_nops(&handle_wrapper, address, &jmp_bytes, bytes_to_save)?;

        let mut current_address = new_code_address;
        
        // Write custom code
        Self::write_memory(&handle_wrapper, current_address, &custom_code)?;
        current_address += custom_code.len();

        // Write original instructions
        Self::write_memory(&handle_wrapper, current_address, &original_bytes)?;
        current_address += bytes_to_save;

        // Write jump back
        let mut jmp_back = CodeAssembler::new(64)?;
        jmp_back.jmp((address + bytes_to_save) as u64)?;
        let jmp_back_bytes = jmp_back.assemble(current_address as u64)?;
        
        Self::write_memory(&handle_wrapper, current_address, &jmp_back_bytes)?;
        current_address += jmp_back_bytes.len();

        // Calculate remaining space
        let used_space = current_address - new_code_address;
        let next_free_size = allocated.region_size - used_space;

        let entry = CodeEntry {
            address: new_code_address,
            size: custom_code.len(),
            code: custom_code,
        };

        Ok(Self {
            pid,
            address,
            original_bytes,
            allocated_region: allocated,
            next_free_addr: current_address,
            next_free_size,
            entries: vec![entry],
        })
    }

    /// Undo the injection and free allocated memory
    pub fn undo(&self) -> Result<(), Box<dyn Error>> {
        let handle_wrapper = ProcessHandleWrapper(unsafe {
            OpenProcess(
                PROCESS_VM_WRITE | PROCESS_VM_OPERATION,
                BOOL::from(false),
                self.pid,
            )
        }.map_err(|e| format!("Failed to open process: {:?}", e))?);

        // Restore original bytes
        Self::write_memory(&handle_wrapper, self.address, &self.original_bytes)?;

        // Free allocated memory
        unsafe {
            use windows::Win32::System::Memory::{VirtualFreeEx, MEM_RELEASE};
            VirtualFreeEx(
                handle_wrapper.0,
                self.allocated_region.base_address as *mut c_void,
                0,
                MEM_RELEASE,
            ).map_err(|_| "Failed to free allocated memory")?;
        }

        Ok(())
    }

    /// Get all code entries
    pub fn get_entries(&self) -> &[CodeEntry] {
        &self.entries
    }

    // Helper methods
    fn read_original_instructions(handle: &ProcessHandleWrapper, address: usize) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut buffer = vec![0u8; 32];  // Increase buffer size to capture more instructions
        let mut bytes_read = 0;

        unsafe {
            ReadProcessMemory(
                handle.0,
                address as *const _,
                buffer.as_mut_ptr() as *mut _,
                buffer.len(),
                Some(&mut bytes_read),
            )
        }.map_err(|_| "Failed to read original instructions")?;

        // Find required size for all necessary instructions
        let mut decoder = Decoder::with_ip(64, &buffer, address as u64, DecoderOptions::NONE);
        let mut size = 0;
        let mut required_instructions = 0;
        
        // Keep reading until we have enough instructions and at least 5 bytes for JMP
        while size < 5 || required_instructions < 3 {  // Ensure we get at least 3 instructions
            let instruction = decoder.decode();
            size += instruction.len();
            required_instructions += 1;
            
            // Check if we've hit a conditional jump or return (0xC3)
            if instruction.is_jcc_near() || instruction.code() == Code::Retnq {
                break;
            }
        }

        buffer.truncate(size);
        Ok(buffer)
    }

    fn write_memory(handle: &ProcessHandleWrapper, address: usize, data: &[u8]) -> Result<(), Box<dyn Error>> {
        let mut bytes_written = 0;
        unsafe {
            WriteProcessMemory(
                handle.0,
                address as *mut _,
                data.as_ptr() as *const c_void,
                data.len(),
                Some(&mut bytes_written),
            )
        }.map_err(|_| "Failed to write memory")?;

        if bytes_written != data.len() {
            return Err("Incomplete write".into());
        }
        Ok(())
    }

    fn write_with_nops(handle: &ProcessHandleWrapper, address: usize, data: &[u8], total_size: usize) -> Result<(), Box<dyn Error>> {
        Self::write_memory(handle, address, data)?;
        
        if data.len() < total_size {
            let nops = vec![0x90u8; total_size - data.len()];
            Self::write_memory(handle, address + data.len(), &nops)?;
        }
        Ok(())
    }

    fn create_jmp(from: u64, to: u64) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut encoder = Encoder::new(64);
        let instruction = Instruction::with_branch(Code::Jmp_rel32_64, to)?;
        encoder.encode(&instruction, from)?;
        Ok(encoder.take_buffer())
    }
}
