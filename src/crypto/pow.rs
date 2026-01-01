//! Proof of Work generation for Geetest captcha.

use md5::{Digest as Md5Digest, Md5};
use sha1::Sha1;
use sha2::Sha256;

use super::rand_uid;

/// Result of PoW computation.
#[derive(Debug, Clone)]
pub struct PowResult {
    pub pow_msg: String,
    pub pow_sign: String,
}

/// Generate Proof of Work for Geetest captcha.
///
/// This brute-forces a nonce that produces a hash with the required number
/// of leading zero bits.
///
/// # Arguments
/// * `lot_number` - Lot number from captcha load response
/// * `captcha_id` - Captcha ID
/// * `hash_func` - Hash function to use ("md5", "sha1", "sha256")
/// * `version` - PoW version string
/// * `bits` - Number of leading zero bits required
/// * `datetime` - Datetime string from server
///
/// # Returns
/// PoW message and signature
pub fn generate_pow(
    lot_number: &str,
    captcha_id: &str,
    hash_func: &str,
    version: &str,
    bits: u32,
    datetime: &str,
) -> PowResult {
    let bit_division = (bits / 4) as usize;
    let bit_remainder = bits % 4;
    let prefix = "0".repeat(bit_division);

    let pow_base = format!(
        "{}|{}|{}|{}|{}|{}||",
        version, bits, hash_func, datetime, captcha_id, lot_number
    );

    loop {
        let nonce = rand_uid();
        let pow_msg = format!("{}{}", pow_base, nonce);

        let hash = match hash_func {
            "md5" => {
                let mut hasher = Md5::new();
                hasher.update(pow_msg.as_bytes());
                hex::encode(hasher.finalize())
            }
            "sha1" => {
                let mut hasher = Sha1::new();
                hasher.update(pow_msg.as_bytes());
                hex::encode(hasher.finalize())
            }
            "sha256" => {
                let mut hasher = Sha256::new();
                hasher.update(pow_msg.as_bytes());
                hex::encode(hasher.finalize())
            }
            _ => panic!("Unsupported hash function: {}", hash_func),
        };

        if verify_pow(&hash, &prefix, bit_remainder, bit_division) {
            return PowResult {
                pow_msg,
                pow_sign: hash,
            };
        }
    }
}

/// Verify if a hash meets the PoW requirements.
fn verify_pow(hash: &str, prefix: &str, bit_remainder: u32, bit_division: usize) -> bool {
    if !hash.starts_with(prefix) {
        return false;
    }

    if bit_remainder == 0 {
        return true;
    }

    // Check the next hex digit after the prefix
    if let Some(next_char) = hash.chars().nth(bit_division) {
        let threshold = match bit_remainder {
            1 => '7',
            2 => '3',
            3 => '1',
            _ => return false,
        };
        return next_char <= threshold;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_pow_zero_bits() {
        // With 0 bits required, should return immediately
        let result = generate_pow(
            "test_lot_number",
            "test_captcha_id",
            "md5",
            "1",
            0,
            "2025-01-01T00:00:00+00:00",
        );

        assert!(!result.pow_msg.is_empty());
        assert!(!result.pow_sign.is_empty());
        assert_eq!(result.pow_sign.len(), 32); // MD5 produces 32 hex chars
    }

    #[test]
    fn test_generate_pow_with_bits() {
        // With 4 bits (1 leading zero hex digit)
        let result = generate_pow(
            "test_lot_number",
            "test_captcha_id",
            "md5",
            "1",
            4,
            "2025-01-01T00:00:00+00:00",
        );

        assert!(result.pow_sign.starts_with('0'));
    }

    #[test]
    fn test_verify_pow() {
        // Test with exact prefix match
        assert!(verify_pow("0000abc", "0000", 0, 4));
        assert!(!verify_pow("000abc", "0000", 0, 4));

        // Test with bit remainder
        assert!(verify_pow("0007abc", "000", 1, 3)); // 7 <= 7
        assert!(!verify_pow("0008abc", "000", 1, 3)); // 8 > 7

        assert!(verify_pow("0003abc", "000", 2, 3)); // 3 <= 3
        assert!(!verify_pow("0004abc", "000", 2, 3)); // 4 > 3

        assert!(verify_pow("0001abc", "000", 3, 3)); // 1 <= 1
        assert!(!verify_pow("0002abc", "000", 3, 3)); // 2 > 1
    }
}
