/*
    Injects to ssl_send and getModes.
    - ssl_send - to obtain socket handle
    - getModes - to execute our code, which will receive actual player ranks
*/

use crate::modules::base::InjectionManager;
use crate::modules::mem_alloc::{MemoryAllocator, DataType};
use crate::modules::hashlink::*;
use crate::modules::basic::aob_scan_mrprotect;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_VM_READ, PROCESS_VM_WRITE, PROCESS_VM_OPERATION};
use windows::Win32::System::Diagnostics::Debug::{ReadProcessMemory, WriteProcessMemory};
use windows::Win32::System::Memory::{PAGE_EXECUTE, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE};
use windows::Win32::Foundation::BOOL;
use iced_x86::code_asm::*;
use std::error::Error;
use std::sync::atomic::{AtomicUsize, AtomicBool, Ordering};

const MAX_LB_MEMORY_REGION_SIZE: usize = 0x10000;
const MAX_LB_SEND_BUFSIZE: i32 = 0x1000;
const MAX_LB_RECV_BUFSIZE: i32 = 0x5000;
const MAX_LB_OPLAYER_COUNT: i32 = 0x2000;
const LB_OPLAYER_COUNT_DEFAULT: i32 = 40;

// Request builder for leaderboard API calls
struct LeaderboardRequestBuilder {
    leaderboard_type: LeaderboardType,
    observe_player_count: i32,
}

impl LeaderboardRequestBuilder {
    fn new(lb_type: LeaderboardType, player_count: i32) -> Self {
        Self {
            leaderboard_type: lb_type,
            observe_player_count: player_count,
        }
    }
    
    fn build_request(&self, uid: u32) -> Vec<u8> {
        let (season, header_byte) = match self.leaderboard_type {
            LeaderboardType::Duels => ("NG_RANK_DUELS_16", 0x85),
            LeaderboardType::FreeForAll => ("NG_RANK_FREEFORALL_16", 0x8A),
            LeaderboardType::Teams => ("NG_RANK_TEAMS_16", 0x85),
        };

        let json_payload = format!(
            r#"{{"uid":{},"args":{{"start":0,"v2":true,"count":{},"subrank":0,"season":"{}","game":"northgard"}},"cmd":"ranking/getRank"}}"#,
            uid, self.observe_player_count, season
        );

        let mut message = Vec::new();
        message.extend_from_slice(&[0x81, 0x7E, 0x00, header_byte]);
        message.extend_from_slice(json_payload.as_bytes());
        message
    }
}

#[derive(Clone, Copy)]
pub enum LeaderboardType {
    Duels,
    FreeForAll,
    Teams,
}

pub struct LeaderboardScores {
    pid: u32,
    enabled: bool,

    address_ssl_send: usize,
    address_ssl_recv: usize,

    // fn command@11808 (mpman.net.Connection, String, dynamic) -> mpman.Promise (22 regs, 45 ops)
    address_command: usize,
    // fn getModes@30686 (ui.menus.multiplayer.LobbyFinderForm) -> mpman.Lobby (4 regs, 10 ops)
    address_getmodes: usize,
    
    injection_manager: InjectionManager,
    memory_allocator: MemoryAllocator,
    
    // Buffer management
    var_ptr_sendbuffer: usize,
    var_ptr_recvbuffer: usize,
    var_ptr_recvbufsize: usize,
    var_ptr_socket: usize,
    var_ptr_connection_uid: usize, // pointer to mpman.net.Connection->uid
    var_ptr_uid: usize, // uid value itself
    
    // Callback infrastructure
    var_ptr_callback: usize,
    var_ptr_resp_callback: usize,

    sendbuf_size: i32,
    
    // Request builder
    request_builder: LeaderboardRequestBuilder,
}

static LEADERBOARD_INSTANCE_PTR: AtomicUsize = AtomicUsize::new(0);
static LB_PENDING: AtomicBool = AtomicBool::new(false);

pub struct LeaderboardResponse {
    pub data: Vec<u8>,
}

impl LeaderboardScores {
    extern "C" fn update_leaderboard_callback(uid: u32) {
        if uid == 0 || uid > 1000000 {
            return;
        }
        let ptr = LEADERBOARD_INSTANCE_PTR.load(Ordering::Relaxed);
        if ptr == 0 {
            return;
        }
        unsafe {
            let instance = &mut *(ptr as *mut LeaderboardScores);
            let _ = instance.update_request_with_uid(uid);
        }
    }

