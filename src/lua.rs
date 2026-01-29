//! Lua C API FFI bindings for WoW 1.12.1
//!
//! WoW 1.12 uses a custom Lua 5.0 variant with __fastcall convention for most functions.
//! This module provides type-safe wrappers around the raw function pointers.

use crate::offsets;
use std::ffi::{c_char, c_int, c_void, CStr};
use std::mem::transmute;

/// Opaque Lua state pointer
pub type LuaState = *mut c_void;

/// Lua C function signature: int function(lua_State *L)
/// WoW uses __fastcall, which on x86 passes first arg in ECX
#[allow(dead_code)]
pub type LuaCFunction = unsafe extern "fastcall" fn(LuaState) -> c_int;

// =============================================================================
// Lua C API Function Types (all __fastcall in WoW 1.12)
// =============================================================================

type LuaGettopFn = unsafe extern "fastcall" fn(LuaState) -> c_int;
type LuaSettopFn = unsafe extern "fastcall" fn(LuaState, c_int);
type LuaTypeFn = unsafe extern "fastcall" fn(LuaState, c_int) -> c_int;
type LuaIsnumberFn = unsafe extern "fastcall" fn(LuaState, c_int) -> c_int;
type LuaIsstringFn = unsafe extern "fastcall" fn(LuaState, c_int) -> c_int;
type LuaTonumberFn = unsafe extern "fastcall" fn(LuaState, c_int) -> f64;
type LuaTostringFn = unsafe extern "fastcall" fn(LuaState, c_int) -> *const c_char;
type LuaPushnumberFn = unsafe extern "fastcall" fn(LuaState, f64);
type LuaPushstringFn = unsafe extern "fastcall" fn(LuaState, *const c_char);
type LuaPushnilFn = unsafe extern "fastcall" fn(LuaState);
type LuaPushbooleanFn = unsafe extern "fastcall" fn(LuaState, c_int);
type LuaErrorFn = unsafe extern "cdecl" fn(LuaState, *const c_char); // Note: __cdecl for lua_error

/// Type for GetLuaContext function
type GetLuaContextFn = unsafe extern "fastcall" fn() -> LuaState;

/// Type for FrameScript_RegisterFunction
type RegisterFunctionFn = unsafe extern "fastcall" fn(*const c_char, *const c_void);

// =============================================================================
// Lua API Wrapper
// =============================================================================

/// Provides safe(r) access to WoW's Lua C API
#[allow(dead_code)]
pub struct LuaApi {
    gettop: LuaGettopFn,
    settop: LuaSettopFn,
    lua_type: LuaTypeFn,
    isnumber: LuaIsnumberFn,
    isstring: LuaIsstringFn,
    tonumber: LuaTonumberFn,
    tostring: LuaTostringFn,
    pushnumber: LuaPushnumberFn,
    pushstring: LuaPushstringFn,
    pushnil: LuaPushnilFn,
    pushboolean: LuaPushbooleanFn,
    error: LuaErrorFn,
    get_context: GetLuaContextFn,
    register_function: RegisterFunctionFn,
}

#[allow(dead_code)]
impl LuaApi {
    /// Initialize the Lua API by casting memory offsets to function pointers
    ///
    /// # Safety
    /// This assumes the offsets are correct for WoW 1.12.1.5875
    pub unsafe fn new() -> Self {
        Self {
            gettop: transmute(offsets::lua_api::GETTOP),
            settop: transmute(offsets::lua_api::SETTOP),
            lua_type: transmute(offsets::lua_api::TYPE),
            isnumber: transmute(offsets::lua_api::ISNUMBER),
            isstring: transmute(offsets::lua_api::ISSTRING),
            tonumber: transmute(offsets::lua_api::TONUMBER),
            tostring: transmute(offsets::lua_api::TOSTRING),
            pushnumber: transmute(offsets::lua_api::PUSHNUMBER),
            pushstring: transmute(offsets::lua_api::PUSHSTRING),
            pushnil: transmute(offsets::lua_api::PUSHNIL),
            pushboolean: transmute(offsets::lua_api::PUSHBOOLEAN),
            error: transmute(offsets::lua_api::ERROR),
            get_context: transmute(offsets::lua_state::GET_CONTEXT),
            register_function: transmute(offsets::script::REGISTER_FUNCTION),
        }
    }

