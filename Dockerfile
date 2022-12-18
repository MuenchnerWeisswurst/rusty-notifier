FROM lukemathwalker/cargo-chef:latest-rust-1.59.0 AS chef
RUN update-ca-certificates
RUN apt update && apt install -y libssl-dev  && rm -rf /var/lib/apt/lists/*

WORKDIR /app/

FROM chef AS planner
COPY ./src ./src
COPY ./Cargo.toml ./Cargo.toml

RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder 
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY ./src ./src
COPY ./Cargo.toml ./Cargo.toml
RUN cargo build --release

FROM gcr.io/distroless/cc
WORKDIR /app
COPY --from=builder /app/target/release/rusty-notify /app/rusty-notify
CMD ["./rusty-notify"]