//! AES-CBC encryption for Geetest w parameter.

use aes::Aes128;
use cbc::cipher::{block_padding::Pkcs7, BlockEncryptMut, KeyIvInit};

type Aes128CbcEnc = cbc::Encryptor<Aes128>;

/// Encrypt plaintext using AES-128-CBC with PKCS7 padding.
///
/// # Arguments
/// * `plaintext` - The text to encrypt
/// * `key` - 16-character key string
///
/// # Returns
/// Encrypted bytes
pub fn encrypt_aes_cbc(plaintext: &str, key: &str) -> Vec<u8> {
    let key_bytes = key.as_bytes();
    // Geetest uses a static IV of all zeros (as string "0000000000000000")
    let iv = b"0000000000000000";

    let cipher = Aes128CbcEnc::new(key_bytes.into(), iv.into());
    cipher.encrypt_padded_vec_mut::<Pkcs7>(plaintext.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aes_encryption() {
        let key = "56e508d726649e0d";
        let plaintext = "Hello world!";
        let encrypted = encrypt_aes_cbc(plaintext, key);

        // Should produce some output
        assert!(!encrypted.is_empty());
        // AES block size is 16, so output should be multiple of 16
        assert_eq!(encrypted.len() % 16, 0);
    }

    #[test]
    fn test_aes_deterministic() {
        let key = "56e508d726649e0d";
        let plaintext = "test message";

        let enc1 = encrypt_aes_cbc(plaintext, key);
        let enc2 = encrypt_aes_cbc(plaintext, key);

        // Same key + plaintext + IV should produce same output
        assert_eq!(enc1, enc2);
    }
}
