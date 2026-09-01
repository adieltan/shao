# Multi-stage ultra-lightweight build
FROM rust:1.80-alpine AS builder

RUN apk add --no-cache musl-dev sqlite-dev build-base

WORKDIR /usr/src/shao
COPY . .

RUN cargo build --release

# Final runtime image (< 15 MB)
FROM alpine:3.20

RUN apk add --no-cache ca-certificates tzdata

WORKDIR /app
COPY --from=builder /usr/src/shao/target/release/shao /app/shao
COPY config.toml.example /app/config.toml

EXPOSE 8080

ENTRYPOINT ["/app/shao"]
CMD ["--config", "/app/config.toml"]
