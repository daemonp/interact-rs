//! Error types for interact-rs
//!
//! Provides strongly-typed errors for better error handling and debugging.

use thiserror::Error;

/// Errors that can occur during hook initialization and operation
#[derive(Debug, Error)]
pub enum HookError {
    /// Failed to initialize a function hook
    #[error("Failed to initialize hook at {addr:#010x}: {message}")]
    InitFailed { addr: usize, message: String },

    /// Failed to enable a hook after initialization
    #[error("Failed to enable hook: {0}")]
    EnableFailed(String),

    /// Hook operation failed due to retour error
    #[error("Hook error: {0}")]
    RetourError(#[from] retour::Error),
}

/// Errors that can occur with Lua API operations
#[derive(Debug, Error)]
pub enum LuaError {
    /// Lua API was accessed before initialization
    #[error("Lua API not initialized - ensure DLL is properly loaded")]
    NotInitialized,
}

/// Top-level error type for the interact library
#[derive(Debug, Error)]
pub enum InteractError {
    /// Hook-related error
    #[error(transparent)]
    Hook(#[from] HookError),

    /// Lua-related error
    #[error(transparent)]
    Lua(#[from] LuaError),
}

impl From<retour::Error> for InteractError {
    fn from(err: retour::Error) -> Self {
        InteractError::Hook(HookError::RetourError(err))
    }
}
