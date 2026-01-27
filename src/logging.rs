//! Debug logging for interact-rs
//!
//! Writes logs to Logs\interact_debug.log
//! Uses raw Windows API for maximum compatibility in DLL context.

use std::sync::atomic::{AtomicUsize, Ordering};

// =============================================================================
// File Handle Management
// =============================================================================

// Global file handle - using atomic for simple thread-safe storage
static LOG_HANDLE: AtomicUsize = AtomicUsize::new(0);

// Windows constants
const INVALID_HANDLE_VALUE: usize = usize::MAX;
const CREATE_ALWAYS: u32 = 2;
const GENERIC_WRITE: u32 = 0x40000000;
const FILE_SHARE_READ: u32 = 0x00000001;
const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;

#[repr(C)]
struct SystemTime {
    year: u16,
    month: u16,
    day_of_week: u16,
    day: u16,
    hour: u16,
    minute: u16,
    second: u16,
    milliseconds: u16,
}

extern "system" {
    fn CreateDirectoryA(path: *const i8, security: *mut std::ffi::c_void) -> i32;
    fn CreateFileA(
        filename: *const i8,
        access: u32,
        share_mode: u32,
        security: *mut std::ffi::c_void,
        creation: u32,
        flags: u32,
        template: *mut std::ffi::c_void,
    ) -> usize;
    fn WriteFile(
        handle: usize,
        buffer: *const u8,
        len: u32,
        written: *mut u32,
        overlapped: *mut std::ffi::c_void,
    ) -> i32;
    fn FlushFileBuffers(handle: usize) -> i32;
    fn CloseHandle(handle: usize) -> i32;
    fn GetLocalTime(lp_system_time: *mut SystemTime);
    fn DeleteFileA(filename: *const i8) -> i32;
    fn MoveFileA(existing: *const i8, new: *const i8) -> i32;
}

/// Initialize the logging system
pub fn init() {
    // Don't reinitialize
    let current = LOG_HANDLE.load(Ordering::SeqCst);
    if current != 0 {
        return;
    }

    unsafe {
        // Create Logs directory
        let logs_dir = b"Logs\0";
        CreateDirectoryA(logs_dir.as_ptr() as *const i8, std::ptr::null_mut());

        // Rotate old logs
        let log3 = b"Logs\\interact_debug.log.3\0";
        let log2 = b"Logs\\interact_debug.log.2\0";
        let log1 = b"Logs\\interact_debug.log.1\0";
        let log0 = b"Logs\\interact_debug.log\0";

        DeleteFileA(log3.as_ptr() as *const i8);
        MoveFileA(log2.as_ptr() as *const i8, log3.as_ptr() as *const i8);
        MoveFileA(log1.as_ptr() as *const i8, log2.as_ptr() as *const i8);
        MoveFileA(log0.as_ptr() as *const i8, log1.as_ptr() as *const i8);

        // Open new log file
        let handle = CreateFileA(
            log0.as_ptr() as *const i8,
            GENERIC_WRITE,
            FILE_SHARE_READ,
            std::ptr::null_mut(),
            CREATE_ALWAYS,
            FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        );

        // Store handle if valid
        if handle != 0 && handle as u32 != 0xFFFFFFFF {
            LOG_HANDLE.store(handle, Ordering::SeqCst);

            // Write an immediate test message
            let test_msg = b"[INIT] interact-rs logging initialized\r\n";
            let mut written: u32 = 0;
            WriteFile(
                handle,
                test_msg.as_ptr(),
                test_msg.len() as u32,
                &mut written,
                std::ptr::null_mut(),
            );
            FlushFileBuffers(handle);
        }
    }
}

/// Write a log message with timestamp
fn write_log(message: &str) {
    let handle = LOG_HANDLE.load(Ordering::SeqCst);
    if handle == 0 || handle == INVALID_HANDLE_VALUE {
        return;
    }

    let timestamp = get_timestamp();
    let line = format!("[{}] {}\r\n", timestamp, message);

    unsafe {
        let mut written: u32 = 0;
        WriteFile(
            handle,
            line.as_ptr(),
            line.len() as u32,
            &mut written,
            std::ptr::null_mut(),
        );
        FlushFileBuffers(handle);
    }
}

/// Log a debug message
pub fn log_debug(message: &str) {
    write_log(message);
}

/// Shutdown logging
pub fn shutdown() {
    let handle = LOG_HANDLE.swap(0, Ordering::SeqCst);
    if handle != 0 && handle != INVALID_HANDLE_VALUE {
        unsafe {
            CloseHandle(handle);
        }
    }
}

/// Get a timestamp string
fn get_timestamp() -> String {
    unsafe {
        let mut st: SystemTime = std::mem::zeroed();
        GetLocalTime(&mut st);
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
            st.year, st.month, st.day, st.hour, st.minute, st.second, st.milliseconds
        )
    }
}

/// Convenience macro for debug logging
#[macro_export]
macro_rules! debug_log {
    ($($arg:tt)*) => {
        $crate::logging::log_debug(&format!($($arg)*))
    };
}
