/*
    Injects to ssl_send and get_premade.
    - ssl_send - to obtain socket handle
    - get_premade - to execute our code, which will receive actual player ranks
*/

use crate::modules::base::InjectionManager;
use crate::modules::mem_alloc::{MemoryAllocator, DataType};
use crate::modules::hashlink::*;
use crate::modules::basic::aob_scan_mrprotect;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_VM_READ};
use windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;
use windows::Win32::System::Memory::{PAGE_EXECUTE, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE};
use windows::Win32::Foundation::BOOL;
use iced_x86::code_asm::*;
use std::error::Error;

const MAX_LB_SEND_BUFSIZE: i32 = 0x1000;
const MAX_LB_RECV_BUFSIZE: i32 = 0x5000;
const MAX_LB_OPLAYER_COUNT: i32 = 0x2000;
const LB_OPLAYER_COUNT_DEFAULT: i32 = 40;

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
    address_get_premade: usize,
    injection_manager: InjectionManager,
    memory_allocator: MemoryAllocator,
    enabled: bool,
    
    // Buffer management
    var_ptr_sendbuffer: usize,
    var_ptr_recvbuffer: usize,
    var_ptr_recvbufsize: usize,
    var_ptr_socket: usize,

    sendbuf_size: i32,
    
    // Adjustable by user
    observe_player_count: i32,
    leaderboard_type: LeaderboardType,
}

