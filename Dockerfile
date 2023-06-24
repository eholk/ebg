FROM rust:1.70.0

RUN rustup update
RUN cargo install ebg --version 0.2.0
