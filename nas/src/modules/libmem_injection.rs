use crate::utils::libmem_ex::{read_bytes_ex, write_bytes_ex, free, allocate_region_near};
use iced_x86::code_asm::*;
use iced_x86::{Decoder, DecoderOptions};
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
    overwritten_len: usize,
    resume_address: usize,
}

impl LibmemInjection {
    pub fn new(pid: u32, address: usize, code: &mut CodeAssembler, process: &Process) -> Result<Self, Box<dyn Error>> {
        // Allocate memory near the target address (payload + room for stolen bytes + tail jmp)
        // We will assemble the payload using the allocated base address to ensure correct relative offsets
        let alloc = allocate_region_near(pid, address, 0x1000, PAGE_EXECUTE_READWRITE)?;
        let payload_bytes = code.assemble(alloc.base_address as u64)?;

        // Build trampoline bytes targeting our allocated payload
        let mut trampoline_final = CodeAssembler::new(64)?;
        trampoline_final.jmp(alloc.base_address as u64)?;
        let trampoline_bytes = trampoline_final.assemble(address as u64)?;

        // Determine stolen instruction length (>= near JMP size)
        let probe = read_bytes_ex(process, address, 32).ok_or("libmem read_bytes_ex failed")?;
        let mut decoder = Decoder::with_ip(64, &probe, address as u64, DecoderOptions::NONE);
        let mut stolen_len = 0usize;
        while stolen_len < trampoline_bytes.len() {
            let instr = decoder.decode();
            let ilen = instr.len();
            if ilen == 0 { break; }
            stolen_len += ilen;
        }

        // Read stolen bytes exactly for undo and for re-emitting in the stub
        let original = read_bytes_ex(process, address, stolen_len).ok_or("libmem read_bytes_ex failed")?;

        // Write the payload to allocated memory
        write_bytes_ex(process, alloc.base_address, &payload_bytes).ok_or("libmem write_bytes_ex failed")?;

        // Emit stolen bytes into the allocated region after our payload
        let stolen_dst = alloc.base_address + payload_bytes.len();
        write_bytes_ex(process, stolen_dst, &original).ok_or("libmem write_bytes_ex failed")?;

        // Append a tail jump back to the original function after the stolen bytes
        let resume_address = address + stolen_len;
        let mut tail = CodeAssembler::new(64)?;
        tail.jmp(resume_address as u64)?;
        let tail_bytes = tail.assemble((stolen_dst + original.len()) as u64)?;
        write_bytes_ex(process, stolen_dst + original.len(), &tail_bytes).ok_or("libmem write_bytes_ex failed")?;

        // Patch the hook site with the trampoline and NOP the remainder of stolen bytes
        write_bytes_ex(process, address, &trampoline_bytes).ok_or("libmem write_bytes_ex failed")?;
        if stolen_len > trampoline_bytes.len() {
            let nops = vec![0x90u8; stolen_len - trampoline_bytes.len()];
            write_bytes_ex(process, address + trampoline_bytes.len(), &nops).ok_or("libmem write_bytes_ex failed")?;
        }

        let original_len = original.len();
        Ok(Self {
            pid,
            address,
            original_bytes: original.clone(),
            allocated_addr: alloc.base_address,
            allocated_size: alloc.region_size,
            next_free_addr: alloc.base_address + payload_bytes.len() + original_len + tail_bytes.len(),
            next_free_size: alloc.region_size - (payload_bytes.len() + original_len + tail_bytes.len()),
            entries: Vec::new(),
            process: process.clone(),
            overwritten_len: stolen_len,
            resume_address,
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
}