# Stage 1: Builder
# Full Rust toolchain on CentOS Stream 10 for compilation and testing.
FROM quay.io/centos/centos:stream10 AS builder

# Install build dependencies
RUN dnf install -y \
        gcc \
        make \
        pkgconf-pkg-config \
        && dnf clean all

# Install Rust toolchain via rustup (SEC-009: HTTPS from canonical source)
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --default-toolchain none
ENV PATH="/root/.cargo/bin:${PATH}"

# Copy rust-toolchain.toml first to install the pinned version (SEC-009)
COPY rust-toolchain.toml /build/
WORKDIR /build
RUN rustup show

# Copy manifests for dependency caching
COPY Cargo.toml Cargo.lock* /build/

# Create a dummy main.rs to pre-build dependencies
RUN mkdir -p src && echo 'fn main() {}' > src/main.rs
RUN cargo build --release 2>/dev/null || true && \
    rm -f target/release/tuix target/release/deps/tuix-* && \
    rm -rf src

# Copy actual source
COPY src/ /build/src/
COPY tests/ /build/tests/

# Build release binary
RUN cargo build --release

# Run tests
RUN cargo test --release

# -------------------------------------------------------------------
# Stage 2: Export
# Minimal CentOS image containing only the compiled binary.
# Provides glibc and core shared libraries for dynamic linking.
FROM quay.io/centos/centos:stream10-minimal AS export

COPY --from=builder /build/target/release/tuix /usr/local/bin/tuix

# Default entrypoint copies the binary to /out (volume mount point)
CMD ["cp", "/usr/local/bin/tuix", "/out/tuix"]
