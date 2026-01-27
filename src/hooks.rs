//! Function hooks for WoW 1.12.1
//!
//! Uses the `retour` crate to hook game functions for:
//! - Bootstrap initialization
//! - Lua function registration

use crate::{lua, offsets, scripts};
use retour::static_detour;
use std::sync::atomic::{AtomicBool, Ordering};

// =============================================================================
// Version Information
// =============================================================================

pub const MAJOR_VERSION: u32 = 1;
pub const MINOR_VERSION: u32 = 0;
pub const PATCH_VERSION: u32 = 0;

// =============================================================================
// Function Type Definitions
// =============================================================================

/// void __fastcall SysMsgInitialize()
type SysMsgInitializeFn = extern "fastcall" fn();

/// void __stdcall LoadScriptFunctions()
type LoadScriptFunctionsFn = extern "stdcall" fn();

// =============================================================================
// Static Detours
// =============================================================================

static_detour! {
    static SysMsgInitHook: extern "fastcall" fn();
    static LoadScriptFunctionsHook: extern "stdcall" fn();
}

// =============================================================================
// Initialization State
// =============================================================================

static HOOKS_INITIALIZED: AtomicBool = AtomicBool::new(false);

// =============================================================================
// Hook Implementations
// =============================================================================

/// SysMsgInitialize hook - bootstraps all other hooks
fn sys_msg_init_detour() {
    // Call original
    SysMsgInitHook.call();

    // Initialize hooks only once
    if HOOKS_INITIALIZED.swap(true, Ordering::SeqCst) {
        return;
    }

    // Initialize logging (safe to do file I/O now, we're past DllMain)
    crate::logging::init();

    debug_log!(
        "=== interact-rs v{}.{}.{} ===",
        MAJOR_VERSION,
        MINOR_VERSION,
        PATCH_VERSION
    );
    debug_log!("SysMsgInitialize called - initializing hooks");

    // Initialize all other hooks
    unsafe {
        match init_all_hooks() {
            Ok(()) => debug_log!("All hooks initialized successfully"),
            Err(e) => debug_log!("Failed to initialize hooks: {:?}", e),
        }
    }
}

/// LoadScriptFunctions hook - register our Lua functions
fn load_script_functions_detour() {
    // Call original first
    LoadScriptFunctionsHook.call();

    debug_log!("LoadScriptFunctions called - registering Lua functions");

    // Initialize Lua API
    unsafe {
        lua::init();

        // Register our custom Lua functions
        scripts::register_functions();
    }

    debug_log!("Lua functions registered: InteractNearest");
}

// =============================================================================
// Hook Initialization
// =============================================================================

/// Initialize all secondary hooks (called from SysMsgInit hook)
unsafe fn init_all_hooks() -> Result<(), Box<dyn std::error::Error>> {
    // Hook LoadScriptFunctions
    let load_script_functions: LoadScriptFunctionsFn =
        std::mem::transmute(offsets::bootstrap::LOAD_SCRIPT_FUNCTIONS);
    LoadScriptFunctionsHook
        .initialize(load_script_functions, load_script_functions_detour)?
        .enable()?;

    Ok(())
}

/// Main entry point - install bootstrap hook
///
/// # Safety
/// Must be called from DllMain during DLL_PROCESS_ATTACH
pub unsafe fn load() -> Result<(), Box<dyn std::error::Error>> {
    // Hook SysMsgInitialize as bootstrap
    let sys_msg_init: SysMsgInitializeFn =
        std::mem::transmute(offsets::bootstrap::SYS_MSG_INITIALIZE);

    SysMsgInitHook
        .initialize(sys_msg_init, sys_msg_init_detour)?
        .enable()?;

    Ok(())
}
