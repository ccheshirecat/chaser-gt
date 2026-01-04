//! Main Geeked client for solving Geetest v4 captchas.

use crate::deobfuscate::Deobfuscator;
use crate::error::{GeekedError, Result};
use crate::models::{Constants, GeetestResponse, LoadResponse, RiskType, SecCode, VerifyResponse};
use crate::sign::{generate_w_parameter, SolverResult};
use crate::solvers::{GobangSolver, SlideSolver};
use rquest::{Client, Proxy};
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Builder for creating a Geeked client.
pub struct GeekedBuilder {
    captcha_id: String,
    risk_type: RiskType,
    proxy: Option<String>,
    user_info: Option<String>,
    local_address: Option<IpAddr>,
}

impl GeekedBuilder {
    /// Create a new builder with required parameters.
    pub fn new(captcha_id: impl Into<String>, risk_type: RiskType) -> Self {
        Self {
            captcha_id: captcha_id.into(),
            risk_type,
            proxy: None,
            user_info: None,
            local_address: None,
        }
    }

    /// Set HTTP/SOCKS5 proxy.
    ///
    /// # Examples
    /// ```ignore
    /// .proxy("http://user:pass@host:port")
    /// .proxy("socks5://127.0.0.1:1080")
    /// ```
    pub fn proxy(mut self, proxy: impl Into<String>) -> Self {
        self.proxy = Some(proxy.into());
        self
    }

    /// Set local address to bind outgoing connections to.
    ///
    /// This is useful for routing traffic through a specific network interface
    /// or IPv6 address from a BGP subnet.
    ///
    /// # Examples
    /// ```ignore
    /// use std::net::IpAddr;
    /// 
    /// .local_address("2a11:29c0:4f50::1".parse().unwrap())
    /// .local_address(IpAddr::V6("::1".parse().unwrap()))
    /// ```
    pub fn local_address(mut self, addr: IpAddr) -> Self {
        self.local_address = Some(addr);
        self
    }

    /// Set user_info for site-specific binding.
    ///
    /// Some sites require a user_info parameter to bind the captcha
    /// verification to a specific user/session/account.
    ///
    /// # Examples
    /// ```ignore
    /// .user_info("account_id=12345")
    /// .user_info(serde_json::to_string(&user_data).unwrap())
    /// ```
    pub fn user_info(mut self, user_info: impl Into<String>) -> Self {
        self.user_info = Some(user_info.into());
        self
    }

    /// Build the Geeked client.
    pub async fn build(self) -> Result<Geeked> {
        // rquest v5 has TLS fingerprinting built-in by default
        let mut builder = Client::builder();

        // Set local address for IPv6 binding
        if let Some(addr) = self.local_address {
            builder = builder.local_address(addr);
        }

        if let Some(proxy_url) = &self.proxy {
            builder = builder.proxy(Proxy::all(proxy_url)?);
        }

        let client = builder.build()?;

        // Auto-fetch and cache constants
        let deobfuscator = Deobfuscator::new();
        let constants = deobfuscator.get_constants().await?;

        Ok(Geeked {
            client,
            captcha_id: self.captcha_id,
            risk_type: self.risk_type,
            challenge: uuid::Uuid::new_v4().to_string(),
            constants: Arc::new(constants),
            user_info: self.user_info,
        })
    }
}

/// Geeked captcha solver client.
///
/// # Example
/// ```ignore
/// use chaser_gt::{Geeked, RiskType};
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let solver = Geeked::builder("your_captcha_id", RiskType::Slide)
///         .proxy("http://127.0.0.1:8080")
///         .build()
///         .await?;
///
///     let result = solver.solve().await?;
///     println!("Solved: {:?}", result);
///     Ok(())
/// }
/// ```
pub struct Geeked {
    client: Client,
    captcha_id: String,
    risk_type: RiskType,
    challenge: String,
    constants: Arc<Constants>,
    user_info: Option<String>,
}

impl Geeked {
    /// Create a builder for the Geeked client.
    pub fn builder(captcha_id: impl Into<String>, risk_type: RiskType) -> GeekedBuilder {
        GeekedBuilder::new(captcha_id, risk_type)
    }

