//! C FFI bindings for chaser-gt.
//!
//! Provides a simple blocking API for solving Geetest v4 captchas from C, Python, Go, etc.
//!
//! # Example (C)
//!
//! ```c
//! #include "chaser_gt.h"
//!
//! int main() {
//!     char* result = geeked_solve("captcha_id", "slide", NULL, NULL);
//!     if (result) {
//!         printf("Result: %s\n", result);
//!         geeked_free_string(result);
//!     }
//!     return 0;
//! }
//! ```

use std::ffi::{c_char, CStr, CString};
use std::ptr;

use crate::{Geeked, RiskType};

/// Result structure returned by solve functions.
///
/// All string fields are heap-allocated and must be freed with `geeked_free_result`.
#[repr(C)]
pub struct GeekedResult {
    /// 0 = success, non-zero = error
    pub error_code: i32,
    /// Error message if error_code != 0, NULL otherwise
    pub error_message: *mut c_char,
    /// Captcha ID used
    pub captcha_id: *mut c_char,
    /// Lot number from Geetest
    pub lot_number: *mut c_char,
    /// Pass token for verification
    pub pass_token: *mut c_char,
    /// Generation timestamp
    pub gen_time: *mut c_char,
    /// Encrypted captcha output
    pub captcha_output: *mut c_char,
}

impl GeekedResult {
    fn success(
        captcha_id: String,
        lot_number: String,
        pass_token: String,
        gen_time: String,
        captcha_output: String,
    ) -> Self {
        Self {
            error_code: 0,
            error_message: ptr::null_mut(),
            captcha_id: string_to_ptr(captcha_id),
            lot_number: string_to_ptr(lot_number),
            pass_token: string_to_ptr(pass_token),
            gen_time: string_to_ptr(gen_time),
            captcha_output: string_to_ptr(captcha_output),
        }
    }

    fn error(code: i32, message: String) -> Self {
        Self {
            error_code: code,
            error_message: string_to_ptr(message),
            captcha_id: ptr::null_mut(),
            lot_number: ptr::null_mut(),
            pass_token: ptr::null_mut(),
            gen_time: ptr::null_mut(),
            captcha_output: ptr::null_mut(),
        }
    }
}

/// Convert Rust String to C string pointer.
fn string_to_ptr(s: String) -> *mut c_char {
    CString::new(s)
        .map(|cs| cs.into_raw())
        .unwrap_or(ptr::null_mut())
}

/// Convert C string to Rust String, returns None if null or invalid UTF-8.
unsafe fn ptr_to_string(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    CStr::from_ptr(ptr).to_str().ok().map(|s| s.to_string())
}

/// Parse risk type string to enum.
fn parse_risk_type(s: &str) -> Option<RiskType> {
    match s.to_lowercase().as_str() {
        "slide" => Some(RiskType::Slide),
        "gobang" => Some(RiskType::Gobang),
        "icon" => Some(RiskType::Icon),
        "ai" | "invisible" => Some(RiskType::Ai),
        _ => None,
    }
}

/// Solve a Geetest v4 captcha (blocking).
///
/// # Parameters
///
/// - `captcha_id`: The Geetest captcha ID (required)
/// - `risk_type`: Captcha type: "slide", "gobang", "icon", or "ai" (required)
/// - `proxy`: Optional proxy URL (e.g., "http://user:pass@host:port" or "socks5://host:port")
/// - `user_info`: Optional user info for site-specific binding
///
/// # Returns
///
/// A `GeekedResult` struct. Check `error_code` for success (0) or failure (non-zero).
/// The caller must free the result with `geeked_free_result`.
///
/// # Safety
///
/// - `captcha_id` must be a valid null-terminated C string
/// - `risk_type` must be a valid null-terminated C string
/// - `proxy` must be NULL or a valid null-terminated C string
/// - `user_info` must be NULL or a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn geeked_solve(
    captcha_id: *const c_char,
    risk_type: *const c_char,
    proxy: *const c_char,
    user_info: *const c_char,
) -> GeekedResult {
    // Parse captcha_id
    let captcha_id = match ptr_to_string(captcha_id) {
        Some(s) if !s.is_empty() => s,
        _ => return GeekedResult::error(1, "captcha_id is required".to_string()),
    };

    // Parse risk_type
    let risk_type_str = match ptr_to_string(risk_type) {
        Some(s) => s,
        None => return GeekedResult::error(2, "risk_type is required".to_string()),
    };

    let risk_type = match parse_risk_type(&risk_type_str) {
        Some(rt) => rt,
        None => {
            return GeekedResult::error(
                3,
                format!(
                    "Invalid risk_type '{}'. Valid values: slide, gobang, icon, ai",
                    risk_type_str
                ),
            )
        }
    };

    // Parse optional proxy
    let proxy = ptr_to_string(proxy);

    // Parse optional user_info
    let user_info = ptr_to_string(user_info);

    // Create tokio runtime for blocking call
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => return GeekedResult::error(4, format!("Failed to create runtime: {}", e)),
    };

    // Run the async solve
    runtime.block_on(async {
        // Build the solver
        let mut builder = Geeked::builder(&captcha_id, risk_type);

        if let Some(p) = proxy {
            builder = builder.proxy(p);
        }

        if let Some(ui) = user_info {
            builder = builder.user_info(ui);
        }

        let solver = match builder.build().await {
            Ok(s) => s,
            Err(e) => return GeekedResult::error(5, format!("Failed to build solver: {}", e)),
        };

        // Solve
        match solver.solve().await {
            Ok(result) => GeekedResult::success(
                result.captcha_id,
                result.lot_number,
                result.pass_token,
                result.gen_time,
                result.captcha_output,
            ),
            Err(e) => GeekedResult::error(6, format!("Solve failed: {}", e)),
        }
    })
}

