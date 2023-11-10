FROM rustlang/rust:nightly
# FROM rust:1.73.0

RUN cargo install ebg --version 0.2.2
