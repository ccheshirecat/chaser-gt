# chaser-gt

A high-performance Rust port of [GeekedTest](https://github.com/xKiian/GeekedTest) - a Geetest v4 captcha solver.

## Features

- **Automatic Deobfuscation**: Constants are automatically updated when Geetest releases new versions - no manual intervention required!
- **Multiple Captcha Types**: Supports Slide, Gobang, Icon, and AI/Invisible captchas
- **TLS Fingerprinting**: Uses `rquest` for Chrome-like TLS fingerprinting
- **Proxy Support**: HTTP and SOCKS5 proxy support with authentication
- **Async/Await**: Built on Tokio for efficient concurrent captcha solving
- **Cross-Platform**: Works on Windows, macOS, and Linux

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
chaser-gt = { git = "https://github.com/ccheshirecat/chaser-gt" }
tokio = { version = "1", features = ["full"] }
```

## Quick Start

Basic usage:

```rust
use chaser_gt::{Geeked, RiskType};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create a solver for slide captcha
    let solver = Geeked::builder("your_captcha_id", RiskType::Slide)
        .build()
        .await?;

    // Solve the captcha
    let result = solver.solve().await?;

    println!("captcha_id: {}", result.captcha_id);
    println!("lot_number: {}", result.lot_number);
    println!("pass_token: {}", result.pass_token);
    println!("gen_time: {}", result.gen_time);
    println!("captcha_output: {}", result.captcha_output);

    Ok(())
}
```

## With Proxy

```rust
let solver = Geeked::builder("captcha_id", RiskType::Slide)
    .proxy("http://user:pass@proxy.example.com:8080")  // HTTP proxy
    // or: .proxy("socks5://127.0.0.1:1080")           // SOCKS5 proxy
    .build()
    .await?;
```

## With User Info (Site-Specific Binding)

Some sites require a `user_info` parameter to bind captcha verification to a specific user/session:

```rust
let solver = Geeked::builder("captcha_id", RiskType::Ai)
    .user_info("account_id=12345")  // Site-specific data
    .proxy("http://proxy:8080")
    .build()
    .await?;
```

## With IPv6/Local Address Binding

For scenarios where you need to route captcha solving through a specific network interface or IPv6 address (e.g., BGP exit nodes):

```rust
use std::net::IpAddr;

let ipv6: IpAddr = "2a11:29c0:4f50::1".parse().unwrap();

let solver = Geeked::builder("captcha_id", RiskType::Ai)
    .local_address(ipv6)  // Bind to specific IPv6
    .build()
    .await?;
```

This is useful for:
- **BGP exit nodes**: Route each account through a unique /128 from your subnet
- **Multi-IP setups**: Distribute captcha solving across multiple IPs
- **IP consistency**: Ensure captcha and subsequent requests use the same IP

## Supported Captcha Types

| Type | Enum | Description |
|------|------|-------------|
| Slide | `RiskType::Slide` | Slide puzzle captcha |
| Gobang | `RiskType::Gobang` | Five-in-a-row puzzle |
| Icon | `RiskType::Icon` | Icon selection captcha (requires `icon` feature) |
| AI | `RiskType::Ai` | AI/Invisible captcha |

## Icon Captcha Support

To enable icon captcha support, add the `icon` feature:

```toml
[dependencies]
chaser-gt = { git = "https://github.com/ccheshirecat/chaser-gt", features = ["icon"] }
```

The icon solver uses:
- **ONNX Runtime** for neural network inference
- **Image processing** to detect icon regions
- A bundled **classification model** to identify arrow directions

The ONNX model (`geetest_v4_icon.onnx`) is embedded in the binary for easy distribution.

## Key Improvements

### Automatic Constant Updates

Unlike the Python version which requires manually running `deobfuscate.py` when Geetest updates, this Rust implementation:

1. **Checks for new versions** on startup
2. **Automatically deobfuscates** the Geetest script
3. **Caches constants** to `~/.cache/chaser-gt/constants.json`
4. **Refreshes automatically** when Geetest updates

This means the solver stays functional without any manual intervention!

### Multi-Round Verification Support

Some sites use multi-round verification where Geetest returns `result: "continue"` with updated payload. This library automatically handles the retry loop, making it compatible with sites like shuffle.com that require multiple verification rounds.

## Architecture

```
chaser-gt/
├── src/
│   ├── lib.rs           # Public API exports
│   ├── client.rs        # Main Geeked client
│   ├── deobfuscate.rs   # Auto-deobfuscation system
│   ├── sign.rs          # W parameter generation
│   ├── error.rs         # Error types
│   ├── models.rs        # Data structures
│   ├── crypto/
│   │   ├── aes_enc.rs   # AES-CBC encryption
│   │   ├── rsa_enc.rs   # RSA PKCS1v1.5
│   │   └── pow.rs       # Proof of Work
│   └── solvers/
│       ├── slide.rs     # Slide captcha solver
│       ├── gobang.rs    # Gobang puzzle solver
│       └── icon.rs      # Icon captcha solver
└── models/
    └── geetest_v4_icon.onnx  # ONNX model for icon detection
