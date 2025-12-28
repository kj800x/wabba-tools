# Build Stage
FROM rust:1.91-alpine AS builder
WORKDIR /usr/src/
# Install required build dependencies
RUN apk add --no-cache musl-dev pkgconfig openssl-dev openssl-libs-static gcc g++ make

# - Install dependencies
WORKDIR /usr/src
COPY Cargo.toml Cargo.lock ./
RUN USER=root cargo new --name wabba-server wabba-server
RUN USER=root cargo new --name wabba-protocol wabba-protocol
RUN USER=root cargo new --name wabba-tools wabba-tools
COPY wabba-protocol/Cargo.toml ./wabba-protocol/
COPY wabba-server/Cargo.toml ./wabba-server/
COPY wabba-tools/Cargo.toml ./wabba-tools/
WORKDIR /usr/src
RUN cargo build --release

# - Copy sources
COPY wabba-tools/src ./wabba-tools/src
COPY wabba-protocol/src ./wabba-protocol/src
COPY wabba-server/src ./wabba-server/src
WORKDIR /usr/src
RUN touch wabba-server/src/main.rs && cargo build --release

# ---- Runtime Stage ----
FROM alpine:latest AS runtime
COPY --from=builder /usr/src/target/release/wabba-server /usr/local/bin/wabba-server
USER 1000
EXPOSE 8080
CMD ["wabba-server"]
