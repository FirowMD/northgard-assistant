use libmem::{
    Address,
    Process,
    free_memory_ex,
    get_process,
    get_process_ex,
    read_memory_ex,
    write_memory_ex,
};
use std::error::Error;
use windows::Win32::Foundation::BOOL;
use windows::Win32::System::Memory::{
    PAGE_PROTECTION_FLAGS, MEM_COMMIT, MEM_RESERVE, MEMORY_BASIC_INFORMATION,
    VirtualAllocEx, VirtualQueryEx, MEM_RELEASE, VirtualFreeEx,
};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_VM_OPERATION, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ, PROCESS_VM_WRITE};

// Acquire a libmem Process for a target PID when external, or current when internal.
pub fn get_target_process(pid: u32) -> Option<Process> {
    // Prefer external by PID; fall back to current process if unavailable.
    get_process_ex(pid).or_else(|| get_process())
}

// Free previously allocated memory in the target process.
pub fn free(process: &Process, addr: Address, size: usize) -> Option<()> {
    free_memory_ex(process, addr, size)
}

// Read a 64-bit value from a target address.
pub fn read_qword_ex(process: &Process, address: Address) -> Option<u64> {
    read_memory_ex::<u64>(process, address)
}

// Read a UTF-16 string up to max_chars from a target address.
pub fn read_utf16_string_ex(process: &Process, address: Address, max_chars: usize) -> Option<String> {
    let mut buf = Vec::<u16>::with_capacity(max_chars);
    for i in 0..max_chars {
        let ch = read_memory_ex::<u16>(process, address + i * 2)?;
        if ch == 0 { break; }
        buf.push(ch);
    }
    Some(String::from_utf16_lossy(&buf))
}

// Read a contiguous block of bytes from a target address.
pub fn read_bytes_ex(process: &Process, address: Address, size: usize) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(size);
    for i in 0..size {
        out.push(read_memory_ex::<u8>(process, address + i)?);
    }
    Some(out)
}

// Write a contiguous block of bytes to a target address.
pub fn write_bytes_ex(process: &Process, address: Address, data: &[u8]) -> Option<()> {
    write_memory_ex(process, address, data)
}

// Simple allocation descriptor used by libmem-based helpers.
#[derive(Clone, Copy, Debug)]
pub struct AllocRegion {
    pub base_address: usize,
    pub region_size: usize,
}

// Allocate memory region with specified protection using Windows APIs.
// Returns AllocRegion with base address and size.
pub fn allocate_region_mrprotect(pid: u32, size: usize, protection: PAGE_PROTECTION_FLAGS) -> Result<AllocRegion, Box<dyn Error>> {
    let handle = unsafe {
        OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ | PROCESS_VM_WRITE | PROCESS_VM_OPERATION,
            BOOL::from(false),
            pid,
        )
    }.map_err(|e| format!("Failed to open process: {:?}", e))?;

    let address = unsafe {
        VirtualAllocEx(
            handle,
            None,
            size,
            MEM_COMMIT | MEM_RESERVE,
            protection,
        )
    };

    if address.is_null() {
        return Err("Failed to allocate memory region".into());
    }

    // Query for the actual region size
    let mut mbi = MEMORY_BASIC_INFORMATION::default();
    let result = unsafe {
        VirtualQueryEx(
            handle,
            Some(address),
            &mut mbi,
            std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
        )
    };
    if result == 0 {
        return Err("Failed to query allocated memory region".into());
    }

    Ok(AllocRegion { base_address: mbi.BaseAddress as usize, region_size: mbi.RegionSize })
}

// Allocate memory region close to the specified address (±2GB) for relative jumps.
pub fn allocate_region_near(
    pid: u32,
    target_address: usize,
    size: usize,
    protection: PAGE_PROTECTION_FLAGS,
) -> Result<AllocRegion, Box<dyn Error>> {
    // Try to allocate within ±2GB range of target address to ensure RIP-relative addressing works
    const MAX_DISTANCE: i64 = 0x7FFF_0000; // ~2GB

    let handle = unsafe {
        OpenProcess(
            PROCESS_VM_OPERATION,
            BOOL::from(false),
            pid,
        )
    }.map_err(|e| format!("Failed to open process: {:?}", e))?;

    let mut offset = 0;
    while offset < MAX_DISTANCE {
        for &addr in &[
            target_address.wrapping_add(offset as usize),
            target_address.wrapping_sub(offset as usize),
        ] {
            let ptr = unsafe {
                VirtualAllocEx(
                    handle,
                    Some(addr as *mut _),
                    size,
                    MEM_COMMIT | MEM_RESERVE,
                    protection,
                )
            };

            if !ptr.is_null() {
                let base = ptr as usize;
                let distance = (base as i64) - (target_address as i64);
                if distance.abs() < MAX_DISTANCE {
                    return Ok(AllocRegion { base_address: base, region_size: size });
                }

                // Not within acceptable range; free and continue
                let _ = unsafe { VirtualFreeEx(handle, ptr, 0, MEM_RELEASE) };
            }
        }

        // Increase offset by page size
        offset += 0x1000;
    }

    tracing::warn!("Could not allocate memory near target region, falling back to regular allocation");
    allocate_region_mrprotect(pid, size, protection)
}