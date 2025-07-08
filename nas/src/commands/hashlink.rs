/* 
    Singleton that allows to use hashlink functions through a simple interface
*/

use crate::commands::basic::*;
use std::error::Error;
use std::sync::{Mutex, Once};
use windows::Win32::System::Memory::{PAGE_EXECUTE, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE, PAGE_READWRITE};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};
use windows::Win32::System::ProcessStatus::K32GetModuleFileNameExW;
use std::path::PathBuf;
use std::result::Result;
use std::fs::File;
use std::io::Write;

pub struct HLFunction {
    pub name: String,
    pub address: usize,
}

pub struct Hashlink {
    pub pid: u32,
    pub address_allocstring: usize,
    pub functions: Vec<HLFunction>,
    pub hashlink_version: u32,
    pub hlbootdat_address: usize,
    pub structure_address: usize,
}

impl Hashlink {
    pub fn new(pid: u32) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            pid,
            address_allocstring: 0,
            functions: Vec::new(),
            hlbootdat_address: 0,
            structure_address: 0,
            hashlink_version: 0,
        })
    }

    pub fn instance(pid: u32) -> &'static Mutex<Option<Hashlink>> {
        static SINGLETON: Mutex<Option<Hashlink>> = Mutex::new(None);
        static INIT: Once = Once::new();
        
        INIT.call_once(|| {
            if let Ok(mut hashlink) = Hashlink::new(pid) {
                // Log initialization attempt
                tracing::info!("Initializing Hashlink singleton for PID: {}", pid);
                match hashlink.init_hashlink() {
                    Ok(_) => {
                        tracing::info!("Successfully initialized Hashlink");
                        *SINGLETON.lock().unwrap() = Some(hashlink);
                    }
                    Err(e) => {
                        tracing::error!("Failed to initialize Hashlink: {}", e);
                    }
                }
            } else {
                tracing::error!("Failed to create new Hashlink instance");
            }
        });
        &SINGLETON
    }

    fn find_hlbootdat_address(&mut self) -> Result<(), Box<dyn Error>> {
        const EXPECTED_STRING: &str = "hlboot.dat";
        const IMAGE_TYPE: u32 = 0x1000000;

        let mut pattern = String::new();
        for c in EXPECTED_STRING.encode_utf16() {
            pattern.push_str(&format!("{:02x} {:02x} ", c as u8, (c >> 8) as u8));
        }
        pattern.pop();

        let addrs = aob_scan_mrtype(self.pid, &pattern, IMAGE_TYPE)?;
        self.hlbootdat_address = addrs[0];
        Ok(())
    }

    fn setup_hashlink_version(&mut self) -> bool {
        let addr = match read_qword(self.pid, self.structure_address + 0x8) {
            Ok(addr) => {
                addr
            },
            Err(_) => return false
        };

        let current_value = match read_dword(self.pid, addr as usize) {
            Ok(val) => {
                val
            },
            Err(_) => return false
        };

        let possible_values = [3, 4, 5];
        for value in possible_values {
            if current_value == value {
                tracing::info!("Hashlink version: {}", value);
                self.hashlink_version = value;
                return true;
            }
        }
        false
    }

    fn get_structure_address(&mut self) -> Result<(), Box<dyn Error>> {
        let mut pattern = String::new();
        for byte in self.hlbootdat_address.to_le_bytes() {
            pattern.push_str(&format!("{:02x} ", byte));
        }
        pattern.pop();

        let executable_protection = PAGE_READWRITE.0;
        let addrs = aob_scan_mrprotect(self.pid, &pattern, executable_protection)?;
        for addr in addrs {
            self.structure_address = addr;
            if self.setup_hashlink_version() {
                return Ok(());
            }
        }
        Err("No valid structure address found".into())
    }

    fn get_nfunctions(&self) -> Result<u32, Box<dyn Error>> {
        let mut nfunctions_offset: usize = 28;
        if self.hashlink_version >= 4 {
            nfunctions_offset = 32;
        }

        let addr = read_pointer(self.pid, self.structure_address + 0x8)?;
        let nfunctions = read_dword(self.pid, addr + nfunctions_offset)?;
        Ok(nfunctions)
    }

    fn get_function_list(&self, nfunctions: u32) -> Result<Vec<usize>, Box<dyn Error>> {
        let mut result = Vec::new();
        let hl_module_pointer = read_pointer(self.pid, self.structure_address + 0x10)?;
        let functions_pointer = read_pointer(self.pid, hl_module_pointer + 0x20)?;

        let bytes = read_bytes(self.pid, functions_pointer, (nfunctions * 8) as usize)?;

        for i in 0..nfunctions as usize {
            let func = byte_array_to_pointer(&bytes[i * 8..(i + 1) * 8]);
            result.push(func);
        }

        Ok(result)
    }

    pub fn get_directory(pid: u32) -> Result<String, Box<dyn Error>> {
        let handle = unsafe {
            OpenProcess(
                PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
                false,
                pid
            )
        }.map_err(|e| format!("Failed to open process: {:?}", e))?;

        let mut buffer = [0u16; 260];
        let length = unsafe {
            K32GetModuleFileNameExW(
                handle,
                None,
                &mut buffer,
            )
        };

        if length == 0 {
            return Err("Failed to get module filename".into());
        }

        let path = String::from_utf16_lossy(&buffer[..length as usize]);
        let path_buf = PathBuf::from(path);
        
        if let Some(parent) = path_buf.parent() {
            Ok(parent.to_string_lossy().into_owned())
        } else {
            Err("Failed to get parent directory".into())
        }
    }

    pub fn init_hashlink(&mut self) -> Result<usize, Box<dyn Error>> {
        
        // Find allocString function
        let hex_pattern_allocstring = "55 48 8B ?? 48 83 ?? ?? 48 89 ?? ?? 89 ?? ?? 48 B9 ?? ?? ?? ?? ?? ?? ?? ?? 48 B8 ?? ?? ?? ?? ?? ?? ?? ?? 48 83 ?? ?? FF ?? 48 89 ?? ?? ?? 48 83 ?? ?? 48 89 ?? ?? 48 8B ?? ?? 48 89 ?? ?? 8B ?? ?? 89 ?? ?? 48 83 ?? ?? 5D 48 C3";
        let executable_protection = PAGE_EXECUTE.0 | PAGE_EXECUTE_READ.0 | PAGE_EXECUTE_READWRITE.0;
        let addrs = aob_scan_mrprotect(self.pid, hex_pattern_allocstring, executable_protection)?;
        if addrs.is_empty() {
            return Err("Pattern not found: init_hashlink: hex_pattern_allocstring".into());
        }
        self.address_allocstring = addrs[0];

        // Initialize other addresses
        self.find_hlbootdat_address()?;
        tracing::info!("HLBootdat address: {:#016x}", self.hlbootdat_address);

        self.get_structure_address()?;
        tracing::info!("Structure address: {:#016x}", self.structure_address);

        // Get functions
        let nfunctions = self.get_nfunctions()?;
        tracing::info!("Function number: {}", nfunctions);

        let function_list = self.get_function_list(nfunctions)?;

        // Load hlboot.dat and match functions
        let directory = Self::get_directory(self.pid)?;
        let path = PathBuf::from(directory).join("hlboot.dat");
        tracing::info!("File path: {}", path.to_string_lossy());
        let bytecode = hlbc::Bytecode::from_file(path)?;

        for function in &bytecode.functions {
            let name = function.name(&bytecode).to_string();
            if function.findex.0 < function_list.len() {
                let address = function_list[function.findex.0];
                self.functions.push(HLFunction { name, address });
            }
        }

        Ok(self.address_allocstring)
    }

    pub fn save_functions(&self, path: &str) -> Result<(), Box<dyn Error>> {
        let mut file = File::create(path)?;
        for function in &self.functions {
            writeln!(file, "{:#016x} {}", function.address, function.name)?;
        }
        Ok(())
    }

    pub fn get_function_address(&self, name: &str, idx: Option<usize>) -> Result<usize, Box<dyn Error>> {
        let mut found_count = 0;
        let target_idx = idx.unwrap_or(0);

        for function in &self.functions {
            if function.name == name {
                if found_count == target_idx {
                    return Ok(function.address);
                }
                found_count += 1;
            }
        }

        if found_count == 0 {
            Err(format!("Function '{}' not found", name).into())
        } else {
            Err(format!("Function '{}' index {} not found (max index: {})", 
                name, target_idx, found_count - 1).into())
        }
    }
}