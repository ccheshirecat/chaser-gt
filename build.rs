//! Build script for chaser-gt
//!
//! Generates C header file when the `ffi` feature is enabled.

fn main() {
    // Only generate headers when ffi feature is enabled
    #[cfg(feature = "ffi")]
    {
        let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();

        // Create include directory if it doesn't exist
        let include_dir = std::path::Path::new(&crate_dir).join("include");
        std::fs::create_dir_all(&include_dir).ok();

        // Generate C header
        let config = cbindgen::Config::from_file("cbindgen.toml").unwrap_or_default();

        cbindgen::Builder::new()
            .with_crate(&crate_dir)
            .with_config(config)
            .generate()
            .map(|bindings| {
                bindings.write_to_file(include_dir.join("chaser_gt.h"));
            })
            .ok();
    }
}
