//! Cryptography module for Geetest w parameter encryption.

mod aes_enc;
mod pow;
mod rsa_enc;

pub use aes_enc::encrypt_aes_cbc;
pub use pow::{generate_pow, PowResult};
pub use rsa_enc::encrypt_rsa;

/// Generate a random 16-character hex string (like Python's rand_uid).
pub fn rand_uid() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut result = String::with_capacity(16);
    for _ in 0..4 {
        let val: u16 = rng.gen_range(0x1000..=0xFFFF);
        result.push_str(&format!("{:04x}", val));
    }
    result
}

/// Encrypt the w parameter for Geetest.
pub fn encrypt_w(raw_input: &str, pt: &str) -> crate::error::Result<String> {
    if pt.is_empty() || pt == "0" {
        return Ok(urlencoding::encode(raw_input).to_string());
    }

    let random_uid = rand_uid();

    match pt {
        "1" => {
            let enc_key = encrypt_rsa(&random_uid);
            let enc_input = encrypt_aes_cbc(raw_input, &random_uid);
            Ok(hex::encode(enc_input) + &enc_key)
        }
        "2" => Err(crate::error::GeekedError::Encryption(
            "Encryption type 2 (SM2) is not implemented yet".to_string(),
        )),
        _ => Err(crate::error::GeekedError::Encryption(format!(
            "Unknown encryption type: {}",
            pt
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rand_uid_length() {
        let uid = rand_uid();
        assert_eq!(uid.len(), 16);
        assert!(uid.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
