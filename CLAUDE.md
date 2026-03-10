# Proxy Centauri

HTTP CONNECT proxy server with authentication and rate limiting.

## Tech
- Rust (edition 2024), Tokio, httparse, anyhow, tracing

## Structure
```
proxima_centauri/lib/
├── server.rs      # Accept loop
├── handler.rs     # Request handling, auth, limits
├── tunnel.rs      # Bidirectional TCP streaming
├── auth.rs        # User database
├── registry.rs    # User registry with limits & stats
├── context.rs     # Global application context
├── config.rs      # Configuration
├── http_utils/    # ProxyResponse enum
└── tests/         # Integration tests
```

## Rules

1. **Wait for confirmation before making changes**
2. **DO NOT add comments** — code should be self-documenting
3. **Minimal changes** — solve the task, nothing more
4. **Always run tests** after changes: `cargo test --lib`
5. **Use async/await** — never block, use `tokio::` types
6. **Error handling** — `anyhow::Result`, `bail!`, no `unwrap()` on user data

## Commands
```bash
cargo check          # Compile check
cargo test --lib     # Run tests (16 tests)
cargo clippy         # Lint (0 warnings)
cargo run            # Run server
```

## CI
GitHub Actions runs on PR:
- Tests job: `cargo test --lib`
- Clippy job: `cargo clippy -- -D warnings`

## Auth
Users: `procent:o953zY7lnkYMEl5D`, `admin:12345`
