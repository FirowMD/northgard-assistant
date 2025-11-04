use crate::utils::libmem_ex::{read_bytes_ex, write_bytes_ex, free, allocate_region_near};
use iced_x86::{code_asm::*, Decoder, DecoderOptions, Code};
use libmem::Process;
use windows::Win32::System::Memory::PAGE_EXECUTE_READWRITE;
use std::error::Error;

#[allow(dead_code)]
pub struct CodeEntry {
    address: usize,
    size: usize,
    code: Vec<u8>,
}

#[allow(dead_code)]
pub struct LibmemInjection {
    pid: u32,
    address: usize,
    original_bytes: Vec<u8>,
    allocated_addr: usize,
    allocated_size: usize,
    next_free_addr: usize,
    next_free_size: usize,
    entries: Vec<CodeEntry>,
    process: Process,
}

impl LibmemInjection {
    pub fn new(pid: u32, address: usize, code: &mut CodeAssembler, process: &Process) -> Result<Self, Box<dyn Error>> {
        // Save original bytes using libmem
        let original_bytes = Self::read_original_instructions(process, address)?;
        let bytes_to_save = original_bytes.len();

        // Allocate memory for our code with RWX permissions, near target
        let estimated_code_size = 1024 + bytes_to_save + 5; // generous estimate + original + jmp
        let allocated = allocate_region_near(
            pid,
            address,
            estimated_code_size,
            PAGE_EXECUTE_READWRITE,
        )?;
        let new_code_address = allocated.base_address;

        // Assemble custom code at the correct address
        let custom_code = code.assemble(new_code_address as u64)?;

        // Create jump to our code
        let mut jmp_to_code = CodeAssembler::new(64)?;
        jmp_to_code.jmp(new_code_address as u64)?;
        let jmp_bytes = jmp_to_code.assemble(address as u64)?;

        // Write jump and original code using libmem
        Self::write_with_nops(process, address, &jmp_bytes, bytes_to_save)?;

        let mut current_address = new_code_address;

        // Write custom code
        write_bytes_ex(process, current_address, &custom_code)
            .ok_or("libmem write_bytes_ex failed")?;
        current_address += custom_code.len();

        // Write original instructions
        write_bytes_ex(process, current_address, &original_bytes)
            .ok_or("libmem write_bytes_ex failed")?;
        current_address += bytes_to_save;

        // Write jump back
        let mut jmp_back = CodeAssembler::new(64)?;
        jmp_back.jmp((address + bytes_to_save) as u64)?;
        let jmp_back_bytes = jmp_back.assemble(current_address as u64)?;
        write_bytes_ex(process, current_address, &jmp_back_bytes)
            .ok_or("libmem write_bytes_ex failed")?;
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
            allocated_addr: allocated.base_address,
            allocated_size: allocated.region_size,
            next_free_addr: current_address,
            next_free_size,
            entries: vec![entry],
            process: process.clone(),
        })
    }

    /// Undo the injection and free allocated memory
    pub fn undo(&self) -> Result<(), Box<dyn Error>> {
        // Restore original bytes using libmem
        write_bytes_ex(&self.process, self.address, &self.original_bytes)
            .ok_or("libmem write_bytes_ex failed")?;

        // Free allocated memory via libmem
        free(&self.process, self.allocated_addr, self.allocated_size)
            .ok_or("libmem free failed")?;

        Ok(())
    }

    /// Get all code entries
    pub fn get_entries(&self) -> &[CodeEntry] {
        &self.entries
    }

    // Helper methods
    fn read_original_instructions(process: &Process, address: usize) -> Result<Vec<u8>, Box<dyn Error>> {
        // Read a sufficiently large buffer to decode instructions
        let buffer = read_bytes_ex(process, address, 64)
            .ok_or("libmem read_bytes_ex failed")?;

        // Find required size for all necessary instructions
        let mut decoder = Decoder::with_ip(64, &buffer, address as u64, DecoderOptions::NONE);
        let mut size = 0;
        let mut required_instructions = 0;

        // Keep reading until we have enough instructions and at least 5 bytes for JMP
        while size < 5 || required_instructions < 3 {  // Ensure we get at least 3 instructions
            let instruction = decoder.decode();
            size += instruction.len();
            required_instructions += 1;

            // Stop if we hit conditional jump or return
            if instruction.is_jcc_near() || instruction.code() == Code::Retnq {
                break;
            }
        }

        let mut out = buffer;
        out.truncate(size);
        Ok(out)
    }

    fn write_with_nops(process: &Process, address: usize, data: &[u8], total_size: usize) -> Result<(), Box<dyn Error>> {
        write_bytes_ex(process, address, data)
            .ok_or("libmem write_bytes_ex failed")?;

        if data.len() < total_size {
            let nops = vec![0x90u8; total_size - data.len()];
            write_bytes_ex(process, address + data.len(), &nops)
                .ok_or("libmem write_bytes_ex failed")?;
        }
        Ok(())
    }
}