    /// Generate a random callback string.
    /// Format matches Python: geetest_{random + timestamp}
    fn random_callback() -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let random = (rand::random::<f64>() * 10000.0) as u64;
        format!("geetest_{}", random + timestamp)
    }

    /// Parse JSONP response from Geetest.
    fn parse_jsonp<T: serde::de::DeserializeOwned>(response: &str, callback: &str) -> Result<T> {
        // Format: callback({"status": "success", "data": {...}})
        let prefix = format!("{}(", callback);
        let json_start = response.find(&prefix).ok_or_else(|| {
            tracing::error!(
                "Invalid JSONP response: {}",
                &response[..response.len().min(200)]
            );
            GeekedError::InvalidResponse("Invalid JSONP format".into())
        })? + prefix.len();
        let json_end = response.len() - 1; // Remove trailing ')'

        let json_str = &response[json_start..json_end];
        let wrapper: GeetestResponse<T> = serde_json::from_str(json_str)?;

        if wrapper.status != "success" {
            return Err(GeekedError::InvalidResponse(format!(
                "Geetest returned status: {}",
                wrapper.status
            )));
        }

        Ok(wrapper.data)
    }

    /// Load captcha data from Geetest server.
    async fn load_captcha(&self) -> Result<LoadResponse> {
        let callback = Self::random_callback();

        let mut params = vec![
            ("captcha_id", self.captcha_id.as_str()),
            ("challenge", self.challenge.as_str()),
            ("client_type", "web"),
            ("risk_type", self.risk_type.as_str()),
            ("lang", "eng"),
            ("callback", callback.as_str()),
        ];

        // Add user_info if provided (for site-specific binding)
        if let Some(ref user_info) = self.user_info {
            params.push(("user_info", user_info.as_str()));
        }

        let response = self
            .client
            .get("https://gcaptcha4.geevisit.com/load")
            .query(&params)
            .send()
            .await?
            .text()
            .await?;

        Self::parse_jsonp(&response, &callback)
    }

    /// Download image from Geetest static server.
    async fn download_image(&self, path: &str) -> Result<Vec<u8>> {
        let url = format!("https://static.geetest.com/{}", path);
        let bytes = self.client.get(&url).send().await?.bytes().await?;
        Ok(bytes.to_vec())
    }

    /// Solve the captcha based on risk type.
    async fn solve_captcha(&self, data: &LoadResponse) -> Result<SolverResult> {
        match self.risk_type {
            RiskType::Slide => {
                let slice_path = data.slice.as_ref().ok_or_else(|| {
                    GeekedError::InvalidResponse("Missing slice path for slide captcha".into())
                })?;
                let bg_path = data.bg.as_ref().ok_or_else(|| {
                    GeekedError::InvalidResponse("Missing bg path for slide captcha".into())
                })?;

                let (slice_bytes, bg_bytes) = tokio::try_join!(
                    self.download_image(slice_path),
                    self.download_image(bg_path)
                )?;

                let solver = SlideSolver::from_bytes(&slice_bytes, &bg_bytes)?;
                let position = solver.find_position();

                // Add small random variation
                let variation: f64 = rand::random::<f64>() * 0.5;
                Ok(SolverResult::Slide {
                    left: position + variation,
                })
            }

            RiskType::Gobang => {
                let ques = data.ques.as_ref().ok_or_else(|| {
                    GeekedError::InvalidResponse("Missing ques for gobang captcha".into())
                })?;

                // Parse the board from JSON
                let board: Vec<Vec<i32>> = serde_json::from_value(ques.clone())?;
                let solver = GobangSolver::new(board);

                let result =
                    solver
                        .find_four_in_line()
                        .ok_or_else(|| GeekedError::VerificationFailed {
                            message: "Could not solve gobang puzzle".into(),
                        })?;

                Ok(SolverResult::Gobang {
                    response: vec![
                        vec![result[0][0], result[0][1]],
                        vec![result[1][0], result[1][1]],
                    ],
                })
            }

            RiskType::Icon => {
                #[cfg(feature = "icon")]
                {
                    use crate::solvers::IconSolver;

                    let imgs_path = data.imgs.as_ref().ok_or_else(|| {
                        GeekedError::InvalidResponse("Missing imgs path for icon captcha".into())
                    })?;
                    let ques = data.ques.as_ref().ok_or_else(|| {
                        GeekedError::InvalidResponse("Missing ques for icon captcha".into())
                    })?;

                    let questions: Vec<String> = serde_json::from_value(ques.clone())?;
                    let img_bytes = self.download_image(imgs_path).await?;

                    let mut solver = IconSolver::new()?;
                    let positions = solver.find_icon_positions(&img_bytes, &questions)?;

                    Ok(SolverResult::Icon {
                        positions: positions.into_iter().map(|p| vec![p[0], p[1]]).collect(),
                    })
                }

                #[cfg(not(feature = "icon"))]
                {
                    Err(GeekedError::UnsupportedType(
                        "Icon captcha requires 'icon' feature to be enabled".into(),
                    ))
                }
            }

            RiskType::Ai => {
                // AI/invisible captcha doesn't need solving
                Ok(SolverResult::Ai)
            }
        }
    }

    /// Submit the solved captcha to Geetest server.
    /// Returns the full VerifyResponse to allow handling "continue" responses.
    async fn submit_captcha(
        &self,
        lot_number: &str,
        payload: &str,
        process_token: &str,
        w: &str,
    ) -> Result<VerifyResponse> {
        let callback = Self::random_callback();

        let params = [
            ("callback", callback.as_str()),
            ("captcha_id", self.captcha_id.as_str()),
            ("client_type", "web"),
            ("lot_number", lot_number),
            ("risk_type", self.risk_type.as_str()),
            ("payload", payload),
            ("process_token", process_token),
            ("payload_protocol", "1"),
            ("pt", "1"),
            ("w", w),
        ];

        let response = self
            .client
            .get("https://gcaptcha4.geevisit.com/verify")
            .query(&params)
            .send()
            .await?
            .text()
            .await?;

        Self::parse_jsonp(&response, &callback)
    }

    /// Solve the captcha and return the security code.
    ///
    /// This is the main entry point for solving captchas.
    ///
    /// # Returns
    /// A `SecCode` containing all the verification tokens needed to prove
    /// the captcha was solved.
    ///
    /// # Note
    /// Some sites use multi-round verification where Geetest returns
    /// `result: "continue"` with updated payload/process_token. This method
    /// automatically handles the retry loop.
    pub async fn solve(&self) -> Result<SecCode> {
        // Load captcha data
        let data = self.load_captcha().await?;

        tracing::debug!(
            "Loaded captcha: lot_number={}, pt={}",
            data.lot_number,
            data.pt
        );

        // Solve based on risk type
        let solver_result = self.solve_captcha(&data).await?;

        // Generate W parameter
        let w = generate_w_parameter(
            &data,
            &self.captcha_id,
            self.risk_type,
            &self.constants,
            Some(solver_result),
        )?;

        // Track mutable state for continue loop
        let mut lot_number = data.lot_number.clone();
        let mut payload = data.payload.clone();
        let mut process_token = data.process_token.clone();
        let mut current_w = w;

        // Retry loop for "continue" responses
        const MAX_RETRIES: u32 = 10;
        for attempt in 0..MAX_RETRIES {
            let verify_response = self
                .submit_captcha(&lot_number, &payload, &process_token, &current_w)
                .await?;

            // Success - got seccode
            if let Some(seccode) = verify_response.seccode {
                tracing::debug!("Captcha solved on attempt {}", attempt + 1);
                return Ok(seccode);
            }

            // Check for "continue" response
            if verify_response.result.as_deref() == Some("continue") {
                tracing::debug!(
                    "Received 'continue' response on attempt {}, retrying...",
                    attempt + 1
                );

                // Update state with new values from response
                if let Some(new_payload) = verify_response.payload {
                    payload = new_payload;
                }
                if let Some(new_process_token) = verify_response.process_token {
                    process_token = new_process_token;
                }
                if let Some(new_lot_number) = verify_response.lot_number {
                    lot_number = new_lot_number;
                }

                // Generate new W parameter for retry
                // For continue responses, we typically don't need solver results
                current_w = generate_w_parameter(
                    &data,
                    &self.captcha_id,
                    self.risk_type,
                    &self.constants,
                    None, // No solver result needed for continue
                )?;

                continue;
            }

            // Failed - no seccode and not continue
            return Err(GeekedError::VerificationFailed {
                message: verify_response
                    .result
                    .unwrap_or_else(|| "Unknown verification error".into()),
            });
        }

        Err(GeekedError::VerificationFailed {
            message: format!("Max retries ({}) exceeded", MAX_RETRIES),
        })
    }

    /// Get the captcha ID.
    pub fn captcha_id(&self) -> &str {
        &self.captcha_id
    }

    /// Get the risk type.
    pub fn risk_type(&self) -> RiskType {
        self.risk_type
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_random_callback() {
        let cb1 = Geeked::random_callback();
        let cb2 = Geeked::random_callback();

        assert!(cb1.starts_with("geetest_"));
        assert!(cb2.starts_with("geetest_"));
        // Should be unique due to timestamp
        assert_ne!(cb1, cb2);
    }

    #[test]
    fn test_parse_jsonp() {
        let callback = "geetest_12345";
        let response = r#"geetest_12345({"status": "success", "data": {"lot_number": "abc123"}})"#;

        #[derive(serde::Deserialize)]
        struct TestData {
            lot_number: String,
        }

        let result: Result<TestData> = Geeked::parse_jsonp(response, callback);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().lot_number, "abc123");
    }
}
