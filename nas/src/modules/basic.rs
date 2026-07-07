use crate::utils::memory::{enum_memory_regions, MemoryRegion};

use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ, PROCESS_VM_WRITE, PROCESS_VM_OPERATION};
use windows::Win32::Foundation::{HANDLE, BOOL, CloseHandle};
use windows::Win32::System::Diagnostics::Debug::{ReadProcessMemory, WriteProcessMemory};
use windows::Win32::System::Memory::{MEMORY_BASIC_INFORMATION, VirtualQueryEx};
use std::ffi::c_void;

use std::error::Error;

const CHUNK_SIZE: usize = 0x1000; // 4KB chunks
const MAX_HIT_COUNT: u32 = 5000;

/// Convert an address to a scan data.
/// 
/// Example of argument `address`:
/// ```
/// 0x12345678
/// ```
/// 
/// Example of return value (64-bit):
/// ```
/// [0x78, 0x56, 0x34, 0x12, 0x00, 0x00, 0x00, 0x00]
/// ```
pub fn convert_address_to_scan_data(address: usize) -> Vec<u8> {
    let bytes = address.to_le_bytes();
    bytes.iter().map(|b| *b as u8).collect()
}

/// Convert a UTF-16 string to a scan data.
/// 
/// Example of argument `string`:
/// ```
/// abc
/// ```
/// 
/// Example of return value:
/// ```
/// [0x61, 0x00, 0x62, 0x00, 0x63, 0x00]
/// ```
pub fn convert_string_utf16_to_scan_data(text: &str) -> Vec<u8> {
    let bytes: Vec<u8> = text.encode_utf16()
        .flat_map(|c| c.to_le_bytes())
        .collect();
    bytes.iter().map(|b| *b as u8).collect()
}

/// Convert a UTF-8 string to a scan data.
/// 
/// Example of argument `text`:
/// ```
/// abc
/// ```
/// 
/// Example of return value:
/// ```
/// [0x61, 0x62, 0x63]
/// ```
pub fn convert_string_utf8_to_scan_data(text: &str) -> Vec<u8> {
    text.as_bytes().to_vec()
}

struct ProcessHandleWrapper(HANDLE);

impl Drop for ProcessHandleWrapper {
    fn drop(&mut self) {
        unsafe { let _ = CloseHandle(self.0); }
    }
}

/// Read byte from a process's memory.
/// 
/// Returns the byte at the specified address.
pub fn read_byte(pid: u32, address: usize) -> Result<u8, Box<dyn Error>> {
    let mut buffer = [0u8; 1];
    
    let handle_wrapper = ProcessHandleWrapper(unsafe {
        OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ | PROCESS_VM_WRITE | PROCESS_VM_OPERATION,
            BOOL::from(false),
            pid,
        )
    }.map_err(|e| format!("Failed to open process: {:?}", e))?);

    let mut bytes_read = 0;
    let _success = unsafe {
        ReadProcessMemory(
            handle_wrapper.0,
            address as *const _,
            buffer.as_mut_ptr() as *mut _,
            buffer.len(),
            Some(&mut bytes_read),
        )
    };

    if _success.is_err() || bytes_read != buffer.len() {
        Err("Failed to read memory".into())
    } else {
        Ok(buffer[0])
    }
}

/// Read word from a process's memory.  
/// 
/// Returns the word at the specified address.
pub fn read_word(pid: u32, address: usize) -> Result<u16, Box<dyn Error>> {
    let mut buffer = [0u8; 2];
    
    let handle_wrapper = ProcessHandleWrapper(unsafe {
        OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ | PROCESS_VM_WRITE | PROCESS_VM_OPERATION,
            BOOL::from(false),
            pid,
        )
    }.map_err(|e| format!("Failed to open process: {:?}", e))?);

    let mut bytes_read = 0;
    let _success = unsafe {
        ReadProcessMemory(
            handle_wrapper.0,
            address as *const _,
            buffer.as_mut_ptr() as *mut _,
            buffer.len(),
            Some(&mut bytes_read),
        )
    };

    if _success.is_err() || bytes_read != buffer.len() {
        Err("Failed to read memory".into())
    } else {
        Ok(u16::from_le_bytes(buffer))
    }
}

