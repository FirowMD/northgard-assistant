/*
    Injects to ssl_new and get_premade.
    - ssl_new - to obtain socket handle
    - get_premade - to execute our code, which will receive actual player ranks
*/

use crate::modules::base::{Command, CommandContext, InjectionManager};
use crate::modules::mem_alloc::{MemoryAllocator, DataType};
use crate::utils::memory::{read_bytes, write_byte};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_VM_READ};
use windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;
use windows::Win32::Foundation::BOOL;
use iced_x86::code_asm::*;
use std::error::Error;
use std::collections::HashMap;
use std::sync::Mutex;
use windows::Win32::Foundation::HANDLE;

const MAX_LB_SEND_BUFSIZE: usize = 0x1000;
const MAX_LB_RECV_BUFSIZE: usize = 0x5000;
const MAX_LB_OPLAYER_COUNT: usize = 0x2000;
const LB_OPLAYER_COUNT_DEFAULT: usize = 40;

#[derive(Clone, Copy)]
pub enum LeaderboardType {
    Duels,
    FreeForAll,
    Teams,
}

pub struct LeaderboardScores {
    pid: u32,
    address_ssl_send: usize,
    address_ssl_recv: usize,
    address_ssl_new: usize,
    address_get_premade: usize,
    injection_manager: InjectionManager,
    enabled: bool,
    
    // Buffer management
    var_ptr_sendbuffer: usize,
    var_ptr_recvbuffer: usize,
    var_ptr_recvbufsize: usize,
    var_ptr_socket: usize,

    sendbuf_size: usize,
    
    // Adjustable by user
    observe_player_count: i32,
    leaderboard_type: LeaderboardType,
}

impl LeaderboardScores {
    pub fn new(pid: u32) -> Result<Self, Box<dyn Error>> {
        let mut memory_allocator = MemoryAllocator::new(pid, 0x10000)?;
        let var_ptr_sendbuffer = memory_allocator.allocate_var_with_size("SendBuffer", DataType::ByteArray, MAX_LB_SEND_BUFSIZE)?;
        let var_ptr_recvbuffer = memory_allocator.allocate_var_with_size("RecvBuffer", DataType::ByteArray, MAX_LB_RECV_BUFSIZE)?;
        let var_ptr_recvbufsize = memory_allocator.allocate_var("RecvBufSize", DataType::I32)?;
        memory_allocator.write_var("RecvBufSize", MAX_LB_RECV_BUFSIZE)?;
        let var_ptr_socket = memory_allocator.allocate_var("Socket", DataType::Pointer)?;

        let mut injection_manager = InjectionManager::new(pid);
        injection_manager.add_injection("get_premade".to_string());
        injection_manager.add_injection("ssl_new".to_string());

        let mut leaderboard_scores = Self {
            pid,
            address_ssl_send: 0,
            address_ssl_recv: 0,
            address_ssl_new: 0,
            address_get_premade: 0,
            injection_manager,
            enabled: false,
            var_ptr_sendbuffer,
            var_ptr_recvbuffer,
            var_ptr_recvbufsize,
            var_ptr_socket,
            sendbuf_size: MAX_LB_SEND_BUFSIZE,
            observe_player_count: LB_OPLAYER_COUNT_DEFAULT,
            leaderboard_type: LeaderboardType::Duels,
        };

        leaderboard_scores.init()?;

        Ok(leaderboard_scores)
    }

