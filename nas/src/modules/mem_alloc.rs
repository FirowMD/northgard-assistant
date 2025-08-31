use crate::utils::memory::{MemoryRegion, allocate_region};
use crate::modules::basic::*;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_VM_WRITE, PROCESS_VM_OPERATION, PROCESS_VM_READ};
use windows::Win32::System::Diagnostics::Debug::{WriteProcessMemory, ReadProcessMemory};
use windows::Win32::Foundation::{HANDLE, BOOL, CloseHandle};
use std::error::Error;
use std::ffi::c_void;
use std::collections::HashMap;

struct ProcessHandleWrapper(HANDLE);

impl Drop for ProcessHandleWrapper {
    fn drop(&mut self) {
        unsafe { let _ = CloseHandle(self.0); }
    }
}

pub struct Variable {
    pub name: String,
    pub address: usize,
    pub size: usize,
    pub data_type: DataType,
}

#[derive(Clone, Copy)]
pub enum DataType {
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
    Bool,
    Pointer,
    ByteArray,
}

impl DataType {
    fn size(&self) -> usize {
        match self {
            DataType::U8 | DataType::I8 | DataType::Bool => 1,
            DataType::U16 | DataType::I16 => 2,
            DataType::U32 | DataType::I32 | DataType::F32 => 4,
            DataType::U64 | DataType::I64 | DataType::F64 => 8,
            DataType::Pointer => 8,
            DataType::ByteArray => 1,
        }
    }
}

pub struct MemoryAllocator {
    pid: u32,
    allocated_region: MemoryRegion,
    next_free_addr: usize,
    next_free_size: usize,
    variables: HashMap<String, Variable>,
}

impl MemoryAllocator {
    pub fn new(pid: u32, total_size: usize) -> Result<Self, Box<dyn Error>> {
        let allocated = allocate_region(pid, total_size)?;
        
        Ok(Self {
            pid,
            next_free_addr: allocated.base_address,
            next_free_size: allocated.region_size,
            allocated_region: allocated,
            variables: HashMap::new(),
        })
    }

    /// Allocate space for a new variable
    pub fn allocate_var(&mut self, name: &str, data_type: DataType) -> Result<usize, Box<dyn Error>> {
        let size = data_type.size();
        
        if size > self.next_free_size {
            return Err("Not enough space for variable".into());
        }

        if self.variables.contains_key(name) {
            return Err("Variable already exists".into());
        }

        let var = Variable {
            name: name.to_string(),
            address: self.next_free_addr,
            size,
            data_type,
        };

        let addr = var.address;
        self.next_free_addr += size;
        self.next_free_size -= size;
        
        self.variables.insert(name.to_string(), var);
        
        Ok(addr)
    }
    /// Allocate space for a new variable with opportunity to specify the size
    pub fn allocate_var_with_size(&mut self, name: &str, data_type: DataType, size: usize) -> Result<usize, Box<dyn Error>> {
        if size > self.next_free_size {
            return Err("Not enough space for variable".into());
        }

        if self.variables.contains_key(name) {
            return Err("Variable already exists".into());
        }

        let var = Variable {
            name: name.to_string(),
            address: self.next_free_addr,
            size,
            data_type,
        };

        let addr = var.address;
        self.next_free_addr += size;
        self.next_free_size -= size;
        
        self.variables.insert(name.to_string(), var);
        
        Ok(addr)
    }

    /// Allocates a ascii string
    pub fn allocate_string(&mut self, name: &str, str: &str) -> Result<usize, Box<dyn Error>> {
        let size = str.len() + 1;
        let addr = self.allocate_var_with_size(name, DataType::ByteArray, size)?;

        // Write the string to the allocated memory using `write_byte`
        for (i, byte) in str.as_bytes().iter().enumerate() {
            write_byte(self.pid, addr + i, *byte)?;
        }

        // Place null terminator at the end
        write_byte(self.pid, addr + size - 1, 0)?;

        Ok(addr)
    }

    /// Allocates a wide string
    pub fn allocate_wide_string(&mut self, name: &str, str: &str) -> Result<usize, Box<dyn Error>> {
        let size = str.len() * 2 + 2;
        let addr = self.allocate_var_with_size(name, DataType::ByteArray, size)?;

        // Write the string to the allocated memory using `write_utf16_string`
        for (i, byte) in str.as_bytes().iter().enumerate() {
            write_byte(self.pid, addr + i * 2, *byte)?;
            write_byte(self.pid, addr + i * 2 + 1, 0)?;
        }

        // Place null terminator at the end
        write_byte(self.pid, addr + size - 1, 0)?;
        write_byte(self.pid, addr + size - 2, 0)?;

        Ok(addr)
    }

    /// Write value to a variable
    pub fn write_var<T>(&self, name: &str, value: T) -> Result<(), Box<dyn Error>> 
    where T: Sized {
        let var = self.variables.get(name)
            .ok_or("Variable not found")?;

        if std::mem::size_of::<T>() != var.size {
            return Err("Type size mismatch".into());
        }

        let handle_wrapper = ProcessHandleWrapper(unsafe {
            OpenProcess(
                PROCESS_VM_WRITE | PROCESS_VM_OPERATION,
                BOOL::from(false),
                self.pid,
            )
        }.map_err(|e| format!("Failed to open process: {:?}", e))?);

        let mut bytes_written = 0;
        unsafe {
            WriteProcessMemory(
                handle_wrapper.0,
                var.address as *mut _,
                &value as *const T as *const c_void,
                var.size,
                Some(&mut bytes_written),
            )
        }.map_err(|_| "Failed to write memory")?;

        if bytes_written != var.size {
            return Err("Incomplete write".into());
        }

        Ok(())
    }

    /// Read value from a variable
    pub fn read_var<T>(&self, name: &str) -> Result<T, Box<dyn Error>> 
    where T: Default + Sized {
        let var = self.variables.get(name)
            .ok_or("Variable not found")?;

        if std::mem::size_of::<T>() != var.size {
            return Err("Type size mismatch".into());
        }

        let handle_wrapper = ProcessHandleWrapper(unsafe {
            OpenProcess(
                PROCESS_VM_READ,
                BOOL::from(false),
                self.pid,
            )
        }.map_err(|e| format!("Failed to open process: {:?}", e))?);

        let mut value: T = T::default();
        let mut bytes_read = 0;

        unsafe {
            ReadProcessMemory(
                handle_wrapper.0,
                var.address as *const _,
                &mut value as *mut T as *mut c_void,
                var.size,
                Some(&mut bytes_read),
            )
        }.map_err(|_| "Failed to read memory")?;

        if bytes_read != var.size {
            return Err("Incomplete read".into());
        }

        Ok(value)
    }

    /// Get variable address by name
    pub fn get_var_address(&self, name: &str) -> Option<usize> {
        self.variables.get(name).map(|var| var.address)
    }

    /// Free all allocated memory
    pub fn free(&self) -> Result<(), Box<dyn Error>> {
        unsafe {
            use windows::Win32::System::Memory::{VirtualFreeEx, MEM_RELEASE};
            
            let handle_wrapper = ProcessHandleWrapper(
                OpenProcess(
                    PROCESS_VM_OPERATION,
                    BOOL::from(false),
                    self.pid,
                )
            .map_err(|e| format!("Failed to open process: {:?}", e))?);

            VirtualFreeEx(
                handle_wrapper.0,
                self.allocated_region.base_address as *mut c_void,
                0,
                MEM_RELEASE,
            ).map_err(|_| "Failed to free memory")?;
        }

        Ok(())
    }
}