/// Read double word from a process's memory.
/// 
/// Returns the double word at the specified address.
pub fn read_dword(pid: u32, address: usize) -> Result<u32, Box<dyn Error>> {
    let mut buffer = [0u8; 4];
    
    let handle_wrapper = ProcessHandleWrapper(unsafe {
        OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ | PROCESS_VM_WRITE | PROCESS_VM_OPERATION,
            BOOL::from(false),
            pid,
        )
    }.map_err(|e| format!("Failed to open process: {:?}", e))?);

    let mut bytes_read = 0;
    let _success = unsafe {
        ReadProcessMemory(
            handle_wrapper.0,
            address as *const _,
            buffer.as_mut_ptr() as *mut _,
            buffer.len(),
            Some(&mut bytes_read),
        )
    };

    if _success.is_err() || bytes_read != buffer.len() {
        Err("Failed to read memory".into())
    } else {
        Ok(u32::from_le_bytes(buffer))
    }
}

/// Read quad word from a process's memory.
/// 
/// Returns the quad word at the specified address.
pub fn read_qword(pid: u32, address: usize) -> Result<u64, Box<dyn Error>> {
    let mut buffer = [0u8; 8];
    
    let handle_wrapper = ProcessHandleWrapper(unsafe {
        OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ | PROCESS_VM_WRITE | PROCESS_VM_OPERATION,
            BOOL::from(false),
            pid,
        )
    }.map_err(|e| format!("Failed to open process: {:?}", e))?);

    let mut bytes_read = 0;
    let _success = unsafe {
        ReadProcessMemory(
            handle_wrapper.0,
            address as *const _,
            buffer.as_mut_ptr() as *mut _,
            buffer.len(),
            Some(&mut bytes_read),
        )
    };

    if _success.is_err() || bytes_read != buffer.len() {
        Err("Failed to read memory".into())
    } else {
        Ok(u64::from_le_bytes(buffer))
    }
}

/// Reads 4 or 8 bytes from a process's memory, according to architecture.
/// 
/// Returns the value at the specified address.
pub fn read_pointer(pid: u32, address: usize) -> Result<usize, Box<dyn Error>> {
    let size_of_usize = std::mem::size_of::<usize>();
    if size_of_usize == 8 {
        Ok(read_qword(pid, address)? as usize)
    } else {
        Ok(read_dword(pid, address)? as usize)
    }
}

/// Read bytes from a process's memory.
/// 
/// Returns the bytes at the specified address.
pub fn read_bytes(pid: u32, address: usize, length: usize) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut buffer = vec![0u8; length];
    
    let handle_wrapper = ProcessHandleWrapper(unsafe {
        OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ | PROCESS_VM_WRITE | PROCESS_VM_OPERATION,
            BOOL::from(false),
            pid,
        )
    }.map_err(|e| format!("Failed to open process: {:?}", e))?);

    let mut bytes_read = 0;
    let _success = unsafe {
        ReadProcessMemory(
            handle_wrapper.0,
            address as *const _,
            buffer.as_mut_ptr() as *mut _,
            buffer.len(),
            Some(&mut bytes_read),
        )
    };

    if _success.is_err() || bytes_read != buffer.len() {
        Err("Failed to read memory".into())
    } else {
        Ok(buffer)
    }
}

/// Read utf-16 string from a process's memory.
/// Ends reading when null terminator ("\x00\x00") is found.
/// 
/// Returns the string at the specified address.
pub fn read_utf16_string(pid: u32, address: usize) -> Result<String, Box<dyn Error>> {
    let mut buffer = Vec::new();
    let mut i = 0;
    loop {
        let bytes = read_bytes(pid, address + i * 2, 2)?;
        let value = u16::from_le_bytes(bytes.try_into().unwrap());
        if value == 0 {
            break;
        }
        buffer.push(value);
        i += 1;
    }

    Ok(String::from_utf16_lossy(&buffer))
}

