FROM rust:1.73.0

# Run rustup update so we pick up the toolchain version in rust-toolchain.toml
RUN rustup update
RUN cargo install ebg --version 0.3.0
