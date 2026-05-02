# syntax=docker/dockerfile:1
#
# Build a Linux x86_64 `nvoc` binary without installing Rust locally.
#
# Usage:
#   mkdir -p dist
#   docker buildx build --platform linux/amd64 --target artifact --output type=local,dest=dist .
#
# Output:
#   dist/nvoc

# Helper target: generate an updated Cargo.lock (useful when adding deps without Rust installed)
FROM --platform=$BUILDPLATFORM rust:1.85-bookworm AS lockgen
WORKDIR /work
COPY Cargo.toml ./
RUN cargo generate-lockfile

FROM scratch AS lockfile
COPY --from=lockgen /work/Cargo.lock /Cargo.lock

FROM --platform=$BUILDPLATFORM rust:1.85-bookworm AS builder

WORKDIR /work

# Cache deps first.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && printf 'fn main() {}\n' > src/main.rs
RUN cargo build --release --locked && rm -rf src

# Build real binary.
COPY src ./src
# `docker build` preserves file mtimes from the build context; when those mtimes
# are older than the previously-built placeholder crate, Cargo may incorrectly
# treat the target as up-to-date and skip rebuilding the actual binary.
RUN find src -type f -exec touch {} +
RUN cargo build --release --locked
RUN strip target/release/nvoc && cp target/release/nvoc /nvoc

# Artifact stage (no rootfs) for clean local export via buildx `--output`.
FROM --platform=$TARGETPLATFORM scratch AS artifact
COPY --from=builder /nvoc /nvoc
