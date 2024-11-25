FROM mcr.microsoft.com/devcontainers/rust:1-1-bookworm

USER vscode

RUN rustup toolchain install nightly -c clippy -c rustfmt -c rust-src
RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
RUN cargo binstall --no-confirm --locked cargo-nextest cargo-insta cargo-chef

USER root