    extern "C" fn update_leaderboard_response_callback() {
        LB_PENDING.store(true, Ordering::Release);
    }
    
    pub fn new(pid: u32) -> Result<Self, Box<dyn Error>> {
        let mut memory_allocator = MemoryAllocator::new(pid, MAX_LB_MEMORY_REGION_SIZE)?;
        let var_ptr_sendbuffer = memory_allocator.allocate_var_with_size("SendBuffer", DataType::ByteArray, MAX_LB_SEND_BUFSIZE as usize)?;
        let var_ptr_recvbuffer = memory_allocator.allocate_var_with_size("RecvBuffer", DataType::ByteArray, MAX_LB_RECV_BUFSIZE as usize)?;
        let var_ptr_recvbufsize = memory_allocator.allocate_var("RecvBufSize", DataType::I32)?;
        
        let var_ptr_socket = memory_allocator.allocate_var("Socket", DataType::Pointer)?;
        let var_ptr_connection_uid = memory_allocator.allocate_var("ConnectionUID", DataType::Pointer)?;
        let var_ptr_uid = memory_allocator.allocate_var("UID", DataType::I32)?;
        
        // Callback infrastructure
        let var_ptr_callback = memory_allocator.allocate_var("UpdateLeaderboardCallback", DataType::Pointer)?;
        let var_ptr_resp_callback = memory_allocator.allocate_var("UpdateLeaderboardResponseCallback", DataType::Pointer)?;

        let mut injection_manager = InjectionManager::new(pid);
        injection_manager.add_injection("getModes".to_string());
        injection_manager.add_injection("ssl_send".to_string());
        injection_manager.add_injection("command".to_string());

        let mut leaderboard_scores = Self {
            pid,
            address_ssl_send: 0,
            address_ssl_recv: 0,
            address_command: 0,
            address_getmodes: 0,
            injection_manager,
            memory_allocator,
            enabled: false,
            var_ptr_sendbuffer,
            var_ptr_recvbuffer,
            var_ptr_recvbufsize,
            var_ptr_socket,
            var_ptr_connection_uid,
            var_ptr_uid,
            var_ptr_callback,
            var_ptr_resp_callback,
            sendbuf_size: MAX_LB_SEND_BUFSIZE,
            request_builder: LeaderboardRequestBuilder::new(LeaderboardType::Duels, LB_OPLAYER_COUNT_DEFAULT),
        };

        leaderboard_scores.init()?;
        leaderboard_scores.setup_callback_infrastructure()?;
        leaderboard_scores.set_leaderboard_type(LeaderboardType::Duels)?;
        leaderboard_scores.memory_allocator.write_var("RecvBufSize", MAX_LB_RECV_BUFSIZE)?;

        Ok(leaderboard_scores)
    }

