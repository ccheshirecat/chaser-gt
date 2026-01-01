//! Test with shuffle.com captcha

use chaser_gt::{Geeked, RiskType};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing for debug output
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let captcha_id = "b3d286a8bdd3cc048538b57984f36d7f";

    // shuffle.com uses AI/invisible captcha
    println!("Testing with shuffle.com captcha ID: {}", captcha_id);
    println!("Risk type: ai (invisible)\n");

    // Create solver - shuffle.com uses AI captcha
    let solver = Geeked::builder(captcha_id, RiskType::Ai).build().await?;

    println!("Solver initialized successfully!");
    println!("Attempting to solve captcha...\n");

    // Solve the captcha
    match solver.solve().await {
        Ok(result) => {
            println!("=== SUCCESS ===");
            println!("captcha_id: {}", result.captcha_id);
            println!("lot_number: {}", result.lot_number);
            println!("pass_token: {}", result.pass_token);
            println!("gen_time: {}", result.gen_time);
            println!(
                "captcha_output: {}...",
                &result.captcha_output[..50.min(result.captcha_output.len())]
            );
        }
        Err(e) => {
            println!("=== FAILED ===");
            println!("Error: {:?}", e);
        }
    }

    Ok(())
}
