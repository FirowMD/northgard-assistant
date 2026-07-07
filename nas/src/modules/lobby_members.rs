/*
    Allows to see players in queue.
*/

use crate::modules::libmem_injection::LibmemInjection;
use crate::modules::hashlink::*;
use libmem::Process;
use crate::utils::libmem_ex::{get_target_process, free, read_qword_ex, read_utf16_string_ex, allocate_region_mrprotect};
use windows::Win32::System::Memory::PAGE_READWRITE;
use iced_x86::code_asm::*;
use std::error::Error;
use std::sync::Mutex;
use std::sync::Arc;

pub struct LobbyMembers {
    pid: u32,
    address_loglobbyinfo_body: usize,
    address_loguserjoined: usize,
    address_loguserleft: usize,
    address_loglobbyinfo: usize,
    address_logjoinlobby: usize,
    var_ptr_logs: usize,
    var_ptr_lobby: usize,
    injection_loglobbyinfo: Mutex<Option<LibmemInjection>>,
    injection_loguserjoined: Mutex<Option<LibmemInjection>>,
    injection_loguserleft: Mutex<Option<LibmemInjection>>,
    injection_logjoinlobby: Mutex<Option<LibmemInjection>>,
    members: Arc<Mutex<Vec<String>>>,
    lm_process: Process,
    lm_alloc_addr: usize,
    lm_alloc_size: usize,
}

impl LobbyMembers {
    /// Convenience: enable all lobby member injections
    pub fn enable(&self) -> Result<(), Box<dyn Error>> {
        self.lobby_members_apply(true)
    }

    /// Convenience: disable all lobby member injections
    pub fn disable(&self) -> Result<(), Box<dyn Error>> {
        self.lobby_members_apply(false)
    }

    pub fn new(pid: u32) -> Result<Self, Box<dyn Error>> {
        let members = Arc::new(Mutex::new(Vec::new()));
        let lm_process = get_target_process(pid).ok_or("Failed to get process with libmem")?;
        let lm_alloc_size = 0x1000usize;
        let allocated = allocate_region_mrprotect(pid, lm_alloc_size, PAGE_READWRITE)?;
        let lm_alloc_addr = allocated.base_address as usize;
        let var_ptr_logs_tmp = lm_alloc_addr;
        let var_ptr_lobby_tmp = lm_alloc_addr + 8;

        let mut lobby = Self {
            pid,
            address_loglobbyinfo_body: 0,
            address_loguserjoined: 0,
            address_loguserleft: 0,
            address_logjoinlobby: 0,
            address_loglobbyinfo: 0,
            var_ptr_logs: var_ptr_logs_tmp,
            var_ptr_lobby: var_ptr_lobby_tmp,
            injection_loglobbyinfo: Mutex::new(None),
            injection_loguserjoined: Mutex::new(None),
            injection_loguserleft: Mutex::new(None),
            injection_logjoinlobby: Mutex::new(None),
            members,
            lm_process,
            lm_alloc_addr,
            lm_alloc_size,
        };
        lobby.lobby_members_init()?;
        
        Ok(lobby)
    }

    pub fn update_members(&self) {
        if let Some(var_ptr_logs) = read_qword_ex(&self.lm_process, self.var_ptr_logs) {
            if var_ptr_logs == 0 { return; }
        }

        if let Ok(new_members) = self.lobby_members_extract() {
            tracing::debug!("Extracted members: {:?}", new_members);
            if let Ok(mut members) = self.members.lock() {
                *members = new_members.split('\n').map(|s| s.to_string()).collect();
                tracing::debug!("Members list updated");
            }
        }
    }

    /// Initializes `LobbyMembers` by finding the target addresses
    pub fn lobby_members_init(&mut self) -> Result<(), Box<dyn Error>> {
        const OFFSET_LOGLOBBYINFO_BODY: usize = 2035;
        const OFFSET_LOGUSER_JOINED_LEFT: usize = 28;

        let guard = Hashlink::instance(self.pid).lock().unwrap();
        if let Some(hashlink) = guard.as_ref() {
            self.address_loglobbyinfo = hashlink.get_function_address("logLobbyInfo", Some(0))?;
            self.address_loglobbyinfo_body = self.address_loglobbyinfo + OFFSET_LOGLOBBYINFO_BODY;
            self.address_loguserjoined = hashlink.get_function_address("logUserJoined", Some(0))?;
            self.address_loguserjoined += OFFSET_LOGUSER_JOINED_LEFT;
            self.address_loguserleft = hashlink.get_function_address("logUserLeft", Some(0))?;
            self.address_loguserleft += OFFSET_LOGUSER_JOINED_LEFT;
            self.address_logjoinlobby = hashlink.get_function_address("logJoinLobby", Some(0))?;
        } else {
            return Err("Hashlink instance not found".into());
        }

        Ok(())
    }

