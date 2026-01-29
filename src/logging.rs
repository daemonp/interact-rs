//! Debug logging for interact-rs
//!
//! Writes logs to Logs\interact_debug.log
//! Uses the `windows` crate for type-safe Windows API bindings.

use std::sync::atomic::{AtomicUsize, Ordering};
use windows::core::PCSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::Storage::FileSystem::{
    CreateDirectoryA, CreateFileA, DeleteFileA, FlushFileBuffers, MoveFileA, WriteFile,
    CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ,
};
use windows::Win32::System::SystemInformation::GetLocalTime;

// =============================================================================
// File Handle Management
// =============================================================================

/// Global file handle stored as atomic for thread-safe access
static LOG_HANDLE: AtomicUsize = AtomicUsize::new(0);

/// Convert atomic value to HANDLE
fn handle_from_atomic(val: usize) -> HANDLE {
    HANDLE(val as *mut std::ffi::c_void)
}

/// Convert HANDLE to atomic value
fn handle_to_atomic(h: HANDLE) -> usize {
    h.0 as usize
}

/// Check if a handle is valid
fn is_valid_handle(val: usize) -> bool {
    val != 0 && val != handle_to_atomic(INVALID_HANDLE_VALUE)
}

// =============================================================================
// Log File Paths
// =============================================================================

const LOGS_DIR: &[u8] = b"Logs\0";
const LOG_FILE: &[u8] = b"Logs\\interact_debug.log\0";
const LOG_FILE_1: &[u8] = b"Logs\\interact_debug.log.1\0";
const LOG_FILE_2: &[u8] = b"Logs\\interact_debug.log.2\0";
const LOG_FILE_3: &[u8] = b"Logs\\interact_debug.log.3\0";

// =============================================================================
// Public API
// =============================================================================

/// Initialize the logging system
///
/// Creates the Logs directory if needed, rotates old log files,
/// and opens a new log file for writing.
pub fn init() {
    // Don't reinitialize if already done
    let current = LOG_HANDLE.load(Ordering::SeqCst);
    if current != 0 {
        return;
    }

    unsafe {
        // Create Logs directory (ignore error if exists)
        let _ = CreateDirectoryA(PCSTR::from_raw(LOGS_DIR.as_ptr()), None);

        // Rotate old logs: .3 -> delete, .2 -> .3, .1 -> .2, current -> .1
        rotate_logs();

        // Open new log file
        let handle = CreateFileA(
            PCSTR::from_raw(LOG_FILE.as_ptr()),
            windows::Win32::Storage::FileSystem::FILE_GENERIC_WRITE.0,
            FILE_SHARE_READ,
            None,
            CREATE_ALWAYS,
            FILE_ATTRIBUTE_NORMAL,
            None,
        );

        match handle {
            Ok(h) if h != INVALID_HANDLE_VALUE => {
                LOG_HANDLE.store(handle_to_atomic(h), Ordering::SeqCst);

                // Write initialization message
                let init_msg = b"[INIT] interact-rs logging initialized\r\n";
                let mut written: u32 = 0;
                let _ = WriteFile(h, Some(init_msg), Some(&raw mut written), None);
                let _ = FlushFileBuffers(h);
            }
            _ => {
                // Failed to open log file - logging will be disabled
            }
        }
    }
}

/// Write a log message with timestamp
pub fn log_debug(message: &str) {
    let handle_val = LOG_HANDLE.load(Ordering::SeqCst);
    if !is_valid_handle(handle_val) {
        return;
    }

    let handle = handle_from_atomic(handle_val);
    let timestamp = get_timestamp();
    let line = format!("[{timestamp}] {message}\r\n");

    unsafe {
        let mut written: u32 = 0;
        let _ = WriteFile(handle, Some(line.as_bytes()), Some(&raw mut written), None);
        let _ = FlushFileBuffers(handle);
    }
}

/// Shutdown logging and close the file handle
pub fn shutdown() {
    let handle_val = LOG_HANDLE.swap(0, Ordering::SeqCst);
    if is_valid_handle(handle_val) {
        unsafe {
            let _ = CloseHandle(handle_from_atomic(handle_val));
        }
    }
}

// =============================================================================
// Internal Helpers
// =============================================================================

/// Rotate log files: delete .3, move .2->.3, .1->.2, current->.1
unsafe fn rotate_logs() {
    // Delete oldest log
    let _ = DeleteFileA(PCSTR::from_raw(LOG_FILE_3.as_ptr()));

    // Rotate remaining logs
    let _ = MoveFileA(
        PCSTR::from_raw(LOG_FILE_2.as_ptr()),
        PCSTR::from_raw(LOG_FILE_3.as_ptr()),
    );
    let _ = MoveFileA(
        PCSTR::from_raw(LOG_FILE_1.as_ptr()),
        PCSTR::from_raw(LOG_FILE_2.as_ptr()),
    );
    let _ = MoveFileA(
        PCSTR::from_raw(LOG_FILE.as_ptr()),
        PCSTR::from_raw(LOG_FILE_1.as_ptr()),
    );
}

/// Get a formatted timestamp string
fn get_timestamp() -> String {
    // GetLocalTime returns the SYSTEMTIME directly in the windows crate
    let st = unsafe { GetLocalTime() };
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
        st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond, st.wMilliseconds
    )
}

// =============================================================================
// Logging Macro
// =============================================================================

/// Convenience macro for debug logging
#[macro_export]
macro_rules! debug_log {
    ($($arg:tt)*) => {
        $crate::logging::log_debug(&format!($($arg)*))
    };
}
