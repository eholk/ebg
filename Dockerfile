# This Dockerfile is configured via build args during the Docker build process.
# The actual versions are determined by the docker-release.yml workflow.
#
# To build manually with specific versions:
#   docker build --build-arg RUST_VERSION=1.89.0 --build-arg EBG_VERSION=0.6.1 .

ARG RUST_VERSION=1.89.0
FROM rust:${RUST_VERSION}

ARG EBG_VERSION=latest

RUN if [ "$EBG_VERSION" = "latest" ]; then \
    cargo install ebg; \
    else \
    cargo install ebg --version $EBG_VERSION; \
    fi