    /// Apply or remove lobby members at the specified address
    pub fn lobby_members_apply(&self, enable: bool) -> Result<(), Box<dyn Error>> {
        let mut injection_loglobbyinfo = self.injection_loglobbyinfo.lock().unwrap();
        let mut injection_loguserjoined = self.injection_loguserjoined.lock().unwrap();
        let mut injection_loguserleft = self.injection_loguserleft.lock().unwrap();
        let mut injection_logjoinlobby = self.injection_logjoinlobby.lock().unwrap();

        if enable {
            self.ensure_injection(
                &mut injection_loglobbyinfo,
                self.address_loglobbyinfo_body,
                || self.asm_save_log_to_var(),
                "loglobbyinfo",
            )?;

            self.ensure_injection(
                &mut injection_loguserjoined,
                self.address_loguserjoined,
                || self.asm_call_loglobbyinfo_with_lobby_from_var(),
                "loguserjoined",
            )?;

            self.ensure_injection(
                &mut injection_loguserleft,
                self.address_loguserleft,
                || self.asm_call_loglobbyinfo_with_lobby_from_var(),
                "loguserleft",
            )?;

            self.ensure_injection(
                &mut injection_logjoinlobby,
                self.address_logjoinlobby,
                || self.asm_save_lobby_arg_to_var(),
                "logjoinlobby",
            )?;
        } else {
            self.remove_injection(&mut injection_loglobbyinfo, self.address_loglobbyinfo_body, "loglobbyinfo")?;
            self.remove_injection(&mut injection_loguserjoined, self.address_loguserjoined, "loguserjoined")?;
            self.remove_injection(&mut injection_loguserleft, self.address_loguserleft, "loguserleft")?;
            self.remove_injection(&mut injection_logjoinlobby, self.address_logjoinlobby, "logjoinlobby")?;
        }
        
        Ok(())
    }

    /// Extracts users from `var_ptr_logs`
    pub fn lobby_members_extract(&self) -> Result<String, Box<dyn Error>> {
        let log_addr_ptr = read_qword_ex(&self.lm_process, self.var_ptr_logs)
            .ok_or("libmem read_qword_ex failed")? as usize;
        let log_addr = read_qword_ex(&self.lm_process, log_addr_ptr + 8)
            .ok_or("libmem read_qword_ex failed")? as usize;
        let log_data = read_utf16_string_ex(&self.lm_process, log_addr, 0x1000)
            .ok_or("libmem read_utf16_string_ex failed")?;
        
        tracing::debug!("log_data: {}", log_data);

        Ok(log_data)
    }

    

    /// Remove ID of each player
    /// Example of input (FFA):
    /// ```
    /// Members:
    /// Player1(S7a801dc1)
    /// Glatcher(Sbdc6d70c)
    /// Player2(Sa3d12ca)
    /// ```
    /// 
    /// Example of input (1v1/2v2/3v3/4v4 etc.):
    /// ```
    /// Members:
    /// Player1(S7a801dc1) (Team 0)
    /// Player2(Sa3d12ca) (Team 0)
    /// Player3(Sbdc6d70c) (Team 1)
    /// Player4(Sbdc6d70c) (Team 1)
    /// Player5(Sbdc6d70c) (Team 2)
    /// Player6(Sbdc6d70c) (Team 2)
    /// ```
    /// 
    /// So we remove the ID which is inside parentheses
    pub fn remove_id(&self, members: Vec<String>) -> Vec<String> {
        let mut new_members = Vec::new();
    
        for member in members.iter().skip(1) {
            let contains_team = self.contains_team(member);
    
            // Remove IDs inside parentheses
            if contains_team {
                // Remove first ID before "(Team"
                if let Some(team_start) = member.rfind("(Team") {
                    if let Some(first_paren) = member[..team_start].find('(') {
                        let cleaned_member = format!(
                            "{}{}",
                            &member[..first_paren].trim(),
                            &member[team_start..]
                        );
                        new_members.push(cleaned_member);
                    } else {
                        new_members.push(member.clone()); // Fallback to original
                    }
                }
            } else {
                // Remove the single ID
                if let Some(first_paren) = member.find('(') {
                    let cleaned_member = &member[..first_paren].trim();
                    new_members.push(cleaned_member.to_string());
                } else {
                    new_members.push(member.clone()); // Fallback to original
                }
            }
        }
    
        new_members
    }
    
