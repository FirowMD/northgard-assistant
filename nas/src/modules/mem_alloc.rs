use crate::utils::libmem_ex::{allocate_region_mrprotect, write_bytes_ex, read_bytes_ex, free, get_target_process};
use windows::Win32::System::Memory::PAGE_READWRITE;
use std::error::Error;
use libmem::Process;
use std::collections::HashMap;

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
    allocated_addr: usize,
    allocated_size: usize,
    next_free_addr: usize,
    next_free_size: usize,
    variables: HashMap<String, Variable>,
    process: Process,
}

impl MemoryAllocator {
    pub fn new(pid: u32, total_size: usize) -> Result<Self, Box<dyn Error>> {
        let process = get_target_process(pid).ok_or("Failed to get process with libmem")?;
        let allocated = allocate_region_mrprotect(pid, total_size, PAGE_READWRITE)?;
        
        Ok(Self {
            pid,
            allocated_addr: allocated.base_address,
            allocated_size: allocated.region_size,
            next_free_addr: allocated.base_address,
            next_free_size: allocated.region_size,
            variables: HashMap::new(),
            process,
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

        // Write the string bytes
        if !str.is_empty() {
            write_bytes_ex(&self.process, addr, str.as_bytes()).ok_or("libmem write_bytes_ex failed")?;
        }
        // Null terminator
        write_bytes_ex(&self.process, addr + size - 1, &[0u8]).ok_or("libmem write_bytes_ex failed")?;

        Ok(addr)
    }

    /// Allocates a wide string
    pub fn allocate_wide_string(&mut self, name: &str, str: &str) -> Result<usize, Box<dyn Error>> {
        let size = str.len() * 2 + 2;
        let addr = self.allocate_var_with_size(name, DataType::ByteArray, size)?;

        // Write UTF-16LE bytes
        for (i, ch) in str.encode_utf16().enumerate() {
            let bytes = ch.to_le_bytes();
            write_bytes_ex(&self.process, addr + i * 2, &bytes).ok_or("libmem write_bytes_ex failed")?;
        }
        // Null terminator (2 bytes)
        write_bytes_ex(&self.process, addr + size - 2, &[0u8, 0u8]).ok_or("libmem write_bytes_ex failed")?;

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

        let size = std::mem::size_of::<T>();
        let bytes = unsafe { std::slice::from_raw_parts(&value as *const T as *const u8, size) };
        write_bytes_ex(&self.process, var.address, bytes).ok_or("libmem write_bytes_ex failed")?;

        Ok(())
    }

    /// Write byte array to a variable
    pub fn write_byte_array(&self, name: &str, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
        let var = self.variables.get(name)
            .ok_or("Variable not found")?;

        if bytes.len() > var.size {
            return Err("Byte array too large for allocated variable".into());
        }

        write_bytes_ex(&self.process, var.address, bytes).ok_or("libmem write_bytes_ex failed")?;

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

        let bytes = read_bytes_ex(&self.process, var.address, var.size).ok_or("libmem read_bytes_ex failed")?;
        if bytes.len() != var.size { return Err("Incomplete read".into()); }

        let mut out = std::mem::MaybeUninit::<T>::uninit();
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), out.as_mut_ptr() as *mut u8, var.size);
            Ok(out.assume_init())
        }
    }

    /// Get variable address by name
    pub fn get_var_address(&self, name: &str) -> Option<usize> {
        self.variables.get(name).map(|var| var.address)
    }

    /// Free all allocated memory
    pub fn free(&self) -> Result<(), Box<dyn Error>> {
        free(&self.process, self.allocated_addr, self.allocated_size).ok_or("libmem free failed")?;

        Ok(())
    }
}