FROM rust:1.79.0

# Run rustup update so we pick up the toolchain version in rust-toolchain.toml
RUN cargo install ebg --version 0.5.0
