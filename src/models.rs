//! Data models for Geetest v4 captcha.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Supported captcha risk types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskType {
    /// Slide puzzle captcha
    Slide,
    /// Gobang (Five-in-a-row) puzzle
    Gobang,
    /// Icon selection captcha
    Icon,
    /// AI/Invisible captcha
    Ai,
}

impl RiskType {
    /// Returns the string representation for API calls.
    pub fn as_str(&self) -> &'static str {
        match self {
            RiskType::Slide => "slide",
            RiskType::Gobang => "gobang",
            RiskType::Icon => "icon",
            RiskType::Ai => "ai",
        }
    }
}

impl std::fmt::Display for RiskType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Response from successful captcha solve.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecCode {
    pub captcha_id: String,
    pub lot_number: String,
    pub pass_token: String,
    pub gen_time: String,
    pub captcha_output: String,
}

/// Raw response wrapper from Geetest API (JSONP format).
#[derive(Debug, Deserialize)]
pub struct GeetestResponse<T> {
    pub status: String,
    pub data: T,
}

/// Response from /load endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct LoadResponse {
    pub lot_number: String,
    pub payload: String,
    pub process_token: String,
    pub pt: String,
    pub pow_detail: PowDetail,
    // Slide-specific
    #[serde(default)]
    pub slice: Option<String>,
    #[serde(default)]
    pub bg: Option<String>,
    // Gobang-specific
    #[serde(default)]
    pub ques: Option<serde_json::Value>,
    // Icon-specific
    #[serde(default)]
    pub imgs: Option<String>,
}

/// Proof of Work details from server.
#[derive(Debug, Clone, Deserialize)]
pub struct PowDetail {
    pub hashfunc: String,
    pub version: String,
    pub bits: u32,
    pub datetime: String,
}

/// Proof of Work result.
#[derive(Debug, Clone, Serialize)]
pub struct PowResult {
    pub pow_msg: String,
    pub pow_sign: String,
}

/// Response from /verify endpoint.
#[derive(Debug, Deserialize)]
pub struct VerifyResponse {
    pub seccode: Option<SecCode>,
    #[serde(default)]
    pub result: Option<String>,
    /// Score from verification (can be string or integer)
    #[serde(default, deserialize_with = "deserialize_optional_string_or_int")]
    pub score: Option<String>,
    /// Updated payload for continue responses
    #[serde(default)]
    pub payload: Option<String>,
    /// Updated process_token for continue responses
    #[serde(default)]
    pub process_token: Option<String>,
    /// Updated payload_protocol for continue responses (can be string or integer)
    #[serde(default, deserialize_with = "deserialize_optional_string_or_int")]
    pub payload_protocol: Option<String>,
    /// Updated lot_number for continue responses
    #[serde(default)]
    pub lot_number: Option<String>,
}

/// Helper to deserialize fields that can be either string or integer
fn deserialize_optional_string_or_int<'de, D>(deserializer: D) -> std::result::Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Visitor};
    
    struct StringOrIntVisitor;
    
    impl<'de> Visitor<'de> for StringOrIntVisitor {
        type Value = Option<String>;
        
        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string, integer, or null")
        }
        
        fn visit_none<E>(self) -> std::result::Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }
        
        fn visit_unit<E>(self) -> std::result::Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }
        
        fn visit_str<E>(self, v: &str) -> std::result::Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(v.to_string()))
        }
        
        fn visit_string<E>(self, v: String) -> std::result::Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(v))
        }
        
        fn visit_i64<E>(self, v: i64) -> std::result::Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(v.to_string()))
        }
        
        fn visit_u64<E>(self, v: u64) -> std::result::Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(v.to_string()))
        }
    }
    
    deserializer.deserialize_any(StringOrIntVisitor)
}

/// Cached constants from deobfuscation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedConstants {
    /// Geetest script version (e.g., "v1.9.3-26b399")
    pub version: String,
    /// When the constants were fetched
    pub fetched_at: chrono::DateTime<chrono::Utc>,
    /// LotParser mapping pattern
    pub mapping: String,
    /// Additional constants (abo)
    pub abo: HashMap<String, String>,
    /// Device ID (usually empty)
    pub device_id: String,
}

/// Runtime constants used for signing.
#[derive(Debug, Clone)]
pub struct Constants {
    pub mapping: String,
    pub abo: HashMap<String, String>,
    pub device_id: String,
}

impl From<CachedConstants> for Constants {
    fn from(cached: CachedConstants) -> Self {
        Self {
            mapping: cached.mapping,
            abo: cached.abo,
            device_id: cached.device_id,
        }
    }
}
