FROM rust:latest as builder
RUN update-ca-certificates
RUN apt update && apt install -y libssl-dev  && rm -rf /var/lib/apt/lists/*
WORKDIR /usr/src/myapp
COPY . .
RUN cargo build --release

FROM debian:buster-slim
RUN apt update && apt install -y libssl1.1 ca-certificates  && rm -rf /var/lib/apt/lists/*
RUN mkdir /app/
COPY --from=builder /usr/src/myapp/target/release/rusty-notify /app/rusty-notify

WORKDIR /app
CMD ["./rusty-notify"]