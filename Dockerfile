# We use the cargo-chef image for the builder stages to avoid manual installation
FROM lukemathwalker/cargo-chef:latest-rust-1.90 AS chef
WORKDIR /app

# Copy the project to generate a recipe
COPY . .

# The project has a build.rs file which requires protobuf for code generation.
# We must install it for the recipe generation to succeed.
RUN apt-get update && apt-get install -y protobuf-compiler
RUN cargo chef prepare --recipe-path recipe.json

# --- Builder Stage ---
FROM lukemathwalker/cargo-chef:latest-rust-1.90 AS builder
WORKDIR /app

# Install build dependencies like protobuf-compiler
RUN apt-get update && apt-get install -y protobuf-compiler

# Copy the recipe from the chef stage and cook the dependencies.
# This layer will be cached very effectively between builds.
COPY --from=chef /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Now, copy the application source code and build it.
# This will leverage the pre-built dependencies from the previous step.
COPY . .
RUN cargo build --release

# --- Runtime Stage ---
FROM debian:buster-slim AS runtime
WORKDIR /app

# Copy the built binary from the builder stage
COPY --from=builder /app/target/release/mycqu_service /usr/local/bin/mycqu_service

EXPOSE 53211
CMD ["/usr/local/bin/mycqu_service"]