```

## Building

```bash
cd chaser-gt

# Without icon support
cargo build --release

# With icon support (includes ONNX runtime)
cargo build --release --features icon

# With C FFI bindings
cargo build --release --features ffi
```

## C FFI Bindings

chaser-gt provides C FFI bindings for use from Python, Go, Node.js, C/C++, etc.

### Building FFI Library

```bash
cargo build --release --features ffi

# Library outputs:
# - target/release/libchaser_gt.so (Linux)
# - target/release/libchaser_gt.dylib (macOS)
# - target/release/chaser_gt.dll (Windows)

# C header generated at:
# - include/chaser_gt.h
```

### C Example

```c
#include "chaser_gt.h"
#include <stdio.h>

int main() {
    // Solve a slide captcha (blocking call)
    GeekedResult result = geeked_solve(
        "your_captcha_id",  // captcha_id
        "slide",            // risk_type: slide, gobang, icon, ai
        NULL,               // proxy (optional): "http://user:pass@host:port"
        NULL                // user_info (optional)
    );

    if (result.error_code == 0) {
        printf("lot_number: %s\n", result.lot_number);
        printf("pass_token: %s\n", result.pass_token);
        printf("captcha_output: %s\n", result.captcha_output);
    } else {
        printf("Error: %s\n", result.error_message);
    }

    geeked_free_result(result);
    return 0;
}
```

Compile with:
```bash
gcc -o example example.c -L./target/release -lchaser_gt -lpthread -ldl -lm
```

### Python Example (ctypes)

```python
import ctypes
import json
from ctypes import c_char_p, c_int, Structure, POINTER

# Load library
lib = ctypes.CDLL('./target/release/libchaser_gt.so')  # or .dylib on macOS

# Define result structure
class GeekedResult(Structure):
    _fields_ = [
        ('error_code', c_int),
        ('error_message', c_char_p),
        ('captcha_id', c_char_p),
        ('lot_number', c_char_p),
        ('pass_token', c_char_p),
        ('gen_time', c_char_p),
        ('captcha_output', c_char_p),
    ]

# Or use the simpler JSON API
lib.geeked_solve_json.restype = c_char_p

result_json = lib.geeked_solve_json(
    b"your_captcha_id",
    b"slide",
    None,  # proxy
    None   # user_info
)

result = json.loads(result_json.decode())
lib.geeked_free_string(result_json)

if result['success']:
    print(f"pass_token: {result['pass_token']}")
else:
    print(f"Error: {result['error']}")
```

### FFI Functions

```c
// Solve captcha, returns struct with all fields
GeekedResult geeked_solve(
    const char* captcha_id,   // Required
    const char* risk_type,    // Required: "slide", "gobang", "icon", "ai"
    const char* proxy,        // Optional: "http://host:port" or "socks5://host:port"
    const char* user_info     // Optional: site-specific binding
);

// Solve captcha, returns JSON string
char* geeked_solve_json(
    const char* captcha_id,
    const char* risk_type,
    const char* proxy,
    const char* user_info
);

// Free result struct
void geeked_free_result(GeekedResult result);

// Free string
void geeked_free_string(char* s);

// Get library version
const char* geeked_version();
```

## Running Tests

```bash
# Test without icon feature
cargo test

# Test with icon feature
cargo test --features icon
```

## Running Example

```bash
cargo run --example solve_captcha
```

## Dependencies

Key dependencies:
- `rquest` - HTTP client with TLS fingerprinting
- `tokio` - Async runtime
- `aes`, `rsa`, `sha2` - Cryptography (RustCrypto)
- `image`, `imageproc` - Image processing for slide captcha
- `ort` - ONNX Runtime for icon captcha (optional)

## API Reference

### GeekedResult

The `solve()` method returns a `GeekedResult` containing all the fields needed for verification:

```rust
pub struct GeekedResult {
    pub captcha_id: String,      // The captcha ID used
    pub lot_number: String,      // Unique lot number for this solve
    pub pass_token: String,      // Token to submit to the site
    pub gen_time: String,        // Generation timestamp
    pub captcha_output: String,  // Encrypted captcha output
}
```

### Error Handling

```rust
use chaser_gt::{Geeked, RiskType, GeekedError};

match solver.solve().await {
    Ok(result) => println!("Token: {}", result.pass_token),
    Err(GeekedError::NetworkError(e)) => eprintln!("Network failed: {}", e),
    Err(GeekedError::CaptchaFailed(msg)) => eprintln!("Captcha failed: {}", msg),
    Err(GeekedError::DeobfuscationFailed) => eprintln!("Script deobfuscation failed"),
    Err(e) => eprintln!("Other error: {}", e),
}
```

## Requirements

- Rust 1.70+ (for async traits)
- Internet connection (fetches Geetest scripts for deobfuscation)
- Optional: Proxy for production use

## License

MIT License - same as the original GeekedTest project.

## Acknowledgements

- [GeekedTest](https://github.com/xKiian/GeekedTest) - Original Python implementation by xKiian
