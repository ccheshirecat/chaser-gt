//! RSA PKCS1v1.5 encryption for Geetest w parameter.

use num_bigint_dig::BigUint;
use rsa::{Pkcs1v15Encrypt, RsaPublicKey};

/// Geetest's RSA public key modulus (hex).
const MODULUS_HEX: &str = "00C1E3934D1614465B33053E7F48EE4EC87B14B95EF88947713D25EECBFF7E74C7977D02DC1D9451F79DD5D1C10C29ACB6A9B4D6FB7D0A0279B6719E1772565F09AF627715919221AEF91899CAE08C0D686D748B20A3603BE2318CA6BC2B59706592A9219D0BF05C9F65023A21D2330807252AE0066D59CEEFA5F2748EA80BAB81";

/// RSA public exponent.
const EXPONENT: u32 = 0x10001;

/// Encrypt a message using RSA PKCS1v1.5 with Geetest's public key.
///
/// # Arguments
/// * `message` - The message to encrypt (typically the random UID)
///
/// # Returns
/// Hex-encoded encrypted bytes
pub fn encrypt_rsa(message: &str) -> String {
    let n = BigUint::parse_bytes(MODULUS_HEX.as_bytes(), 16).expect("Failed to parse RSA modulus");
    let e = BigUint::from(EXPONENT);

    let public_key = RsaPublicKey::new(n, e).expect("Failed to construct RSA public key");

    let mut rng = rand::thread_rng();
    let encrypted = public_key
        .encrypt(&mut rng, Pkcs1v15Encrypt, message.as_bytes())
        .expect("RSA encryption failed");

    hex::encode(encrypted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rsa_encryption_output_length() {
        let message = "56e508d726649e0d";
        let encrypted = encrypt_rsa(message);

        // RSA-1024 produces 128 bytes = 256 hex chars
        assert_eq!(encrypted.len(), 256);
    }

    #[test]
    fn test_rsa_encryption_is_random() {
        let message = "testmessage12345";

        let enc1 = encrypt_rsa(message);
        let enc2 = encrypt_rsa(message);

        // PKCS1v1.5 uses random padding, so same message produces different output
        assert_ne!(enc1, enc2);
    }

    #[test]
    fn test_rsa_encryption_hex_output() {
        let message = "test";
        let encrypted = encrypt_rsa(message);

        // Should be valid hex
        assert!(encrypted.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
