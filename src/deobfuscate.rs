//! Auto-deobfuscation system for Geetest constants.
//!
//! This module automatically fetches and deobfuscates the latest Geetest
//! JavaScript to extract the required constants (mapping, abo, device_id).
//! Constants are cached locally and automatically refreshed when Geetest
//! updates their script.

use crate::error::{GeekedError, Result};
use crate::models::{CachedConstants, Constants};
use chrono::Utc;
use regex::Regex;
use std::collections::HashMap;
use std::path::PathBuf;

/// Deobfuscator for extracting Geetest constants.
pub struct Deobfuscator {
    cache_path: PathBuf,
}

impl Default for Deobfuscator {
    fn default() -> Self {
        Self::new()
    }
}

impl Deobfuscator {
    /// Create a new Deobfuscator with default cache location.
    pub fn new() -> Self {
        let cache_dir = directories::ProjectDirs::from("com", "geeked", "chaser-gt")
            .map(|dirs| dirs.cache_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from(".cache"));

        Self {
            cache_path: cache_dir.join("constants.json"),
        }
    }

    /// Create a Deobfuscator with a custom cache path.
    pub fn with_cache_path(cache_path: PathBuf) -> Self {
        Self { cache_path }
    }

    /// Get constants, using cache if valid or fetching fresh ones.
    pub async fn get_constants(&self) -> Result<Constants> {
        // Try to load from cache first
        if let Ok(Some(cached)) = self.load_cache() {
            // Check if the cached version is still current
            match self.fetch_current_version().await {
                Ok(current_version) => {
                    if cached.version == current_version {
                        tracing::debug!("Using cached constants (version: {})", cached.version);
                        return Ok(cached.into());
                    }
                    tracing::info!(
                        "Geetest version changed: {} -> {}, refreshing constants",
                        cached.version,
                        current_version
                    );
                }
                Err(e) => {
                    // If we can't check version, use cache anyway
                    tracing::warn!("Failed to check version, using cached constants: {}", e);
                    return Ok(cached.into());
                }
            }
        }

        // Fetch and deobfuscate fresh constants
        let constants = self.fetch_and_deobfuscate().await?;
        self.save_cache(&constants)?;
        Ok(constants.into())
    }

    /// Load cached constants from disk.
    fn load_cache(&self) -> Result<Option<CachedConstants>> {
        if !self.cache_path.exists() {
            return Ok(None);
        }

        let contents = std::fs::read_to_string(&self.cache_path)?;
        let cached: CachedConstants = serde_json::from_str(&contents)?;
        Ok(Some(cached))
    }

    /// Save constants to cache.
    fn save_cache(&self, constants: &CachedConstants) -> Result<()> {
        if let Some(parent) = self.cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = serde_json::to_string_pretty(constants)?;
        std::fs::write(&self.cache_path, contents)?;
        tracing::debug!("Saved constants to cache: {:?}", self.cache_path);
        Ok(())
    }

    /// Fetch the current Geetest script version without downloading the full script.
    async fn fetch_current_version(&self) -> Result<String> {
        let static_path = self.get_static_path().await?;
        // Extract version from path like "/geetest.gt.com/gcaptcha4/v1.9.3-26b399/js/..."
        let version = static_path
            .split('/')
            .nth(3)
            .ok_or_else(|| {
                GeekedError::Deobfuscation("Failed to extract version from path".into())
            })?
            .to_string();
        Ok(version)
    }

    /// Get the static path for the current Geetest script.
    async fn get_static_path(&self) -> Result<String> {
        // rquest v5 has TLS fingerprinting built-in by default
        let client = rquest::Client::new();

        let params = [
            ("callback", "geetest_1738850809870"),
            ("captcha_id", "588a5218557e1eadf33d682a6958c31b"),
            ("challenge", &uuid::Uuid::new_v4().to_string()),
            ("client_type", "web"),
            ("lang", "en"),
        ];

        let resp = client
            .get("https://gcaptcha4.geevisit.com/load")
            .query(&params)
            .send()
            .await?;

        let text = resp.text().await?;

        // Parse JSONP response: geetest_xxx({"status": "success", "data": {...}})
        let json_start = text
            .find('(')
            .ok_or_else(|| GeekedError::Deobfuscation("Invalid JSONP response format".into()))?
            + 1;
        let json_end = text
            .rfind(')')
            .ok_or_else(|| GeekedError::Deobfuscation("Invalid JSONP response format".into()))?;

        let json_str = &text[json_start..json_end];
        let response: serde_json::Value = serde_json::from_str(json_str)?;

        let static_path = response["data"]["static_path"]
            .as_str()
            .ok_or_else(|| GeekedError::Deobfuscation("Missing static_path in response".into()))?
            .to_string();

        Ok(static_path)
    }

