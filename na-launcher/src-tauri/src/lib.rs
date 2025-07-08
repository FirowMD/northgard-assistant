// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

use hudhook::inject::Process;
use std::process::Command;
use std::thread;
use std::time::Duration;
use tauri::command;
use std::fs;
use tauri::AppHandle;
use tauri::Manager;
use tracing;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_CREATE_THREAD, PROCESS_QUERY_INFORMATION, PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE};
use windows::Win32::Foundation::{HANDLE, CloseHandle};
use std::error::Error;


// Include the DLL directly from nas/target/release
const NAS_DLL: &[u8] = include_bytes!("../../../nas/target/release/nas.dll");

#[derive(Debug)]
pub enum AttachError {
    ProcessOpenError(String),
    InvalidHandle,
    InjectionError(String),
}

impl std::fmt::Display for AttachError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProcessOpenError(e) => write!(f, "Failed to open process: {}", e),
            Self::InvalidHandle => write!(f, "Invalid process handle"),
            Self::InjectionError(e) => write!(f, "Injection failed: {}", e),
        }
    }
}

impl Error for AttachError {}

trait ProcessExt {
    fn by_pid(pid: u32) -> Result<Self, AttachError>
    where
        Self: Sized;
}

impl ProcessExt for Process {
    fn by_pid(pid: u32) -> Result<Self, AttachError> {
        let handle = unsafe {
            OpenProcess(
                PROCESS_CREATE_THREAD | 
                PROCESS_QUERY_INFORMATION | 
                PROCESS_VM_OPERATION | 
                PROCESS_VM_READ | 
                PROCESS_VM_WRITE,
                false,
                pid
            )
        }.map_err(|e| AttachError::ProcessOpenError(format!("{:?}", e)))?;

        if handle.is_invalid() {
            unsafe { 
                let _ = CloseHandle(handle);
            };
            return Err(AttachError::InvalidHandle);
        }

        let mut process = Process::by_name("Northgard.exe")
            .map_err(|e| AttachError::ProcessOpenError(e.to_string()))?;
        
        unsafe {
            let process_ptr = &mut process as *mut Process as *mut HANDLE;
            *process_ptr = HANDLE(handle.0);
        }

        Ok(process)
    }
}

fn setup_logging() {
    use tracing_subscriber::EnvFilter;
    
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
}

#[command]
fn launch_northgard(app_handle: AppHandle) -> Result<(), String> {
    setup_logging();
    
    let app_dir = app_handle.path()
        .app_local_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?
        .join("nas");
    
    fs::create_dir_all(&app_dir)
        .map_err(|e| format!("Failed to create directory: {}", e))?;

    let dll_path = app_dir.join("nas.dll");
    fs::write(&dll_path, NAS_DLL)
        .map_err(|e| format!("Failed to write DLL: {}", e))?;

    Command::new("cmd")
        .args(["/C", "start", "steam://rungameid/466560"])
        .spawn()
        .map_err(|e| format!("Failed to start game: {}", e))?;

    thread::sleep(Duration::from_secs(5));

    let app_handle_clone = app_handle.clone();
    
    let dll_path_buf = dll_path.canonicalize()
        .map_err(|e| format!("Failed to get absolute path: {}", e))?;
    thread::spawn(move || {
        loop {
            match Process::by_name("Northgard.exe") {
                Ok(process) => {
                    match process.inject(dll_path_buf.clone()) {
                        Ok(_) => {
                            tracing::info!("Successfully injected nas.dll");
                            app_handle_clone.exit(0);
                            break;
                        }
                        Err(e) => {
                            tracing::error!("Failed to inject: {}", e);
                            thread::sleep(Duration::from_secs(1));
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Waiting for Northgard.exe: {}", e);
                    thread::sleep(Duration::from_secs(1));
                }
            }
        }
    });

    Ok(())
}

#[command]
fn attach_to_pid(pid: u32, app_handle: AppHandle) -> Result<(), String> {
    let app_dir = app_handle.path()
        .app_local_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {}", e))?
        .join("nas");
    
    let dll_path = app_dir.join("nas.dll");
    let dll_path_buf = dll_path.canonicalize()
        .map_err(|e| format!("Failed to get absolute path: {}", e))?;

    match Process::by_pid(pid) {
        Ok(process) => {
            match process.inject(dll_path_buf) {
                Ok(_) => {
                    tracing::info!("Successfully injected nas.dll to PID {}", pid);
                    Ok(())
                }
                Err(e) => {
                    tracing::error!("Failed to inject: {}", e);
                    Err(format!("Failed to inject: {}", e))
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to attach to process: {}", e);
            Err(format!("Failed to attach to process: {}", e))
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            launch_northgard,
            attach_to_pid
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
