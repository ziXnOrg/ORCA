# syntax=docker/dockerfile:1

FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release -p orchestrator

FROM gcr.io/distroless/cc-debian12
WORKDIR /app
COPY --from=builder /app/target/release/orchestrator /usr/local/bin/orchestrator
ENV RUST_LOG=info
EXPOSE 50051
ENTRYPOINT ["/usr/local/bin/orchestrator"]
