# Contributing to chaser-gt

Thank you for your interest in contributing to chaser-gt! This document provides guidelines and instructions for contributing.

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/YOUR_USERNAME/chaser-gt.git`
3. Create a new branch: `git checkout -b feature/your-feature-name`

## Development Setup

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build the project
cargo build

# Run tests
cargo test

# Build with icon feature (requires ONNX runtime)
cargo build --features icon
```

## Code Style

- Run `cargo fmt` before committing
- Run `cargo clippy` and fix any warnings
- Write tests for new functionality
- Keep commits focused and atomic

## Pull Request Process

1. Ensure all tests pass: `cargo test`
2. Update documentation if needed
3. Add a clear description of your changes
4. Reference any related issues

## Reporting Issues

When reporting issues, please include:
- Rust version (`rustc --version`)
- Operating system
- Steps to reproduce
- Expected vs actual behavior

## License

By contributing, you agree that your contributions will be licensed under the MIT license.
