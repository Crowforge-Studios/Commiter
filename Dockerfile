FROM rust:alpine AS builder

RUN apk add --no-cache musl-dev openssl-dev pkgconfig

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/
COPY .cargo/ ./.cargo/

RUN cargo build --target x86_64-unknown-linux-musl --release

FROM scratch
COPY --from=builder /build/target/x86_64-unknown-linux-musl/release/commiter /commiter