    pub fn init(&mut self) -> Result<(), Box<dyn Error>> {
        let guard = Hashlink::instance(self.pid).lock().unwrap();
        if let Some(hashlink) = guard.as_ref() {
            self.address_get_premade = hashlink.get_function_address("get_premade", Some(0))?;
        }

        let hex_pattern_ssl_send = "48 83 ?? ?? 49 63 ?? 48 03 ?? 4D ?? ?? E8 ?? ?? ?? ?? 85";
        let hex_pattern_ssl_recv = "48 83 ?? ?? 49 63 ?? 48 03 ?? 4D ?? ?? E8 ?? ?? ?? ?? 3D";
        let hex_pattern_ssl_new = "48 89 ?? ?? ?? 57 48 83 ?? ?? 48 8B ?? BA ?? ?? ?? ?? 48 8B ?? ?? ?? ?? ?? 41 ?? ?? ?? ?? ?? FF ?? ?? ?? ?? ?? 33 ?? 41 ?? ?? ?? ?? ?? 48 8B ?? 48 8B ?? E8 ?? ?? ?? ?? 48 8B ?? 48 8B ?? E8 ?? ?? ?? ?? 8B ?? 85 ?? 75 ?? 48 8B ?? ?? ?? 48 8B ?? 48 83 ?? ?? 5F C3 48 8B ?? E8 ?? ?? ?? ?? 8B ?? E8 ?? ?? ?? ??";

        let executable_protection = PAGE_EXECUTE.0 | PAGE_EXECUTE_READ.0 | PAGE_EXECUTE_READWRITE.0;

        let addr_ssl_send = aob_scan_mrprotect(self.pid, hex_pattern_ssl_send, executable_protection)?;
        if addr_ssl_send.is_empty() {
            return Err("Pattern not found: ssl_send".into());
        }

        let addr_ssl_recv = aob_scan_mrprotect(self.pid, hex_pattern_ssl_recv, executable_protection)?;
        if addr_ssl_recv.is_empty() {
            return Err("Pattern not found: ssl_recv".into());
        }

        let addr_ssl_new = aob_scan_mrprotect(self.pid, hex_pattern_ssl_new, executable_protection)?;
        if addr_ssl_new.is_empty() {
            return Err("Pattern not found: ssl_new".into());
        }

        let SSL_NEW_RET_OFFSET = 86;

        self.address_ssl_send = addr_ssl_send[0];
        self.address_ssl_recv = addr_ssl_recv[0];
        self.address_ssl_new = addr_ssl_new[0] + SSL_NEW_RET_OFFSET;

        Ok(())
    }

    pub fn apply(&mut self) -> Result<(), Box<dyn Error>> {
        /*
        Injection: ssl_new

            1. Just put result of ssl_new into our variable (socket handle)
        */
        self.injection_manager.remove_injection("get_premade")?;
        self.injection_manager.remove_injection("ssl_new")?;

        let mut code = CodeAssembler::new(64)?;

        code.pushfq()?;
        code.push(rax)?;
        code.push(rbx)?;

        code.mov(rbx, self.var_ptr_socket as u64)?;
        code.mov(qword_ptr(rbx), rax)?;

        code.pop(rbx)?;
        code.pop(rax)?;
        code.popfq()?;

        self.injection_manager.apply_injection("ssl_new", self.address_ssl_new, &mut code)?;

        /*
        Injection: get_premade

            1. Call ssl_send
            2. Call ssl_recv until rax is not 0 or -1 (0xFFFFFFFFFFFFFFFF)
            3. Everytime ssl_recv is called we need to store 
        */

        let mut code2 = CodeAssembler::new(64)?;
        let mut label_loop = code2.create_label();

        code2.pushfq()?;
        code2.push(rax)?;
        code2.push(rbx)?;
        code2.push(rcx)?;
        code2.push(rdx)?;
        code2.push(r8)?;
        code2.push(r9)?;
        code2.push(r10)?;

        // ssl/ssl_send@49013 (mbedtls_ssl_context, bytes, i32, i32) -> i32

        code2.mov(rbx, self.var_ptr_socket as u64)?;
        code2.mov(rcx, qword_ptr(rbx))?; // mbedtls_ssl_context

        code2.mov(rbx, self.var_ptr_sendbuffer as u64)?;
        code2.mov(rdx, qword_ptr(rbx))?; // bytes

        code2.mov(r8, 0)?; // offset
        code2.mov(r9, self.sendbuf_size)?; // length

        code2.mov(rax, self.address_ssl_send as u64)?;
        code2.call(rax)?;

        code2.mov(r10, 0)?;
        code2.set_label(&mut label_loop)?;
        
        // ssl/ssl_recv@49012 (mbedtls_ssl_context, bytes, i32, i32) -> i32
        code2.mov(rbx, self.var_ptr_socket as u64)?;
        code2.mov(rcx, qword_ptr(rbx))?;

        code2.mov(rbx, self.var_ptr_recvbuffer as u64)?;
        code2.mov(rdx, qword_ptr(rbx))?;

        //! Check if we need to adjust `offset`
        code2.mov(r8, r10);

        code2.mov(rbx, self.var_ptr_recvbufsize)?;
        code2.mov(r9, qword_ptr(rbx))?;

        code2.mov(rax, self.address_ssl_recv)?;
        code2.call(rax)?;

        code2.add(r10, rax)?;
        code2.cmp(rax, 0)?;
        code2.jg(label_loop)?;

        code2.pop(r10)?;
        code2.pop(r9)?;
        code2.pop(r8)?;
        code2.pop(rdx)?;
        code2.pop(rcx)?;
        code2.pop(rbx)?;
        code2.pop(rax)?;
        code2.popfq()?;

        self.injection_manager.apply_injection("get_premade", self.address_get_premade, &mut code2)?;
    }

