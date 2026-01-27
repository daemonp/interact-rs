//! interact-rs - Interact with nearest object keybind for World of Warcraft 1.12.1.5875
//!
//! A Rust port of the Interact DLL that provides an "interact with nearest object"
//! keybind functionality for the WoW 1.12.1 Vanilla client.

#![allow(non_snake_case)]
#![allow(non_camel_case_types)]

#[macro_use]
mod logging;
mod game;
mod hooks;
mod lua;
mod offsets;
mod scripts;

use std::ffi::c_void;
use windows::Win32::Foundation::{BOOL, FALSE, HINSTANCE, TRUE};
use windows::Win32::System::SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH};

/// DLL entry point
#[no_mangle]
pub unsafe extern "system" fn DllMain(
    _hinst_dll: HINSTANCE,
    fdw_reason: u32,
    _lpv_reserved: *mut c_void,
) -> BOOL {
    match fdw_reason {
        DLL_PROCESS_ATTACH => {
            // NOTE: Do NOT do file I/O here - it can deadlock due to loader lock
            // Logging is initialized later in SysMsgInitialize hook

            // Install bootstrap hook only
            match hooks::load() {
                Ok(()) => TRUE,
                Err(_) => FALSE,
            }
        }
        DLL_PROCESS_DETACH => {
            debug_log!("interact-rs unloading...");
            logging::shutdown();
            TRUE
        }
        _ => TRUE,
    }
}
