FROM rust:1.70.0

RUN rustup install nightly-2023-06-20
RUN cargo +nightly-2023-06-20 install ebg --version 0.2.0
