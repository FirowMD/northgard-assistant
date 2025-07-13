pub mod commands;
pub mod utils;
pub mod core;

// Re-export commonly used items
pub use commands::*;
pub use utils::*;

use hudhook::*;

mod main_window;

#[no_mangle]
pub unsafe extern "stdcall" fn DllMain(
    hmodule: hudhook::windows::Win32::Foundation::HINSTANCE,
    reason: u32,
    _: *mut std::ffi::c_void,
) {
    if reason == hudhook::windows::Win32::System::SystemServices::DLL_PROCESS_ATTACH {
        // main_window::setup_tracing();
        hudhook::tracing::trace!("DllMain()");
        let _ = std::thread::spawn(move || {
            if let Err(e) = hudhook::Hudhook::builder()
                .with::<hooks::dx11::ImguiDx11Hooks>(main_window::MainWindow::new())
                .with_hmodule(hmodule)
                .build()
                .apply()
            {
                hudhook::tracing::error!("Couldn't apply hooks: {e:?}");
                hudhook::eject();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_accept() {
    }
} 