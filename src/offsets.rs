//! Memory offsets for WoW 1.12.1.5875 client functions
//!
//! These offsets are specific to the Vanilla client and are used for
//! hooking game functions and calling Lua C API functions.

/// Bootstrap / Initialization Hooks
pub mod bootstrap {
    /// void __fastcall SysMsgInitialize()
    pub const SYS_MSG_INITIALIZE: usize = 0x0044CD10;

    /// void __stdcall LoadScriptFunctions()
    pub const LOAD_SCRIPT_FUNCTIONS: usize = 0x00490250;
}

/// Game Functions
pub mod game {
    /// uint32_t __fastcall GetObjectPointer(uint64_t guid)
    pub const GET_OBJECT_POINTER: usize = 0x00464870;

    /// Pointer to byte indicating if player is in world
    pub const IS_IN_WORLD: usize = 0x00B4B424;

    /// void __thiscall RightClickUnit(uint32_t pointer, int autoloot)
    pub const RIGHT_CLICK_UNIT: usize = 0x0060BEA0;

    /// void __thiscall RightClickObject(uint32_t pointer, int autoloot)
    pub const RIGHT_CLICK_OBJECT: usize = 0x005F8660;

    /// void __stdcall SetTarget(uint64_t guid)
    pub const SET_TARGET: usize = 0x00493540;

    /// Pointer to visible objects manager
    pub const VISIBLE_OBJECTS: usize = 0x00B41414;
}

/// Lua C API Functions (__fastcall unless noted)
pub mod lua_api {
    pub const GETTOP: usize = 0x006F3070;
    pub const SETTOP: usize = 0x006F3080;
    pub const TYPE: usize = 0x006F3460;
    pub const ISNUMBER: usize = 0x006F34D0;
    pub const ISSTRING: usize = 0x006F3510;
    pub const TONUMBER: usize = 0x006F3620;
    pub const TOSTRING: usize = 0x006F3690;
    pub const PUSHNUMBER: usize = 0x006F3810;
    pub const PUSHSTRING: usize = 0x006F3890;
    pub const PUSHNIL: usize = 0x006F37F0;
    pub const PUSHBOOLEAN: usize = 0x006F39F0;
    /// Note: __cdecl, takes message parameter directly
    pub const ERROR: usize = 0x006F4940;
}

/// Lua State Access
pub mod lua_state {
    /// uintptr_t* __fastcall GetLuaContext()
    pub const GET_CONTEXT: usize = 0x007040D0;
}

/// Script Registration
pub mod script {
    /// void __fastcall FrameScript_RegisterFunction(const char*, lua_CFunction)
    pub const REGISTER_FUNCTION: usize = 0x00704120;
}
