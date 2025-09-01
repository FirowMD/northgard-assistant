use std::error::Error;
use windows::{
    Win32::System::Memory::{
        MEMORY_BASIC_INFORMATION, VirtualQueryEx, MEM_COMMIT, PAGE_EXECUTE,
        PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE, PAGE_EXECUTE_WRITECOPY,
        PAGE_READONLY, PAGE_READWRITE, PAGE_WRITECOPY, VirtualAllocEx,
        MEM_RESERVE, PAGE_PROTECTION_FLAGS, VirtualFreeEx, MEM_RELEASE,
    },
    Win32::Foundation::{HANDLE, BOOL, CloseHandle},
    Win32::System::Threading::{
        OpenProcess, PROCESS_VM_READ, PROCESS_QUERY_INFORMATION,
        PROCESS_VM_WRITE, PROCESS_VM_OPERATION
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

struct ProcessHandleWrapper(HANDLE);

impl Drop for ProcessHandleWrapper {
    fn drop(&mut self) {
        unsafe { let _ = CloseHandle(self.0); }
    }
}

/// Allocate memory region in a process.
/// 
/// Returns information about the allocated memory region.
pub fn allocate_region(pid: u32, size: usize) -> Result<MemoryRegion, Box<dyn Error>> {
    let handle_wrapper = ProcessHandleWrapper(unsafe {
        OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ | PROCESS_VM_WRITE | PROCESS_VM_OPERATION,
            BOOL::from(false),
            pid,
        )
    }.map_err(|e| format!("Failed to open process: {:?}", e))?);

    let address = unsafe {
        VirtualAllocEx(
            handle_wrapper.0,
            None,              // Let the system choose the address
            size,             // Size of the region
            MEM_COMMIT | MEM_RESERVE,  // Allocation type
            PAGE_EXECUTE_READWRITE,    // Memory protection
        )
    };

    if address.is_null() {
        return Err("Failed to allocate memory region".into());
    }

    // Query the allocated region to get full information
    let mut mbi = MEMORY_BASIC_INFORMATION::default();
    let result = unsafe {
        VirtualQueryEx(
            handle_wrapper.0,
            Some(address),
            &mut mbi,
            std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
        )
    };

    if result == 0 {
        return Err("Failed to query allocated memory region".into());
    }

    Ok(MemoryRegion {
        base_address: mbi.BaseAddress as usize,
        allocation_base: mbi.AllocationBase as usize,
        allocation_protect: mbi.AllocationProtect.0,
        region_size: mbi.RegionSize,
        state: mbi.State.0,
        protect: mbi.Protect.0,
        type_: mbi.Type.0,
    })
}

/// Allocate memory region in a process with specified memory protection flags.
/// 
/// Returns information about the allocated memory region.
pub fn allocate_region_mrprotect(pid: u32, size: usize, protection: PAGE_PROTECTION_FLAGS) -> Result<MemoryRegion, Box<dyn Error>> {
    let handle_wrapper = ProcessHandleWrapper(unsafe {
        OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ | PROCESS_VM_WRITE | PROCESS_VM_OPERATION,
            BOOL::from(false),
            pid,
        )
    }.map_err(|e| format!("Failed to open process: {:?}", e))?);

    let address = unsafe {
        VirtualAllocEx(
            handle_wrapper.0,
            None,              // Let the system choose the address
            size,             // Size of the region
            MEM_COMMIT | MEM_RESERVE,  // Allocation type
            protection,        // Memory protection - user specified
        )
    };

    if address.is_null() {
        return Err("Failed to allocate memory region".into());
    }

    // Query the allocated region to get full information
    let mut mbi = MEMORY_BASIC_INFORMATION::default();
    let result = unsafe {
        VirtualQueryEx(
            handle_wrapper.0,
            Some(address),
            &mut mbi,
            std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
        )
    };

    if result == 0 {
        return Err("Failed to query allocated memory region".into());
    }

    Ok(MemoryRegion {
        base_address: mbi.BaseAddress as usize,
        allocation_base: mbi.AllocationBase as usize,
        allocation_protect: mbi.AllocationProtect.0,
        region_size: mbi.RegionSize,
        state: mbi.State.0,
        protect: mbi.Protect.0,
        type_: mbi.Type.0,
    })
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

/// Allocate memory region close to specified address.
/// Attempts to find free memory region within ±2GB range for relative jumps.
/// 
/// Returns information about the allocated memory region.
pub fn allocate_region_near(
    pid: u32, 
    target_address: usize, 
    size: usize,
    protection: PAGE_PROTECTION_FLAGS
) -> Result<MemoryRegion, Box<dyn Error>> {
    use windows::Win32::System::Memory::{VirtualAllocEx, MEM_COMMIT, MEM_RESERVE};

    // Try to allocate within ±2GB range of target address to ensure RIP-relative addressing works
    const MAX_DISTANCE: i64 = 0x7FFF_0000; // ~2GB
    
    let handle = unsafe {
        OpenProcess(
            PROCESS_VM_OPERATION,
            BOOL::from(false),
            pid,
        )
    }.map_err(|e| format!("Failed to open process: {:?}", e))?;

    // Try multiple addresses starting from closest to target
    let mut offset = 0;
    while offset < MAX_DISTANCE {
        // Try both above and below target address
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
                let allocated = MemoryRegion {
                    base_address: ptr as usize,
                    region_size: size,
                    allocation_base: ptr as usize,
                    allocation_protect: protection.0,
                    state: MEM_COMMIT.0,
                    protect: protection.0,
                    type_: 0x20000,  // MEM_PRIVATE
                };

                // Verify the allocation is within range
                let distance = (allocated.base_address as i64) - (target_address as i64);
                if distance.abs() < MAX_DISTANCE {
                    return Ok(allocated);
                }

                // If not within range, free it and continue searching
                let _ = unsafe { VirtualFreeEx(handle, ptr, 0, MEM_RELEASE) };
            }
        }

        // Increase offset by page size
        offset += 0x1000;
    }

    // Fall back to regular allocation if we couldn't find suitable address
    tracing::warn!("Could not allocate memory near target region, falling back to regular allocation");
    allocate_region(pid, size)
}
