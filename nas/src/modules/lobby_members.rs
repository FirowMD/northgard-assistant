/*
    Allows to see players in queue.
*/

use crate::modules::basic::*;
use crate::modules::aob_injection::AobInjection;
use crate::modules::mem_alloc::*;
use crate::modules::hashlink::*;
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
    injection_loglobbyinfo: Mutex<Option<AobInjection>>,
    injection_loguserjoined: Mutex<Option<AobInjection>>,
    injection_loguserleft: Mutex<Option<AobInjection>>,
    injection_logjoinlobby: Mutex<Option<AobInjection>>,
    members: Arc<Mutex<Vec<String>>>,
    mem_allocator: MemoryAllocator,
}

impl LobbyMembers {
    pub fn new(pid: u32) -> Result<Self, Box<dyn Error>> {
        let members = Arc::new(Mutex::new(Vec::new()));
        let mut memory_allocator = MemoryAllocator::new(pid, 0x1000)?;
        let var_ptr_logs_tmp = memory_allocator.allocate_var("String", DataType::Pointer)?;
        let var_ptr_lobby_tmp = memory_allocator.allocate_var("mpman.Lobby", DataType::Pointer)?;

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
            mem_allocator: memory_allocator,
        };
        lobby.lobby_members_init()?;
        
        Ok(lobby)
    }

    pub fn update_members(&self) {
        if let Ok(var_ptr_logs) = read_qword(self.pid, self.var_ptr_logs) {
            if var_ptr_logs == 0 {
                return;
            }
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
            if injection_loglobbyinfo.is_none() {
                // Save `logLobbyInfo` result to `var_ptr_logs`
                let mut code = CodeAssembler::new(64)?;
                code.push(rcx)?;
                code.mov(rcx, self.var_ptr_logs as u64)?;
                code.mov(qword_ptr(rcx), rax)?;
                code.pop(rcx)?;

                *injection_loglobbyinfo = Some(AobInjection::new(self.pid, self.address_loglobbyinfo_body, &mut code)?);
                tracing::info!("Successfully injected: loglobbyinfo at 0x{:X}", self.address_loglobbyinfo_body);
            }

            if injection_loguserjoined.is_none() {
                // Take `mpman.Lobby` argument from `var_ptr_lobby`
                // Call `logLobbyInfo`
                let mut code = CodeAssembler::new(64)?;
                code.push(rax)?;
                code.push(rcx)?;
                code.mov(rax, self.var_ptr_lobby as u64)?;
                code.mov(rcx, qword_ptr(rax))?;
                code.mov(rax, self.address_loglobbyinfo as u64)?;
                code.call(rax)?;
                code.pop(rcx)?;
                code.pop(rax)?;

                *injection_loguserjoined = Some(AobInjection::new(self.pid, self.address_loguserjoined, &mut code)?);
                tracing::info!("Successfully injected: loguserjoined at 0x{:X}", self.address_loguserjoined);
            }

            if injection_loguserleft.is_none() {
                // Take `mpman.Lobby` argument from `var_ptr_lobby`
                // Call `logLobbyInfo`
                let mut code = CodeAssembler::new(64)?;
                code.push(rax)?;
                code.push(rcx)?;
                code.mov(rax, self.var_ptr_lobby as u64)?;
                code.mov(rcx, qword_ptr(rax))?;
                code.mov(rax, self.address_loglobbyinfo as u64)?;
                code.call(rax)?;
                code.pop(rcx)?;
                code.pop(rax)?;

                *injection_loguserleft = Some(AobInjection::new(self.pid, self.address_loguserleft, &mut code)?);
                tracing::info!("Successfully injected: loguserleft at 0x{:X}", self.address_loguserleft);
            }

            if injection_logjoinlobby.is_none() {
                // Save `mpman.Lobby` argument to `var_ptr_lobby`
                let mut code = CodeAssembler::new(64)?;
                code.push(rax)?;
                code.push(rcx)?;
                code.mov(rax, self.var_ptr_lobby as u64)?;
                code.mov(qword_ptr(rax), rcx)?;
                code.pop(rcx)?;
                code.pop(rax)?;

                *injection_logjoinlobby = Some(AobInjection::new(self.pid, self.address_logjoinlobby, &mut code)?);
                tracing::info!("Successfully injected: logjoinlobby at 0x{:X}", self.address_logjoinlobby);
            }
        } else {
            if let Some(inj) = injection_loglobbyinfo.as_ref() {
                inj.undo()?;
                *injection_loglobbyinfo = None;
                tracing::info!("Successfully removed: loglobbyinfo at 0x{:X}", self.address_loglobbyinfo_body);
            }

            if let Some(inj) = injection_loguserjoined.as_ref() {
                inj.undo()?;
                *injection_loguserjoined = None;
                tracing::info!("Successfully removed: loguserjoined at 0x{:X}", self.address_loguserjoined);
            }

            if let Some(inj) = injection_loguserleft.as_ref() {
                inj.undo()?;
                *injection_loguserleft = None;
                tracing::info!("Successfully removed: loguserleft at 0x{:X}", self.address_loguserleft);
            }

            if let Some(inj) = injection_logjoinlobby.as_ref() {
                inj.undo()?;
                *injection_logjoinlobby = None;
                tracing::info!("Successfully removed: logjoinlobby at 0x{:X}", self.address_logjoinlobby);
            }
        }
        
        Ok(())
    }

    /// Extracts users from `var_ptr_logs`
    pub fn lobby_members_extract(&self) -> Result<String, Box<dyn Error>> {
        let log_addr_ptr = read_qword(self.pid, self.var_ptr_logs)? as usize;
        let log_addr = read_qword(self.pid, log_addr_ptr + 8)? as usize;
        let log_data = read_utf16_string(self.pid, log_addr)?;
        
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
        // let new_members = self.remove_id(members);
        // tracing::debug!("get_members() returning {} members", new_members.len());
        // new_members
        members
    }
}

impl Drop for LobbyMembers {
    fn drop(&mut self) {
        self.mem_allocator.free().unwrap();
    }
}
