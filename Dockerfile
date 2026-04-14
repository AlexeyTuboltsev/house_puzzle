FROM rust:1.88-bookworm

# System deps: libclang for bindgen (mupdf-sys), C build tools for mupdf
RUN apt-get update && apt-get install -y libclang-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy manifests first for dependency caching
COPY Cargo.toml Cargo.toml
COPY crates/hp-core/Cargo.toml crates/hp-core/Cargo.toml
COPY crates/hp-server/Cargo.toml crates/hp-server/Cargo.toml

# Dummy src for dep caching
RUN mkdir -p crates/hp-core/src && echo "pub mod types;" > crates/hp-core/src/lib.rs && touch crates/hp-core/src/types.rs
RUN mkdir -p crates/hp-server/src && echo "fn main() {}" > crates/hp-server/src/main.rs

# Need templates/static for rust-embed at compile time
COPY templates/ templates/
COPY static/ static/

RUN cargo build --release 2>&1 || true

# Copy real source
COPY crates/ crates/
RUN cargo build --release

EXPOSE 5050
CMD ["target/release/hp-server"]
