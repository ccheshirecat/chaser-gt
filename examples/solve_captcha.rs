//! Example: Solving a Geetest v4 captcha.
//!
//! Run with: cargo run --example solve_captcha

use chaser_gt::{Geeked, RiskType};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing for debug output (optional)
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Demo captcha IDs from Geetest's demo page
    // https://gt4.geetest.com/demov4/index-en.html
    let demo_captchas = [
        ("54088bb07d2df3c46b79f80300b0abbe", RiskType::Slide),
        // ("xxx", RiskType::Gobang),  // Add gobang demo ID
        // ("xxx", RiskType::Icon),    // Add icon demo ID (requires 'icon' feature)
        // ("55c86e822ef5984cc0b03a3bbfd1a7c7", RiskType::Ai),
    ];

    for (captcha_id, risk_type) in demo_captchas {
        println!("\n=== Solving {} captcha ===", risk_type);

        // Create solver
        let solver = Geeked::builder(captcha_id, risk_type)
            // Optionally add proxy:
            // .proxy("http://127.0.0.1:8080")
            .build()
            .await?;

        // Solve the captcha
        match solver.solve().await {
            Ok(result) => {
                println!("Success!");
                println!("  captcha_id: {}", result.captcha_id);
                println!("  lot_number: {}", result.lot_number);
                println!("  pass_token: {}", result.pass_token);
                println!("  gen_time: {}", result.gen_time);
                println!(
                    "  captcha_output: {}...",
                    &result.captcha_output[..50.min(result.captcha_output.len())]
                );
            }
            Err(e) => {
                println!("Failed: {}", e);
            }
        }
    }

    Ok(())
}