impl LeaderboardScores {
    pub fn new(pid: u32) -> Result<Self, Box<dyn Error>> {
        let mut memory_allocator = MemoryAllocator::new(pid, 0x10000)?;
        let var_ptr_sendbuffer = memory_allocator.allocate_var_with_size("SendBuffer", DataType::ByteArray, MAX_LB_SEND_BUFSIZE as usize)?;
        let var_ptr_recvbuffer = memory_allocator.allocate_var_with_size("RecvBuffer", DataType::ByteArray, MAX_LB_RECV_BUFSIZE as usize)?;
        let var_ptr_recvbufsize = memory_allocator.allocate_var("RecvBufSize", DataType::I32)?;
        
        let var_ptr_socket = memory_allocator.allocate_var("Socket", DataType::Pointer)?;

        let mut injection_manager = InjectionManager::new(pid);
        injection_manager.add_injection("get_premade".to_string());
        injection_manager.add_injection("ssl_send".to_string());

        let mut leaderboard_scores = Self {
            pid,
            address_ssl_send: 0,
            address_ssl_recv: 0,
            address_get_premade: 0,
            injection_manager,
            memory_allocator,
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
        leaderboard_scores.set_leaderboard_type(LeaderboardType::Duels)?;
        leaderboard_scores.memory_allocator.write_var("RecvBufSize", MAX_LB_RECV_BUFSIZE)?;

        Ok(leaderboard_scores)
    }

    pub fn init(&mut self) -> Result<(), Box<dyn Error>> {
        let guard = Hashlink::instance(self.pid).lock().unwrap();
        if let Some(hashlink) = guard.as_ref() {
            self.address_get_premade = hashlink.get_function_address("get_premade", Some(0))?;
        }

        let hex_pattern_ssl_send = "48 83 ?? ?? 49 63 ?? 48 03 ?? 4D ?? ?? E8 ?? ?? ?? ?? 85";
        let hex_pattern_ssl_recv = "48 83 ?? ?? 49 63 ?? 48 03 ?? 4D ?? ?? E8 ?? ?? ?? ?? 3D";

        let executable_protection = PAGE_EXECUTE.0 | PAGE_EXECUTE_READ.0 | PAGE_EXECUTE_READWRITE.0;

        let addr_ssl_send = aob_scan_mrprotect(self.pid, hex_pattern_ssl_send, executable_protection)?;
        if addr_ssl_send.is_empty() {
            return Err("Pattern not found: ssl_send".into());
        }

        let addr_ssl_recv = aob_scan_mrprotect(self.pid, hex_pattern_ssl_recv, executable_protection)?;
        if addr_ssl_recv.is_empty() {
            return Err("Pattern not found: ssl_recv".into());
        }

        const SSL_NEW_RET_OFFSET: usize = 86;

        self.address_ssl_send = addr_ssl_send[0];
        self.address_ssl_recv = addr_ssl_recv[0];

        Ok(())
    }

    pub fn apply(&mut self, enable: bool) -> Result<(), Box<dyn Error>> {
        if enable {
            /*
            Injection: ssl_send

                1. Check if rdx[0] == 0x81
                2. If so, update socket handle
            */
            let mut code = CodeAssembler::new(64)?;
            let mut label_sslsend = code.create_label();

            code.pushfq()?;
            code.push(rbx)?;
            code.push(rcx)?;

            code.cmp(byte_ptr(rdx), 0x81)?;
            code.jne(label_sslsend)?;

            code.mov(rbx, self.var_ptr_socket as u64)?;
            code.mov(qword_ptr(rbx), rcx)?;

            code.set_label(&mut label_sslsend)?;
            code.pop(rcx)?;
            code.pop(rbx)?;
            code.popfq()?;

            self.injection_manager.apply_injection("ssl_send", self.address_ssl_send, &mut code)?;

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

            code2.mov(rdx, self.var_ptr_sendbuffer as u64)?;
            // code2.mov(rdx, qword_ptr(rbx))?; // bytes

            code2.mov(r8, 0u64)?; // offset
            code2.mov(r9, self.sendbuf_size as u64)?; // length

            code2.mov(rax, self.address_ssl_send as u64)?;
            code2.call(rax)?;

            code2.mov(r10, 0u64)?;
            code2.set_label(&mut label_loop)?;
            
            // ssl/ssl_recv@49012 (mbedtls_ssl_context, bytes, i32, i32) -> i32
            code2.mov(rbx, self.var_ptr_socket as u64)?;
            code2.mov(rcx, qword_ptr(rbx))?;

            code2.mov(rdx, self.var_ptr_recvbuffer as u64)?;
            // code2.mov(rdx, qword_ptr(rbx))?;

            // TODO: Check if we need to adjust `offset`
            code2.mov(r8, r10);

            code2.mov(rbx, self.var_ptr_recvbufsize as u64)?;
            code2.mov(r9, qword_ptr(rbx))?;

            code2.mov(rax, self.address_ssl_recv as u64)?;
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
        } else {
            self.injection_manager.remove_injection("get_premade")?;
            self.injection_manager.remove_injection("ssl_send")?;
        }

        self.enabled = enable;
        Ok(())
    }

    pub fn get_recv_buffer(&self) -> Result<Vec<u8>, Box<dyn Error>> {
        let handle = unsafe {
            OpenProcess(
                PROCESS_VM_READ,
                BOOL::from(false),
                self.pid,
            )
        }.map_err(|e| format!("Failed to open process: {:?}", e))?;

        let mut buffer = vec![0u8; MAX_LB_RECV_BUFSIZE as usize];
        let mut bytes_read = 0;
        
        unsafe {
            ReadProcessMemory(
                handle,
                self.var_ptr_recvbuffer as *const _,
                buffer.as_mut_ptr() as *mut _,
                MAX_LB_RECV_BUFSIZE as usize,
                Some(&mut bytes_read),
            )
        }.map_err(|_| "Failed to read receive buffer")?;

        unsafe { let _ = windows::Win32::Foundation::CloseHandle(handle); }
        
        buffer.truncate(bytes_read);
        Ok(buffer)
    }

    pub fn set_leaderboard_type(&mut self, lb_type: LeaderboardType) -> Result<(), Box<dyn Error>> {
        self.leaderboard_type = lb_type;

        // Currently, we support these requests:
        // 81 7E 00 85 {"uid":46,"args":{"start":0,"v2":true,"count":40,"subrank":0,"season":"NG_RANK_TEAMS_16","game":"northgard"},"cmd":"ranking/getRank"}
        // 81 7E 00 8A {"uid":45,"args":{"start":0,"v2":true,"count":40,"subrank":0,"season":"NG_RANK_FREEFORALL_16","game":"northgard"},"cmd":"ranking/getRank"}
        // 81 7E 00 85 {"uid":44,"args":{"start":0,"v2":true,"count":40,"subrank":0,"season":"NG_RANK_DUELS_16","game":"northgard"},"cmd":"ranking/getRank"}

        let (season, uid, header_byte) = match lb_type {
            LeaderboardType::Duels => ("NG_RANK_DUELS_16", 40, 0x85),
            LeaderboardType::FreeForAll => ("NG_RANK_FREEFORALL_16", 41, 0x8A),
            LeaderboardType::Teams => ("NG_RANK_TEAMS_16", 40, 0x85),
        };

        let json_payload = format!(
            r#"{{"uid":{},"args":{{"start":0,"v2":true,"count":{},"subrank":0,"season":"{}","game":"northgard"}},"cmd":"ranking/getRank"}}"#,
            uid, self.observe_player_count, season
        );

        let mut message = Vec::new();
        message.extend_from_slice(&[0x81, 0x7E, 0x00, header_byte]);
        message.extend_from_slice(json_payload.as_bytes());

        self.sendbuf_size = message.len() as i32;
        self.memory_allocator.write_byte_array("SendBuffer", &message)?;

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