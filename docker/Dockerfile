# 多阶段构建：rust 编译 → 精简运行时。重构建在 CI（开发机 2H2G 不做 release）。
FROM rust:slim-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release -p godoword

FROM debian:bookworm-slim
RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates \
  && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/godoword /usr/local/bin/godoword
COPY web/shell/public ./web/shell/public
RUN mkdir -p /data
EXPOSE 8080
LABEL org.opencontainers.image.source="https://github.com/hinnyuu/godoword" \
      org.opencontainers.image.licenses="AGPL-3.0-only" \
      org.opencontainers.image.title="godoword"
# M0 以容器内 root 运行（rootless 下映射为宿主普通用户）；自适应降权 M1 落地。
ENTRYPOINT ["godoword"]