    pub fn get_recv_buffer(&self) -> Result<Vec<u8>, Box<dyn Error>> {
        let handle = unsafe {
            OpenProcess(
                PROCESS_VM_READ,
                BOOL::from(false),
                self.pid,
            )
        }.map_err(|e| format!("Failed to open process: {:?}", e))?;

        let mut buffer = vec![0u8; MAX_LB_RECV_BUFSIZE];
        let mut bytes_read = 0;
        
        unsafe {
            ReadProcessMemory(
                handle,
                self.var_ptr_recvbuffer as *const _,
                buffer.as_mut_ptr() as *mut _,
                MAX_LB_RECV_BUFSIZE,
                Some(&mut bytes_read),
            )
        }.map_err(|_| "Failed to read receive buffer")?;

        unsafe { let _ = windows::Win32::Foundation::CloseHandle(handle); }
        
        buffer.truncate(bytes_read);
        Ok(buffer)
    }

    pub fn set_leaderboard_type(&mut self, lb_type: LeaderboardType) -> Result<(), Box<dyn Error>> {
        self.leaderboard_type = lb_type;
        
        let request_data = match lb_type {
            LeaderboardType::Duels => b"GET /api/leaderboard/duels HTTP/1.1\r\nHost: northgard.com\r\n\r\n",
            LeaderboardType::FreeForAll => b"GET /api/leaderboard/ffa HTTP/1.1\r\nHost: northgard.com\r\n\r\n",
            LeaderboardType::Teams => b"GET /api/leaderboard/teams HTTP/1.1\r\nHost: northgard.com\r\n\r\n",
        };
        
        self.sendbuf_size = request_data.len();
        
        let handle = unsafe {
            OpenProcess(
                windows::Win32::System::Threading::PROCESS_VM_WRITE | windows::Win32::System::Threading::PROCESS_VM_OPERATION,
                BOOL::from(false),
                self.pid,
            )
        }.map_err(|e| format!("Failed to open process: {:?}", e))?;

        let mut bytes_written = 0;
        unsafe {
            windows::Win32::System::Diagnostics::Debug::WriteProcessMemory(
                handle,
                self.var_ptr_sendbuffer as *mut _,
                request_data.as_ptr() as *const _,
                request_data.len(),
                Some(&mut bytes_written),
            )
        }.map_err(|_| "Failed to write send buffer")?;

        unsafe { let _ = windows::Win32::Foundation::CloseHandle(handle); }
        
        if bytes_written != request_data.len() {
            return Err("Incomplete write to send buffer".into());
        }
        
        Ok(())
    }

    pub fn set_observable_player_count(&mut self, oplayer_count: i32) -> Result<(), Box<dyn Error>> {
        if oplayer_count <= 0 {
            return Err("Observable player count must be positive".into());
        }
        
        if oplayer_count > MAX_LB_OPLAYER_COUNT as i32 {
            return Err(format!("Observable player count cannot exceed {}", MAX_LB_OPLAYER_COUNT).into());
        }
        
        self.observe_player_count = oplayer_count;
        Ok(())
    }
}