//! W parameter generation and LotParser for Geetest captcha.

use crate::crypto::{encrypt_w, generate_pow};
use crate::error::{GeekedError, Result};
use crate::models::{Constants, LoadResponse, RiskType};
use regex::Regex;
use serde_json::{json, Map, Value};

/// Parser for generating lot-number-derived dictionary values.
pub struct LotParser {
    lot: Vec<Vec<Vec<i32>>>,
    lot_res: Vec<Vec<Vec<i32>>>,
}

impl LotParser {
    /// Create a new LotParser from a mapping string.
    ///
    /// The mapping string format is like:
    /// `{"(n[13:15]+n[3:5])+.+(n[1:3]+n[26:28])+.+(n[20:27])":"n[13:18]"}`
    pub fn new(mapping: &str) -> Result<Self> {
        // Parse the mapping string to extract key and value patterns
        // Format can be {"pattern":"result"} or {"pattern":'result'} (mixed quotes)
        // Try double-double first, then double-single like Go
        let re = Regex::new(r#""([^"]+)":"([^"]+)""#)?;
        
        let caps = re.captures(mapping).or_else(|| {
            // Fallback: double quote key, single quote value
            Regex::new(r#""([^"]+)":'([^']+)'"#).ok()?.captures(mapping)
        }).ok_or_else(|| {
            GeekedError::Encryption(format!("Invalid mapping format: {}", mapping))
        })?;

        let key_pattern = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let value_pattern = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        
        tracing::debug!(key_pattern, value_pattern, "LotParser extracted patterns");

        let lot = Self::parse_pattern(key_pattern)?;
        let lot_res = Self::parse_pattern(value_pattern)?;

        Ok(Self { lot, lot_res })
    }

    /// Parse a pattern string like "(n[13:15]+n[3:5])+.+(n[1:3]+n[26:28])"
    fn parse_pattern(pattern: &str) -> Result<Vec<Vec<Vec<i32>>>> {
        let slice_re = Regex::new(r"\[(\d+):(\d+)\]")?;

        let parts: Vec<&str> = pattern.split("+.+").collect();
        let mut result = Vec::new();

        for part in parts {
            let mut group = Vec::new();

            // Split by '+' for concatenated slices within a group
            let subs: Vec<&str> = part.split('+').collect();

            for sub in subs {
                if let Some(caps) = slice_re.captures(sub) {
                    let start: i32 = caps
                        .get(1)
                        .and_then(|m| m.as_str().parse().ok())
                        .unwrap_or(0);
                    let end: i32 = caps
                        .get(2)
                        .and_then(|m| m.as_str().parse().ok())
                        .unwrap_or(0);
                    group.push(vec![start, end]);
                }
            }

            if !group.is_empty() {
                result.push(group);
            }
        }

        Ok(result)
    }

    /// Build a string from parsed pattern and lot number.
    fn build_string(parsed: &[Vec<Vec<i32>>], lot_number: &str) -> String {
        let chars: Vec<char> = lot_number.chars().collect();

        parsed
            .iter()
            .map(|group| {
                group
                    .iter()
                    .map(|slice| {
                        let start = slice[0] as usize;
                        let end = if slice.len() > 1 {
                            (slice[1] + 1) as usize
                        } else {
                            start + 1
                        };
                        chars
                            .get(start..end.min(chars.len()))
                            .map(|s| s.iter().collect::<String>())
                            .unwrap_or_default()
                    })
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join(".")
    }

    /// Generate a nested dictionary based on the lot number.
    ///
    /// For example, with lot_number "f4744c44df4541b3be48c5c270ced20b":
    /// - key string might be "4c44.44.c5c270c"
    /// - value string might be "4c44d"
    /// - result: {"4c44": {"44": {"c5c270c": "4c44d"}}}
    pub fn get_dict(&self, lot_number: &str) -> Value {
        let key_str = Self::build_string(&self.lot, lot_number);
        let value_str = Self::build_string(&self.lot_res, lot_number);

        let parts: Vec<&str> = key_str.split('.').collect();

        // Build nested structure
        let mut result = Value::Object(Map::new());

        if parts.is_empty() {
            return result;
        }

        // Navigate to create nested structure
        let mut current = &mut result;
        for (idx, part) in parts.iter().enumerate() {
            if idx == parts.len() - 1 {
                // Last part gets the value
                if let Value::Object(map) = current {
                    map.insert((*part).to_string(), Value::String(value_str.clone()));
                }
            } else {
                // Create nested object
                if let Value::Object(map) = current {
                    map.entry((*part).to_string())
                        .or_insert(Value::Object(Map::new()));
                    current = map.get_mut(*part).unwrap();
                }
            }
        }

        result
    }
}

/// Generate the W parameter for captcha verification.
pub fn generate_w_parameter(
    data: &LoadResponse,
    captcha_id: &str,
    _risk_type: RiskType,
    constants: &Constants,
    solver_result: Option<SolverResult>,
) -> Result<String> {
    let lot_number = &data.lot_number;

    // Parse the mapping to create LotParser
    let lot_parser = LotParser::new(&constants.mapping)?;

    // Generate PoW
    let pow_result = generate_pow(
        lot_number,
        captcha_id,
        &data.pow_detail.hashfunc,
        &data.pow_detail.version,
        data.pow_detail.bits,
        &data.pow_detail.datetime,
    );

    // Build base payload
    let mut payload = json!({
        "geetest": "captcha",
        "lang": "zh",
        "ep": "123",
        "biht": "1426265548",
        "device_id": "",  // Go version uses empty string
        "lot_number": lot_number,
        "pow_msg": pow_result.pow_msg,
        "pow_sign": pow_result.pow_sign,
        "em": {
            "cp": 0,
            "ek": "11",
            "nt": 0,
            "ph": 0,
            "sc": 0,
            "si": 0,
            "wd": 1
        },
        "gee_guard": {
            "roe": {
                "auh": "3",
                "aup": "3",
                "cdc": "3",
                "egp": "3",
                "res": "3",
                "rew": "3",
                "sep": "3",
                "snh": "3"
            }
        }
    });

    // Merge abo constants
    if let Value::Object(ref mut map) = payload {
        for (k, v) in &constants.abo {
            map.insert(k.clone(), Value::String(v.clone()));
        }
    }

    // Merge lot-derived values
    let lot_dict = lot_parser.get_dict(lot_number);
    if let (Value::Object(ref mut payload_map), Value::Object(lot_map)) = (&mut payload, lot_dict) {
        for (k, v) in lot_map {
            payload_map.insert(k, v);
        }
    }

    // Add solver-specific fields
    if let Some(result) = solver_result {
        match result {
            SolverResult::Slide { left } => {
                let passtime = rand::random::<u32>() % 600 + 600; // 600-1200ms
                let userresponse = left / 1.0059466666666665 + 2.0;

                if let Value::Object(ref mut map) = payload {
                    map.insert("passtime".to_string(), json!(passtime));
                    map.insert("setLeft".to_string(), json!(left));
                    map.insert("userresponse".to_string(), json!(userresponse));
                }
            }
            SolverResult::Gobang { response } => {
                if let Value::Object(ref mut map) = payload {
                    map.insert("userresponse".to_string(), json!(response));
                }
            }
            SolverResult::Icon { positions } => {
                let passtime = rand::random::<u32>() % 600 + 600;

                if let Value::Object(ref mut map) = payload {
                    map.insert("passtime".to_string(), json!(passtime));
                    map.insert("userresponse".to_string(), json!(positions));
                }
            }
            SolverResult::Ai => {
                // AI/invisible captcha doesn't need additional fields
            }
        }
    }

    // Serialize and encrypt
    let payload_str = serde_json::to_string(&payload)?;
    encrypt_w(&payload_str, &data.pt)
}

/// Result from a captcha solver.
#[derive(Debug, Clone)]
pub enum SolverResult {
    /// Slide captcha result with X position.
    Slide { left: f64 },
    /// Gobang result with move positions.
    Gobang { response: Vec<Vec<i32>> },
    /// Icon result with click positions.
    Icon { positions: Vec<Vec<f64>> },
    /// AI/invisible captcha (no user interaction).
    Ai,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lot_parser_creation() {
        let mapping = r#"{"(n[13:15]+n[3:5])+.+(n[1:3]+n[26:28])+.+(n[20:27])":"n[13:18]"}"#;
        let parser = LotParser::new(mapping);
        assert!(parser.is_ok());
    }

    #[test]
    fn test_lot_parser_get_dict() {
        let mapping = r#"{"(n[13:15]+n[3:5])+.+(n[1:3]+n[26:28])+.+(n[20:27])":"n[13:18]"}"#;
        let parser = LotParser::new(mapping).unwrap();

        // Test with a sample lot number
        let lot_number = "f4744c44df4541b3be48c5c270ced20b";
        let result = parser.get_dict(lot_number);

        // Should produce a nested object
        assert!(result.is_object());
    }

    #[test]
    fn test_parse_pattern() {
        let pattern = "(n[13:15]+n[3:5])+.+(n[1:3]+n[26:28])";
        let result = LotParser::parse_pattern(pattern).unwrap();

        // Should have 2 groups (separated by +.+)
        assert_eq!(result.len(), 2);
        // First group should have 2 slices (concatenated with +)
        assert_eq!(result[0].len(), 2);
    }
}
