# syntax=docker/dockerfile:1.7

# We use the cargo-chef image for the builder stages to avoid manual installation

# --- Base Dependencies Stage ---
# This stage installs common dependencies to be reused by other stages,
# and configures mirrors to speed up package downloads.
FROM docker.1ms.run/lukemathwalker/cargo-chef:latest-rust-1.95 AS base-deps

# Configure apt to use Tsinghua mirror for Debian Buster
RUN echo "deb https://mirrors.tuna.tsinghua.edu.cn/debian/ trixie main contrib non-free non-free-firmware" > /etc/apt/sources.list && \
    echo "deb https://mirrors.tuna.tsinghua.edu.cn/debian/ trixie-updates main contrib non-free non-free-firmware" >> /etc/apt/sources.list && \
    echo "deb https://mirrors.tuna.tsinghua.edu.cn/debian/ trixie-backports main contrib non-free non-free-firmware" >> /etc/apt/sources.list && \
    echo "deb https://security.debian.org/debian-security trixie-security main contrib non-free non-free-firmware" >> /etc/apt/sources.list

# Install build dependencies like protobuf-compiler
RUN apt-get update && apt-get install -y protobuf-compiler

# Configure cargo to use Tsinghua mirror
RUN mkdir -p "${CARGO_HOME}" && \
    echo '[source.crates-io]' > "${CARGO_HOME}/config.toml" && \
    echo 'replace-with = "tuna"' >> "${CARGO_HOME}/config.toml" && \
    echo '' >> "${CARGO_HOME}/config.toml" && \
    echo '[source.tuna]' >> "${CARGO_HOME}/config.toml" && \
    echo 'registry = "sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/"' >> "${CARGO_HOME}/config.toml"

WORKDIR /app

# --- Chef Stage ---
# Use the base-deps stage with pre-installed dependencies and mirrors
FROM base-deps AS chef

# Copy the project to generate a recipe
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# --- Builder Stage ---
# Use the base-deps stage with pre-installed dependencies and mirrors
FROM base-deps AS builder

# Copy the recipe from the chef stage and cook the dependencies.
COPY --from=chef /app/recipe.json recipe.json
RUN --mount=type=cache,id=mycqu-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=mycqu-cargo-git,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,id=mycqu-target,target=/app/target,sharing=locked \
    cargo chef cook --release --recipe-path recipe.json

# Now, copy the application source code and build it.
COPY . .
RUN --mount=type=cache,id=mycqu-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=mycqu-cargo-git,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,id=mycqu-target,target=/app/target,sharing=locked \
    cargo build --release && \
    cp /app/target/release/mycqu_service /usr/local/bin/mycqu_service

# --- Runtime Stage ---
FROM debian:trixie-slim AS runtime

# Install SSL root certificates required for making HTTPS requests
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the built binary from the builder stage
COPY --from=builder /usr/local/bin/mycqu_service /usr/local/bin/mycqu_service

EXPOSE 53211 9321
CMD ["/usr/local/bin/mycqu_service"]
