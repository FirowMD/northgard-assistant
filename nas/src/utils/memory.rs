use std::error::Error;
use windows::{
    Win32::System::Memory::{
        MEMORY_BASIC_INFORMATION, VirtualQueryEx, MEM_COMMIT, PAGE_EXECUTE,
        PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE, PAGE_EXECUTE_WRITECOPY,
        PAGE_READONLY, PAGE_READWRITE, PAGE_WRITECOPY,
    },
    Win32::Foundation::{HANDLE, BOOL, CloseHandle},
    Win32::System::Threading::{
        OpenProcess, PROCESS_VM_READ, PROCESS_QUERY_INFORMATION,
    },
};

/// Represents information about a memory region in a process.
/// 
/// This structure contains details about a memory region's location,
/// size, and protection attributes as returned by Windows memory management functions.
#[derive(Debug)]
pub struct MemoryRegion {
    /// The base address of the region of pages.
    pub base_address: usize,

    /// The base address of the allocated region of pages when the region was initially allocated.
    pub allocation_base: usize,

    /// The memory protection option when the region was initially allocated.
    /// 
    /// Can be a combination of:
    /// - PAGE_EXECUTE
    /// - PAGE_EXECUTE_READ
    /// - PAGE_EXECUTE_READWRITE
    /// - PAGE_READONLY
    /// - PAGE_READWRITE
    pub allocation_protect: u32,

    /// The size of the region in bytes.
    pub region_size: usize,

    /// The state of the pages in the region.
    /// 
    /// Can be one of:
    /// - MEM_COMMIT
    /// - MEM_FREE
    /// - MEM_RESERVE
    pub state: u32,

    /// The access protection of the pages in the region.
    /// 
    /// Can be a combination of:
    /// - PAGE_EXECUTE
    /// - PAGE_EXECUTE_READ
    /// - PAGE_EXECUTE_READWRITE
    /// - PAGE_READONLY
    /// - PAGE_READWRITE
    pub protect: u32,

    /// The type of pages in the region.
    /// 
    /// Can be one of:
    /// - MEM_IMAGE (Memory mapped EXE/DLL)
    /// - MEM_MAPPED (Memory mapped file)
    /// - MEM_PRIVATE (Private memory)
    pub type_: u32,
}


/// Enumerate the memory regions of the given process.
/// 
/// Returns a list of memory regions.
pub fn enum_memory_regions(pid: u32) -> Result<Vec<MemoryRegion>, Box<dyn Error>> {
    let process_handle = unsafe {
        OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
            BOOL::from(false),
            pid,
        )
    };

    let handle = process_handle.map_err(|e| format!("Failed to open process: {:?}", e))?;

    let regions = enum_memory_regions_handle(handle);

    unsafe {
        let _ = CloseHandle(handle);
    }

    Ok(regions)
}

fn enum_memory_regions_handle(process_handle: HANDLE) -> Vec<MemoryRegion> {
    let mut regions = Vec::new();
    let mut address: usize = 0;

    loop {
        let mut mbi = MEMORY_BASIC_INFORMATION::default();
        
        let result = unsafe {
            VirtualQueryEx(
                process_handle,
                Some(address as *const _),
                &mut mbi,
                std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        };

        if result == 0 {
            break;
        }

        if mbi.State.0 == MEM_COMMIT.0 {
            regions.push(MemoryRegion {
                base_address: mbi.BaseAddress as usize,
                allocation_base: mbi.AllocationBase as usize,
                allocation_protect: mbi.AllocationProtect.0,
                region_size: mbi.RegionSize,
                state: mbi.State.0,
                protect: mbi.Protect.0,
                type_: mbi.Type.0,
            });
        }

        if let Some(next_address) = (address as usize).checked_add(mbi.RegionSize) {
            address = next_address as usize;
        } else {
            break;
        }
    }

    regions
}

pub fn get_protection_string(protect: u32) -> String {
    let mut rights = Vec::new();

    match protect {
        p if p & PAGE_EXECUTE.0 != 0 => rights.push("X"),
        p if p & PAGE_EXECUTE_READ.0 != 0 => rights.extend_from_slice(&["R", "X"]),
        p if p & PAGE_EXECUTE_READWRITE.0 != 0 => rights.extend_from_slice(&["R", "W", "X"]),
        p if p & PAGE_EXECUTE_WRITECOPY.0 != 0 => rights.extend_from_slice(&["R", "W", "X", "C"]),
        p if p & PAGE_READONLY.0 != 0 => rights.push("R"),
        p if p & PAGE_READWRITE.0 != 0 => rights.extend_from_slice(&["R", "W"]),
        p if p & PAGE_WRITECOPY.0 != 0 => rights.extend_from_slice(&["R", "W", "C"]),
        _ => rights.push("---"),
    }

    rights.join("")
}

pub fn print_memory_regions(pid: u32) -> Result<(), Box<dyn Error>> {
    let regions = enum_memory_regions(pid)?;
    
    println!("Memory Regions:");
    for region in regions {
        println!(
            "Base: {:#016x}, Size: {:#x}, Protection: {} ({})",
            region.base_address,
            region.region_size,
            region.protect,
            get_protection_string(region.protect)
        );
    }

    Ok(())
}

// Allocation helpers have moved to libmem_ex.rs.