/// Solve a Geetest v4 captcha and return JSON (blocking).
///
/// This is a simpler alternative that returns a JSON string.
///
/// # Returns
///
/// A JSON string on success:
/// ```json
/// {"success": true, "captcha_id": "...", "lot_number": "...", "pass_token": "...", "gen_time": "...", "captcha_output": "..."}
/// ```
///
/// Or on error:
/// ```json
/// {"success": false, "error": "error message"}
/// ```
///
/// The caller must free the string with `geeked_free_string`.
///
/// # Safety
///
/// - `captcha_id` must be a valid null-terminated C string
/// - `risk_type` must be a valid null-terminated C string  
/// - `proxy` must be NULL or a valid null-terminated C string
/// - `user_info` must be NULL or a valid null-terminated C string
#[no_mangle]
pub unsafe extern "C" fn geeked_solve_json(
    captcha_id: *const c_char,
    risk_type: *const c_char,
    proxy: *const c_char,
    user_info: *const c_char,
) -> *mut c_char {
    let result = geeked_solve(captcha_id, risk_type, proxy, user_info);

    let json = if result.error_code == 0 {
        let captcha_id = ptr_to_string(result.captcha_id).unwrap_or_default();
        let lot_number = ptr_to_string(result.lot_number).unwrap_or_default();
        let pass_token = ptr_to_string(result.pass_token).unwrap_or_default();
        let gen_time = ptr_to_string(result.gen_time).unwrap_or_default();
        let captcha_output = ptr_to_string(result.captcha_output).unwrap_or_default();

        // Free the result strings since we've copied them
        geeked_free_result(result);

        serde_json::json!({
            "success": true,
            "captcha_id": captcha_id,
            "lot_number": lot_number,
            "pass_token": pass_token,
            "gen_time": gen_time,
            "captcha_output": captcha_output
        })
        .to_string()
    } else {
        let error =
            ptr_to_string(result.error_message).unwrap_or_else(|| "Unknown error".to_string());
        geeked_free_result(result);

        serde_json::json!({
            "success": false,
            "error": error
        })
        .to_string()
    };

    string_to_ptr(json)
}

/// Free a GeekedResult structure.
///
/// # Safety
///
/// - `result` must be a valid GeekedResult previously returned by `geeked_solve`
/// - Each result must only be freed once
#[no_mangle]
pub unsafe extern "C" fn geeked_free_result(result: GeekedResult) {
    if !result.error_message.is_null() {
        let _ = CString::from_raw(result.error_message);
    }
    if !result.captcha_id.is_null() {
        let _ = CString::from_raw(result.captcha_id);
    }
    if !result.lot_number.is_null() {
        let _ = CString::from_raw(result.lot_number);
    }
    if !result.pass_token.is_null() {
        let _ = CString::from_raw(result.pass_token);
    }
    if !result.gen_time.is_null() {
        let _ = CString::from_raw(result.gen_time);
    }
    if !result.captcha_output.is_null() {
        let _ = CString::from_raw(result.captcha_output);
    }
}

/// Free a string returned by chaser-gt FFI functions.
///
/// # Safety
///
/// - `s` must be NULL or a valid pointer previously returned by chaser-gt
/// - Each string must only be freed once
#[no_mangle]
pub unsafe extern "C" fn geeked_free_string(s: *mut c_char) {
    if !s.is_null() {
        let _ = CString::from_raw(s);
    }
}

/// Get the library version.
///
/// # Returns
///
/// A static string with the version number. Do NOT free this string.
#[no_mangle]
pub extern "C" fn geeked_version() -> *const c_char {
    // This is a static string, no need to free
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr() as *const c_char
}