/// Convert a byte array to a dword.
///
/// Returns the dword.
pub fn byte_array_to_dword(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(bytes.try_into().unwrap())
}

/// Convert a byte array to a qword.
///
/// Returns the qword.
pub fn byte_array_to_qword(bytes: &[u8]) -> u64 {
    u64::from_le_bytes(bytes.try_into().unwrap())
}

/// Convert a byte array to a pointer.
///
/// Returns the pointer.
pub fn byte_array_to_pointer(bytes: &[u8]) -> usize {
    usize::from_le_bytes(bytes.try_into().unwrap())
}

/// Convert a pattern string to bytes with wildcards.
/// Example: "12 34 ?? 56" -> (vec![0x12, 0x34, 0x00, 0x56], vec![true, true, false, true])
fn pattern_to_bytes(pattern: &str) -> (Vec<u8>, Vec<bool>) {
    let parts: Vec<&str> = pattern.split_whitespace().collect();
    let mut bytes = Vec::with_capacity(parts.len());
    let mut masks = Vec::with_capacity(parts.len());

    for part in parts {
        if part == "??" {
            bytes.push(0);
            masks.push(false);
        } else {
            if let Ok(byte) = u8::from_str_radix(part, 16) {
                bytes.push(byte);
                masks.push(true);
            }
        }
    }

    (bytes, masks)
}

/// Scan for pattern in a process's memory.
/// Pattern format: "12 34 ?? 56" where ?? is wildcard
/// Returns addresses of all matches.
pub fn aob_scan(pid: u32, pattern: &str) -> Result<Vec<usize>, Box<dyn Error>> {
    let (bytes, masks) = pattern_to_bytes(pattern);
    if bytes.is_empty() {
        return Err("Invalid pattern".into());
    }

    let regions = enum_memory_regions(pid as u32)?;
    let mut matches = Vec::new();
    let mut buffer = vec![0u8; CHUNK_SIZE];
    
    let handle_wrapper = ProcessHandleWrapper(unsafe {
        OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ | PROCESS_VM_WRITE | PROCESS_VM_OPERATION,
            BOOL::from(false),
            pid,
        )
    }.map_err(|e| format!("Failed to open process: {:?}", e))?);

    for region in regions {
        if region.region_size < bytes.len() {
            continue;
        }

        let mut offset = 0;
        while offset <= region.region_size - bytes.len() {
            let read_size = std::cmp::min(CHUNK_SIZE, region.region_size - offset);
            let base_addr = region.base_address + offset;

            let mut bytes_read: usize = 0;
            let _success = unsafe {
                ReadProcessMemory(
                    handle_wrapper.0,
                    base_addr as *const _,
                    buffer.as_mut_ptr() as *mut _,
                    read_size,
                    Some(&mut bytes_read),
                )
            };

            if _success.is_err() || bytes_read == 0 {
                offset += CHUNK_SIZE;
                continue;
            }

            for i in 0..=(bytes_read - bytes.len()) {
                let mut found = true;
                for (j, (&pattern_byte, &mask)) in bytes.iter().zip(masks.iter()).enumerate() {
                    if mask && buffer[i + j] != pattern_byte {
                        found = false;
                        break;
                    }
                }
                if found {
                    matches.push(base_addr + i);
                }
            }

            offset += bytes_read;
        }
    }

    if matches.is_empty() {
        Err(format!("Pattern {} not found", pattern).into())
    } else {
        Ok(matches)
    }
}

