//! interact-rs - Interact with nearest object keybind for World of Warcraft 1.12.1.5875
//!
//! A Rust port of the Interact DLL that provides an "interact with nearest object"
//! keybind functionality for the WoW 1.12.1 Vanilla client.

// =============================================================================
// Lints
// =============================================================================

// Enable comprehensive clippy lints for code quality
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
// Allow these specific patterns that are intentional in this codebase
#![allow(clippy::missing_errors_doc)] // FFI functions don't need error docs
#![allow(clippy::missing_panics_doc)] // Panics are documented where relevant
#![allow(clippy::missing_safety_doc)] // DllMain safety is implicit
#![allow(clippy::must_use_candidate)] // Many functions have side effects
#![allow(clippy::cast_possible_truncation)] // Intentional u64 -> u32 casts for game pointers
#![allow(clippy::cast_possible_wrap)] // Intentional i32 <-> u32 conversions
#![allow(clippy::cast_sign_loss)] // Intentional signed to unsigned conversions
#![allow(clippy::cast_precision_loss)] // Intentional f64 -> f32 conversions
#![allow(clippy::unreadable_literal)] // Memory addresses match game documentation format
#![allow(clippy::doc_markdown)] // Technical terms don't need backticks everywhere
#![allow(clippy::ptr_as_ptr)] // Explicit casts for FFI clarity
#![allow(clippy::missing_transmute_annotations)] // Type inference is sufficient
// Allow non-standard naming to match game's conventions
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]

#[macro_use]
mod logging;
mod errors;
mod game;
mod hooks;
mod lua;
mod offsets;
mod scripts;

pub use errors::{HookError, InteractError, LuaError};

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
