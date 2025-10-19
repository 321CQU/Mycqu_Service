# We use the cargo-chef image for the builder stages to avoid manual installation

# --- Base Dependencies Stage ---
# This stage installs common dependencies to be reused by other stages,
# and configures mirrors to speed up package downloads.
FROM lukemathwalker/cargo-chef:latest-rust-1.90 AS base-deps

# Configure apt to use Tsinghua mirror for Debian Buster
RUN echo "deb https://mirrors.tuna.tsinghua.edu.cn/debian/ trixie main contrib non-free non-free-firmware" > /etc/apt/sources.list && \
    echo "deb https://mirrors.tuna.tsinghua.edu.cn/debian/ trixie-updates main contrib non-free non-free-firmware" >> /etc/apt/sources.list && \
    echo "deb https://mirrors.tuna.tsinghua.edu.cn/debian/ trixie-backports main contrib non-free non-free-firmware" >> /etc/apt/sources.list && \
    echo "deb https://security.debian.org/debian-security trixie-security main contrib non-free non-free-firmware" >> /etc/apt/sources.list

# Install build dependencies like protobuf-compiler
RUN apt-get update && apt-get install -y protobuf-compiler

# Configure cargo to use Tsinghua mirror
RUN mkdir -p /root/.cargo && \
    echo '[source.crates-io]' > /root/.cargo/config.toml && \
    echo 'replace-with = "tuna"' >> /root/.cargo/config.toml && \
    echo '' >> /root/.cargo/config.toml && \
    echo '[source.tuna]' >> /root/.cargo/config.toml && \
    echo 'registry = "https://mirrors.tuna.tsinghua.edu.cn/git/crates.io-index.git"' >> /root/.cargo/config.toml

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
RUN cargo chef cook --release --recipe-path recipe.json

# Now, copy the application source code and build it.
COPY . .
RUN cargo build --release

# --- Runtime Stage ---
FROM debian:trixie-slim AS runtime
WORKDIR /app

# Copy the built binary from the builder stage
COPY --from=builder /app/target/release/mycqu_service /usr/local/bin/mycqu_service

EXPOSE 53211
CMD ["/usr/local/bin/mycqu_service"]