/// Scan for pattern in specified memory region type.
/// Pattern format: "12 34 ?? 56" where ?? is wildcard
/// Returns addresses of all matches.
pub fn aob_scan_mrtype(pid: u32, pattern: &str, mr_type: u32) -> Result<Vec<usize>, Box<dyn Error>> {
    let (bytes, masks) = pattern_to_bytes(pattern);
    if bytes.is_empty() {
        return Err("Invalid pattern".into());
    }

    let regions = enum_memory_regions(pid as u32)?;
    let mut matches = Vec::new();
    let mut buffer = vec![0u8; CHUNK_SIZE];
    
    let filtered_regions: Vec<MemoryRegion> = regions.into_iter()
        .filter(|region| region.type_ == mr_type)
        .collect();

    let handle_wrapper = ProcessHandleWrapper(unsafe {
        OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ | PROCESS_VM_WRITE | PROCESS_VM_OPERATION,
            BOOL::from(false),
            pid,
        )
    }.map_err(|e| format!("Failed to open process: {:?}", e))?);

    for region in filtered_regions {
        if region.region_size < bytes.len() {
            continue;
        }

        let mut offset = 0;
        while offset <= region.region_size - bytes.len() {
            let read_size = std::cmp::min(CHUNK_SIZE, region.region_size - offset);
            let base_addr = region.base_address + offset;

            let mut bytes_read: usize = 0;
            let _success = unsafe {
                ReadProcessMemory(
                    handle_wrapper.0,
                    base_addr as *const _,
                    buffer.as_mut_ptr() as *mut _,
                    read_size,
                    Some(&mut bytes_read),
                )
            };

            if _success.is_err() || bytes_read == 0 {
                offset += CHUNK_SIZE;
                continue;
            }

            for i in 0..=(bytes_read - bytes.len()) {
                let mut found = true;
                for (j, (&pattern_byte, &mask)) in bytes.iter().zip(masks.iter()).enumerate() {
                    if mask && buffer[i + j] != pattern_byte {
                        found = false;
                        break;
                    }
                }
                if found {
                    matches.push(base_addr + i);
                }
            }

            offset += bytes_read - bytes.len() + 1;
        }
    }

    if matches.is_empty() {
        Err(format!("Pattern {} not found", pattern).into())
    } else {
        Ok(matches)
    }
}