    /// Fetch and deobfuscate the Geetest script to extract constants.
    async fn fetch_and_deobfuscate(&self) -> Result<CachedConstants> {
        let static_path = self.get_static_path().await?;
        let version = static_path
            .split('/')
            .nth(3)
            .ok_or_else(|| GeekedError::Deobfuscation("Failed to extract version".into()))?
            .to_string();

        tracing::info!("Fetching Geetest script version: {}", version);

        // rquest v5 has TLS fingerprinting built-in by default
        let client = rquest::Client::new();

        let script_url = format!("https://static.geevisit.com{}/js/gcaptcha4.js", static_path);
        let script = client.get(&script_url).send().await?.text().await?;

        // Extract XOR key and encrypted table
        let (encrypted_table, xor_key) = self.extract_table_and_key(&script)?;

        // Decrypt the lookup table
        let table = self.decrypt_table(&encrypted_table, &xor_key);

        // Replace obfuscated names in script
        let deobfuscated = self.replace_obfuscated_names(&script, &table)?;

        // Extract constants
        let abo = self.extract_abo(&deobfuscated)?;
        let mapping = self.extract_mapping(&deobfuscated)?;
        let device_id = self.extract_device_id(&deobfuscated);

        Ok(CachedConstants {
            version,
            fetched_at: Utc::now(),
            mapping,
            abo,
            device_id,
        })
    }

    /// Extract the encrypted table and XOR key from the script.
    fn extract_table_and_key(&self, script: &str) -> Result<(String, String)> {
        // Extract encrypted table from: decodeURI("...")
        let table_re = Regex::new(r#"decodeURI\("([^"]+)"\)"#)?;
        let encrypted_table = table_re
            .captures(script)
            .and_then(|c| c.get(1))
            .map(|m| {
                urlencoding::decode(m.as_str())
                    .unwrap_or_default()
                    .to_string()
            })
            .ok_or_else(|| {
                GeekedError::Deobfuscation("Failed to extract encrypted table".into())
            })?;

        // Extract XOR key from: }}}\("..."\)}
        let key_re = Regex::new(r#"\}\}\}\("([^"]+)"\)\}"#)?;
        let xor_key = key_re
            .captures(script)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .ok_or_else(|| GeekedError::Deobfuscation("Failed to extract XOR key".into()))?;

        Ok((encrypted_table, xor_key))
    }

    /// Decrypt the lookup table using XOR.
    fn decrypt_table(&self, encrypted: &str, key: &str) -> Vec<String> {
        let key_bytes = key.as_bytes();
        let decrypted: String = encrypted
            .chars()
            .enumerate()
            .map(|(i, c)| {
                let key_byte = key_bytes[i % key_bytes.len()];
                ((c as u8) ^ key_byte) as char
            })
            .collect();

        decrypted.split('^').map(String::from).collect()
    }

    /// Replace obfuscated function calls with actual strings.
    fn replace_obfuscated_names(&self, script: &str, table: &[String]) -> Result<String> {
        // Match patterns like: _xxxx(123)
        let re = Regex::new(r"(_.{4})\((\d+?)\)")?;
        let mut result = script.to_string();

        for cap in re.captures_iter(script) {
            if let (Some(full), Some(index_str)) = (cap.get(0), cap.get(2)) {
                if let Ok(index) = index_str.as_str().parse::<usize>() {
                    if let Some(replacement) = table.get(index) {
                        result = result.replace(full.as_str(), &format!("'{}'", replacement));
                    }
                }
            }
        }

        Ok(result)
    }

    /// Extract the abo constant from deobfuscated script.
    fn extract_abo(&self, script: &str) -> Result<HashMap<String, String>> {
        // Match: ['_lib']={...},
        let re = Regex::new(r"\['_lib']=(\{[^}]+\}),")?;
        let abo_str = re
            .captures(script)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str())
            .ok_or_else(|| GeekedError::Deobfuscation("Failed to extract abo constant".into()))?;

        // Clean up and parse as JSON
        // Convert 'key':'value' to "key":"value"
        let cleaned = abo_str.replace('\'', "\"");
        // Add quotes to unquoted keys
        let key_re = Regex::new(r"([{,])\s*([A-Za-z0-9_]+)\s*:")?;
        let json_str = key_re.replace_all(&cleaned, r#"$1"$2":"#);

        let abo: HashMap<String, String> = serde_json::from_str(&json_str).map_err(|e| {
            GeekedError::Deobfuscation(format!("Failed to parse abo as JSON: {}", e))
        })?;

        Ok(abo)
    }

    /// Extract the mapping constant from deobfuscated script.
    fn extract_mapping(&self, script: &str) -> Result<String> {
        // Match: ['_abo']=...}\()
        let re = Regex::new(r"\['_abo']=(.+?)\}\(\)")?;
        let mapping = re
            .captures(script)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .ok_or_else(|| {
                GeekedError::Deobfuscation("Failed to extract mapping constant".into())
            })?;

        Ok(mapping)
    }

    /// Extract the device_id from deobfuscated script.
    fn extract_device_id(&self, script: &str) -> String {
        // Match: ['options']['deviceId']='...'
        let re = Regex::new(r"\['options']\['deviceId']='([^']*)'").ok();
        re.and_then(|r| r.captures(script))
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decrypt_table() {
        let deob = Deobfuscator::new();

        // Simple test case
        let encrypted = "hello";
        let key = "key";
        let result = deob.decrypt_table(encrypted, key);

        // The decryption should produce some output
        assert!(!result.is_empty());
    }

    #[test]
    fn test_extract_abo_parsing() {
        let deob = Deobfuscator::new();

        // Simulate what the deobfuscated script might look like
        let script = r#"something['_lib']={'TYSC':'opMx'},other"#;
        let result = deob.extract_abo(script);

        assert!(result.is_ok());
        let abo = result.unwrap();
        assert_eq!(abo.get("TYSC"), Some(&"opMx".to_string()));
    }
}