    fn contains_team(&self, member: &str) -> bool {
        if let Some(last_paren) = member.rfind('(') {
            let team_part = &member[last_paren..];
            if team_part.starts_with("(Team ") && team_part.ends_with(')') {
                if let Ok(_) = team_part[6..team_part.len() - 1].parse::<u32>() {
                    return true;
                }
            }
        }
        false
    }

    /// Get the members list
    pub fn get_members(&self) -> Vec<String> {
        self.update_members();
        let members = self.members.lock().unwrap().clone();
        members
    }

    /// Get members with IDs stripped; keeps team designation when present
    pub fn get_members_cleaned(&self) -> Vec<String> {
        self.update_members();
        let members = self.members.lock().unwrap().clone();
        self.remove_id(members)
    }
}

impl Drop for LobbyMembers {
    fn drop(&mut self) {
        let _ = free(&self.lm_process, self.lm_alloc_addr, self.lm_alloc_size);
    }
}

// ---- Internal helpers to assemble and manage injections ----
impl LobbyMembers {
    fn ensure_injection<F>(&self,
        slot: &mut Option<LibmemInjection>,
        target_addr: usize,
        build_code: F,
        label: &str,
    ) -> Result<(), Box<dyn Error>>
    where
        F: FnOnce() -> Result<CodeAssembler, Box<dyn Error>>,
    {
        if slot.is_none() {
            let mut code = build_code()?;
            *slot = Some(LibmemInjection::new(self.pid, target_addr, &mut code, &self.lm_process)?);
            tracing::info!("Successfully injected: {} at 0x{:X}", label, target_addr);
        }
        Ok(())
    }

    fn remove_injection(
        &self,
        slot: &mut Option<LibmemInjection>,
        target_addr: usize,
        label: &str,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(inj) = slot.as_ref() {
            inj.undo()?;
            *slot = None;
            tracing::info!("Successfully removed: {} at 0x{:X}", label, target_addr);
        }
        Ok(())
    }

    // Save `logLobbyInfo` result (RAX) into `var_ptr_logs`
    fn asm_save_log_to_var(&self) -> Result<CodeAssembler, Box<dyn Error>> {
        let mut code = CodeAssembler::new(64)?;
        code.push(rcx)?;
        code.mov(rcx, self.var_ptr_logs as u64)?;
        code.mov(qword_ptr(rcx), rax)?;
        code.pop(rcx)?;
        Ok(code)
    }

    // Take mpman.Lobby from `var_ptr_lobby` and call `logLobbyInfo`
    fn asm_call_loglobbyinfo_with_lobby_from_var(&self) -> Result<CodeAssembler, Box<dyn Error>> {
        let mut code = CodeAssembler::new(64)?;
        code.push(rax)?;
        code.push(rcx)?;
        code.mov(rax, self.var_ptr_lobby as u64)?;
        code.mov(rcx, qword_ptr(rax))?;
        code.mov(rax, self.address_loglobbyinfo as u64)?;
        code.call(rax)?;
        code.pop(rcx)?;
        code.pop(rax)?;
        Ok(code)
    }

    // Save `mpman.Lobby` argument (RCX) into `var_ptr_lobby`
    fn asm_save_lobby_arg_to_var(&self) -> Result<CodeAssembler, Box<dyn Error>> {
        let mut code = CodeAssembler::new(64)?;
        code.push(rax)?;
        code.push(rcx)?;
        code.mov(rax, self.var_ptr_lobby as u64)?;
        code.mov(qword_ptr(rax), rcx)?;
        code.pop(rcx)?;
        code.pop(rax)?;
        Ok(code)
    }
}