/// Translated from Cheat Engine's aobscan function.
pub fn aob_scan_ce(
    h_process: HANDLE,
    pattern: &[u8],
    mask: &str,
    start: u64,
    end: u64,
    inc: usize,
    protection: u32,
    match_addr: &mut Vec<u64>,
) -> Result<usize, i32> {
    let mut tmp = start;
    let mut memory_buffer = vec![0u8; 4096];
    let pattern_length = mask.len();

    while tmp < end {
        let mut mbi = MEMORY_BASIC_INFORMATION::default();
        if unsafe {
            VirtualQueryEx(
                h_process,
                Some(tmp as *const c_void),
                &mut mbi,
                std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        } == 0
        {
            return Err(-1);
        }

        let region = MemoryRegion {
            base_address: mbi.BaseAddress as usize,
            allocation_base: mbi.AllocationBase as usize,
            allocation_protect: mbi.AllocationProtect.0,
            region_size: mbi.RegionSize,
            state: mbi.State.0,
            protect: mbi.Protect.0,
            type_: mbi.Type.0,
        };

        if (region.protect & protection) != 0 {
            let mut tmp2 = tmp;
            while tmp2 < tmp + region.region_size as u64 {
                let _success = unsafe {
                    ReadProcessMemory(
                        h_process,
                        tmp2 as *const c_void,
                        memory_buffer.as_mut_ptr() as *mut c_void,
                        memory_buffer.len(),
                        None,
                    )
                };

                for i in (0..memory_buffer.len()).step_by(inc) {
                    if i + pattern_length > memory_buffer.len() {
                        break;
                    }

                    let mut match_found = true;
                    for k in 0..pattern_length {
                        if mask.as_bytes()[k] != b'?' && pattern[k] != memory_buffer[i + k] {
                            match_found = false;
                            break;
                        }
                    }

                    if match_found {
                        match_addr.push(tmp2 + i as u64);
                        if match_addr.len() >= MAX_HIT_COUNT as usize {
                            return Ok(match_addr.len());
                        }
                    }
                }

                tmp2 += memory_buffer.len() as u64;
            }
        }

        tmp += region.region_size as u64;
    }

    Ok(match_addr.len())
}

/// Scan for pattern in specified memory region protection.
/// Pattern format: "12 34 ?? 56" where ?? is wildcard
/// Returns addresses of all matches.
pub fn aob_scan_mrprotect(pid: u32, pattern: &str, mr_protect: u32) -> Result<Vec<usize>, Box<dyn Error>> {
    let (bytes, masks) = pattern_to_bytes(pattern);
    if bytes.is_empty() {
        return Err("Invalid pattern".into());
    }

    let regions = enum_memory_regions(pid)?;
    let mut matches = Vec::new();
    let mut buffer = vec![0u8; CHUNK_SIZE];
    
    let filtered_regions: Vec<MemoryRegion> = regions.into_iter()
        .filter(|region| region.protect & mr_protect != 0)
        .collect();

    let handle_wrapper = ProcessHandleWrapper(unsafe {
        OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ | PROCESS_VM_WRITE | PROCESS_VM_OPERATION,
            BOOL::from(false),
            pid,
        )
    }.map_err(|e| format!("Failed to open process: {:?}", e))?);

    // Use increment of 1 for maximum accuracy (can be adjusted for speed)
    let inc: usize = 1;

    for region in filtered_regions {
        if region.region_size < bytes.len() {
            continue;
        }

        let mut current_addr = region.base_address;
        while current_addr < region.base_address + region.region_size {
            // Calculate how many bytes we can read in this chunk
            let remaining_in_region = (region.base_address + region.region_size) - current_addr;
            let read_size = std::cmp::min(CHUNK_SIZE, remaining_in_region);
            
            if read_size < bytes.len() {
                break;
            }

            let mut bytes_read: usize = 0;
            let _success = unsafe {
                ReadProcessMemory(
                    handle_wrapper.0,
                    current_addr as *const _,
                    buffer.as_mut_ptr() as *mut _,
                    read_size,
                    Some(&mut bytes_read),
                )
            };

            if _success.is_err() || bytes_read == 0 {
                current_addr += CHUNK_SIZE;
                continue;
            }

            // Scan through the buffer
            let scan_end = bytes_read - bytes.len() + 1;
            for i in (0..scan_end).step_by(inc) {
                let mut found = true;
                for (j, (&pattern_byte, &mask)) in bytes.iter().zip(masks.iter()).enumerate() {
                    if mask && buffer[i + j] != pattern_byte {
                        found = false;
                        break;
                    }
                }
                if found {
                    matches.push(current_addr + i);
                }
            }

            // Move to next chunk, accounting for pattern length to avoid missing matches
            current_addr += bytes_read - bytes.len() + 1;
        }
    }

    if matches.is_empty() {
        Err(format!("Pattern {} not found", pattern).into())
    } else {
        Ok(matches)
    }
}

pub fn write_byte(pid: u32, address: usize, value: u8) -> Result<(), Box<dyn Error>> {
    let handle_wrapper = ProcessHandleWrapper(unsafe {
        OpenProcess(
            PROCESS_VM_WRITE | PROCESS_VM_OPERATION | PROCESS_VM_READ,
            BOOL::from(false),
            pid,
        )
    }.map_err(|e| format!("Failed to open process: {:?}", e))?);

    let mut bytes_written = 0;
    unsafe {
        WriteProcessMemory(
            handle_wrapper.0,
            address as *mut _,
            &value as *const u8 as *const c_void,
            1,
            Some(&mut bytes_written),
        )
    }.map_err(|_| "Failed to write memory")?;

    Ok(())
}