    /// Get the current Lua state pointer
    #[inline]
    pub unsafe fn get_state(&self) -> LuaState {
        (self.get_context)()
    }

    /// Get the index of the top element in the stack
    #[inline]
    pub unsafe fn gettop(&self, l: LuaState) -> i32 {
        (self.gettop)(l)
    }

    /// Set the stack top to the given index
    #[inline]
    pub unsafe fn settop(&self, l: LuaState, idx: i32) {
        (self.settop)(l, idx);
    }

    /// Pop n elements from the stack
    #[inline]
    pub unsafe fn pop(&self, l: LuaState, n: i32) {
        self.settop(l, -n - 1);
    }

    /// Get the type of the value at the given index
    #[inline]
    pub unsafe fn type_of(&self, l: LuaState, idx: i32) -> i32 {
        (self.lua_type)(l, idx)
    }

    /// Check if the value at index is a number
    #[inline]
    pub unsafe fn isnumber(&self, l: LuaState, idx: i32) -> bool {
        (self.isnumber)(l, idx) != 0
    }

    /// Check if the value at index is a string
    #[inline]
    pub unsafe fn isstring(&self, l: LuaState, idx: i32) -> bool {
        (self.isstring)(l, idx) != 0
    }

    /// Convert value at index to a number
    #[inline]
    pub unsafe fn tonumber(&self, l: LuaState, idx: i32) -> f64 {
        (self.tonumber)(l, idx)
    }

    /// Convert value at index to a string
    /// Returns None if the value is not a string or is null
    pub unsafe fn tostring(&self, l: LuaState, idx: i32) -> Option<&'static str> {
        let ptr = (self.tostring)(l, idx);
        if ptr.is_null() {
            return None;
        }
        CStr::from_ptr(ptr).to_str().ok()
    }

    /// Convert value at index to a raw C string pointer
    #[inline]
    pub unsafe fn tostring_raw(&self, l: LuaState, idx: i32) -> *const c_char {
        (self.tostring)(l, idx)
    }

    /// Push a number onto the stack
    #[inline]
    pub unsafe fn pushnumber(&self, l: LuaState, n: f64) {
        (self.pushnumber)(l, n);
    }

    /// Push a string onto the stack
    #[inline]
    pub unsafe fn pushstring(&self, l: LuaState, s: *const c_char) {
        (self.pushstring)(l, s);
    }

    /// Push nil onto the stack
    #[inline]
    pub unsafe fn pushnil(&self, l: LuaState) {
        (self.pushnil)(l);
    }

    /// Push a boolean onto the stack
    #[inline]
    pub unsafe fn pushboolean(&self, l: LuaState, b: bool) {
        (self.pushboolean)(l, i32::from(b));
    }

    /// Raise a Lua error with a message
    /// Note: This function does not return!
    #[inline]
    pub unsafe fn error(&self, l: LuaState, msg: *const c_char) -> ! {
        (self.error)(l, msg);
        // The Lua error function performs a longjmp and never returns
        std::hint::unreachable_unchecked()
    }

    /// Register a new global Lua function
    pub unsafe fn register_function(&self, name: *const c_char, func: *const c_void) {
        (self.register_function)(name, func);
    }
}

// Global Lua API instance
use crate::errors::LuaError;
use once_cell::sync::OnceCell;
static LUA_API: OnceCell<LuaApi> = OnceCell::new();

/// Get the global Lua API instance
///
/// # Panics
/// Panics if Lua API is not initialized. This should never happen
/// after the DLL has been properly loaded and hooks installed.
pub fn api() -> &'static LuaApi {
    LUA_API
        .get()
        .expect("Lua API not initialized - this is a bug in interact-rs")
}

/// Try to get the global Lua API instance
///
/// Returns `None` if the API hasn't been initialized yet.
/// Prefer `api()` in normal code paths where initialization is guaranteed.
#[allow(dead_code)] // Utility function for defensive code paths
pub fn try_api() -> Result<&'static LuaApi, LuaError> {
    LUA_API.get().ok_or(LuaError::NotInitialized)
}

/// Initialize the global Lua API instance
///
/// # Safety
/// Must only be called once, after DLL is loaded into WoW process
pub unsafe fn init() {
    LUA_API.get_or_init(|| LuaApi::new());
}
