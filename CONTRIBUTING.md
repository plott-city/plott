# Contributing to stalkr

Thank you for your interest in contributing.

## Development Setup

1. Install Rust (latest stable) and Node.js (v18+)
2. Clone the repository
3. Run `cargo build` to verify the workspace compiles
4. Run `cd sdk && npm install && npm test` for SDK tests

## Code Style

- Rust: Follow `rustfmt.toml` settings, run `cargo fmt` before committing
- TypeScript: Use the project tsconfig settings
- Write clear, descriptive commit messages

## Pull Requests

- Fork the repository and create a feature branch
- Write tests for new functionality
- Ensure all existing tests pass
- Submit a pull request with a clear description
<!-- set workspace resolver to v2 [1.3] -->