    pub fn init(&mut self) -> Result<(), Box<dyn Error>> {
        let guard = Hashlink::instance(self.pid).lock().unwrap();
        if let Some(hashlink) = guard.as_ref() {
            const COMMAND_OFFSET: usize = 31; // inc eax; mov [rbp-04],eax
            self.address_command = hashlink.get_function_address("command", Some(5))? + COMMAND_OFFSET;
            self.address_getmodes = hashlink.get_function_address("getModes", Some(0))?;
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

        // const SSL_NEW_RET_OFFSET: usize = 86;

        self.address_ssl_send = addr_ssl_send[0];
        self.address_ssl_recv = addr_ssl_recv[0];

        Ok(())
    }
    
    fn setup_callback_infrastructure(&mut self) -> Result<(), Box<dyn Error>> {
        let callback_addr = Self::update_leaderboard_callback as *const () as usize;
        self.memory_allocator.write_var("UpdateLeaderboardCallback", callback_addr)?;
        let resp_addr = Self::update_leaderboard_response_callback as *const () as usize;
        self.memory_allocator.write_var("UpdateLeaderboardResponseCallback", resp_addr)?;
        LEADERBOARD_INSTANCE_PTR.store(self as *const Self as usize, Ordering::Relaxed);
        Ok(())
    }
    
    fn update_request_with_uid(&mut self, uid: u32) -> Result<(), Box<dyn Error>> {
        let request = self.request_builder.build_request(uid);
        self.sendbuf_size = request.len() as i32;
        
        // Direct WriteProcessMemory instead of going through memory_allocator
        let handle = unsafe {
            OpenProcess(
                PROCESS_VM_WRITE | PROCESS_VM_OPERATION,
                BOOL::from(false),
                self.pid,
            )
        }.map_err(|e| format!("Failed to open process: {:?}", e))?;
        
        let result = unsafe {
            WriteProcessMemory(
                handle,
                self.var_ptr_sendbuffer as *mut _,
                request.as_ptr() as *const _,
                request.len(),
                None,
            )
        };
        
        // Uncommenting this crashes the game
        // unsafe { let _ = windows::Win32::Foundation::CloseHandle(handle); }
        
        result.map_err(|_| "Failed to write request to SendBuffer")?;
        
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
            Injection: getModes

                1. Call ssl_send
                2. Call ssl_recv until rax is not 0 or -1 (0xFFFFFFFFFFFFFFFF)
                3. Everytime ssl_recv is called we need to store data in the receive buffer
            */

            let mut code2 = CodeAssembler::new(64)?;
            let mut label_loop = code2.create_label();

            code2.push(rbp)?;
            code2.mov(rbp, rsp)?;
            code2.pushfq()?;
            code2.push(rax)?;
            code2.push(rbx)?;
            code2.push(rcx)?;
            code2.push(rdx)?;
            code2.push(r8)?;
            code2.push(r9)?;
            code2.push(r10)?;
            code2.push(r11)?;
            code2.push(r12)?;

            // Update `uid` in `mpman.net.Connection` and call Rust callback
            code2.mov(rbx, self.var_ptr_uid as u64)?;
            code2.mov(eax, dword_ptr(rbx))?;
            code2.mov(rbx, self.var_ptr_connection_uid as u64)?;
            code2.mov(dword_ptr(rbx), eax)?;
            
            //
            // Request buffer
            //

            code2.mov(ecx, eax)?;
            code2.mov(rbx, self.var_ptr_callback as u64)?;
            code2.mov(rax, qword_ptr(rbx))?;
            code2.sub(rsp, 0x20)?;
            code2.call(rax)?;
            code2.add(rsp, 0x20)?;

            //
            // ssl/ssl_send@49013 (mbedtls_ssl_context, bytes, i32, i32) -> i32
            //

            code2.mov(rbx, self.var_ptr_socket as u64)?;
            code2.mov(rcx, qword_ptr(rbx))?; // mbedtls_ssl_context

            code2.mov(rdx, self.var_ptr_sendbuffer as u64)?; // bytes

            code2.mov(r8, 0u64)?; // offset
            code2.mov(r9, self.sendbuf_size as u64)?; // length

            code2.mov(rax, self.address_ssl_send as u64)?;
            code2.sub(rsp, 0x20)?;
            code2.call(rax)?;
            code2.add(rsp, 0x20)?;

            code2.mov(r10, 0u64)?;
            code2.set_label(&mut label_loop)?;
            
            //
            // ssl/ssl_recv@49012 (mbedtls_ssl_context, bytes, i32, i32) -> i32
            //

            code2.mov(rbx, self.var_ptr_socket as u64)?;
            code2.mov(rcx, qword_ptr(rbx))?; // mbedtls_ssl_context

            code2.mov(rdx, self.var_ptr_recvbuffer as u64)?; // bytes

            // TODO: Check if we need to adjust `offset`
            code2.mov(r8, 0u64)?; // data collected is stored in r10

            code2.mov(rbx, self.var_ptr_recvbufsize as u64)?;
            code2.mov(r9, qword_ptr(rbx))?; // length

            code2.mov(rax, self.address_ssl_recv as u64)?;
            code2.sub(rsp, 0x20)?;
            code2.call(rax)?;
            code2.add(rsp, 0x20)?;

            code2.add(r10, rax)?;
            code2.cmp(eax, 0)?;
            code2.jg(label_loop)?;

            code2.mov(rbx, self.var_ptr_resp_callback as u64)?;
            code2.mov(rax, qword_ptr(rbx))?;
            code2.sub(rsp, 0x20)?;
            code2.call(rax)?;
            code2.add(rsp, 0x20)?;

            code2.pop(r12)?;
            code2.pop(r11)?;
            code2.pop(r10)?;
            code2.pop(r9)?;
            code2.pop(r8)?;
            code2.pop(rdx)?;
            code2.pop(rcx)?;
            code2.pop(rbx)?;
            code2.pop(rax)?;
            code2.popfq()?;
            code2.pop(rbp)?;

            self.injection_manager.apply_injection("getModes", self.address_getmodes, &mut code2)?;

            /*
            Injection: command

                1. Obtain `uid` needed for ssl_send requests
                2. Obtain address of `mpman.net.Connection->uid`
            */

            let mut code3 = CodeAssembler::new(64)?;

            code3.pushfq()?;
            code3.push(rax)?;
            code3.push(rbx)?;
            code3.push(rcx)?;
            
            code3.mov(rbx, self.var_ptr_uid as u64)?;
            code3.inc(eax)?;
            code3.mov(dword_ptr(rbx), eax)?;

            code3.mov(rbx, self.var_ptr_connection_uid as u64)?;
            code3.add(rcx, 0x30)?;
            code3.mov(qword_ptr(rbx), rcx)?;
            
            code3.pop(rcx)?;
            code3.pop(rbx)?;
            code3.pop(rax)?;
            code3.popfq()?;

            self.injection_manager.apply_injection("command", self.address_command, &mut code3)?;
        } else {
            self.injection_manager.remove_injection("ssl_send")?;
            self.injection_manager.remove_injection("getModes")?;
            self.injection_manager.remove_injection("command")?;
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
        
        // Define response patterns for different leaderboard types
        let patterns = [
            [0x81, 0x7E, 0x1C, 0x39, 0x7B], // NG_RANK_TEAMS_16
            [0x81, 0x7E, 0x1A, 0xAD, 0x7B], // NG_RANK_FREEFORALL_16
            [0x81, 0x7E, 0x19, 0xE0, 0x7B], // NG_RANK_DUELS_16
        ];
        
        // Find the start of JSON data using any of the patterns
        for pattern in &patterns {
            if let Some(start_pos) = buffer.windows(pattern.len()).position(|window| window == pattern) {
                // Start from the '{' character (last byte of pattern)
                let json_start = start_pos + pattern.len() - 1;
                if json_start < buffer.len() {
                    // Find the end by looking for consecutive null bytes (0x00 0x00)
                    let mut end_pos = buffer.len();
                    for i in json_start..buffer.len()-1 {
                        if buffer[i] == 0x00 && buffer[i + 1] == 0x00 {
                            end_pos = i;
                            break;
                        }
                    }
                    
                    return Ok(buffer[json_start..end_pos].to_vec());
                }
            }
        }
        
        Ok(buffer)
    }

    pub fn set_leaderboard_type(&mut self, lb_type: LeaderboardType) -> Result<(), Box<dyn Error>> {
        self.request_builder = LeaderboardRequestBuilder::new(lb_type, self.request_builder.observe_player_count);
        
        let placeholder_uid = 1;
        self.update_request_with_uid(placeholder_uid)?;
        
        Ok(())
    }

    pub fn set_observable_player_count(&mut self, oplayer_count: i32) -> Result<(), Box<dyn Error>> {
        if oplayer_count <= 0 {
            return Err("Observable player count must be positive".into());
        }
        
        if oplayer_count > MAX_LB_OPLAYER_COUNT as i32 {
            return Err(format!("Observable player count cannot exceed {}", MAX_LB_OPLAYER_COUNT).into());
        }
        
        // Update the request builder with new player count
        self.request_builder = LeaderboardRequestBuilder::new(self.request_builder.leaderboard_type, oplayer_count);
        
        // Regenerate request with current settings
        let placeholder_uid = 1;
        self.update_request_with_uid(placeholder_uid)?;
        
        Ok(())
    }

    pub fn take_pending_leaderboard() -> Option<LeaderboardResponse> {
        if LB_PENDING.swap(false, Ordering::Acquire) {
            let ptr = LEADERBOARD_INSTANCE_PTR.load(Ordering::Relaxed);
            if ptr == 0 { return None; }
            unsafe {
                let instance = &*(ptr as *const LeaderboardScores);
                if let Ok(bytes) = instance.get_recv_buffer() {
                    Some(LeaderboardResponse { data: bytes })
                } else {
                    None
                }
            }
        } else {
            None
        }
    }
}