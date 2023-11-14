FROM rust:1.73.0

# Run rustup update so we pick up the toolchain version in rust-toolchain.toml
RUN rustup toolchain install nightly-2023-11-12
RUN cargo +nightly-2023-11-12 install ebg --version 0.3.0
