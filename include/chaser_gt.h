/**
 * chaser-gt C FFI bindings
 *
 * High-performance Geetest v4 captcha solver.
 *
 * Build with: cargo build --release --features ffi
 * Link with: -lchaser_gt -lpthread -ldl -lm
 */


#ifndef CHASER_GT_H
#define CHASER_GT_H

#include <stdint.h>
#include <stdbool.h>

/**
 * Result structure returned by solve functions.
 *
 * All string fields are heap-allocated and must be freed with `geeked_free_result`.
 */
typedef struct GeekedResult {
  /**
   * 0 = success, non-zero = error
   */
  int32_t error_code;
  /**
   * Error message if error_code != 0, NULL otherwise
   */
  char *error_message;
  /**
   * Captcha ID used
   */
  char *captcha_id;
  /**
   * Lot number from Geetest
   */
  char *lot_number;
  /**
   * Pass token for verification
   */
  char *pass_token;
  /**
   * Generation timestamp
   */
  char *gen_time;
  /**
   * Encrypted captcha output
   */
  char *captcha_output;
} GeekedResult;

/**
 * Solve a Geetest v4 captcha (blocking).
 *
 * # Parameters
 *
 * - `captcha_id`: The Geetest captcha ID (required)
 * - `risk_type`: Captcha type: "slide", "gobang", "icon", or "ai" (required)
 * - `proxy`: Optional proxy URL (e.g., "http://user:pass@host:port" or "socks5://host:port")
 * - `user_info`: Optional user info for site-specific binding
 *
 * # Returns
 *
 * A `GeekedResult` struct. Check `error_code` for success (0) or failure (non-zero).
 * The caller must free the result with `geeked_free_result`.
 *
 * # Safety
 *
 * - `captcha_id` must be a valid null-terminated C string
 * - `risk_type` must be a valid null-terminated C string
 * - `proxy` must be NULL or a valid null-terminated C string
 * - `user_info` must be NULL or a valid null-terminated C string
 */

struct GeekedResult geeked_solve(const char *captcha_id,
                                 const char *risk_type,
                                 const char *proxy,
                                 const char *user_info);

/**
 * Solve a Geetest v4 captcha and return JSON (blocking).
 *
 * This is a simpler alternative that returns a JSON string.
 *
 * # Returns
 *
 * A JSON string on success:
 * ```json
 * {"success": true, "captcha_id": "...", "lot_number": "...", "pass_token": "...", "gen_time": "...", "captcha_output": "..."}
 * ```
 *
 * Or on error:
 * ```json
 * {"success": false, "error": "error message"}
 * ```
 *
 * The caller must free the string with `geeked_free_string`.
 *
 * # Safety
 *
 * - `captcha_id` must be a valid null-terminated C string
 * - `risk_type` must be a valid null-terminated C string
 * - `proxy` must be NULL or a valid null-terminated C string
 * - `user_info` must be NULL or a valid null-terminated C string
 */

char *geeked_solve_json(const char *captcha_id,
                        const char *risk_type,
                        const char *proxy,
                        const char *user_info);

/**
 * Free a GeekedResult structure.
 *
 * # Safety
 *
 * - `result` must be a valid GeekedResult previously returned by `geeked_solve`
 * - Each result must only be freed once
 */
 void geeked_free_result(struct GeekedResult result);

/**
 * Free a string returned by chaser-gt FFI functions.
 *
 * # Safety
 *
 * - `s` must be NULL or a valid pointer previously returned by chaser-gt
 * - Each string must only be freed once
 */
 void geeked_free_string(char *s);

/**
 * Get the library version.
 *
 * # Returns
 *
 * A static string with the version number. Do NOT free this string.
 */
 const char *geeked_version(void);

#endif /* CHASER_GT_H */
