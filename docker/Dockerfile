# 多阶段构建：rust 编译 + node 前端构建 → 精简运行时。重构建在 CI（开发机 2H2G 不做 release）。
FROM rust:slim-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release -p godoword

# 前端构建（npm workspaces：web/core + web/shell → Svelte SPA dist）。
FROM node:22-slim AS webbuilder
WORKDIR /web
COPY package.json package-lock.json tsconfig.base.json ./
COPY web/core/package.json web/core/package.json
COPY web/shell/package.json web/shell/package.json
RUN npm ci --no-audit --no-fund
COPY web/core web/core
COPY web/shell web/shell
RUN npm run build -w @godoword/shell

FROM debian:bookworm-slim
RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates \
  && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/godoword /usr/local/bin/godoword
# 前端构建产物伺服目录（axum ServeDir web/shell/public；hash 路由深链稳健）。
COPY --from=webbuilder /web/web/shell/dist ./web/shell/public
RUN mkdir -p /data
EXPOSE 8080
LABEL org.opencontainers.image.source="https://github.com/hinnyuu/godoword" \
      org.opencontainers.image.licenses="AGPL-3.0-only" \
      org.opencontainers.image.title="godoword"
# 自适应降权（M1b 已落地）：rootless 保持 uid 0 映射宿主调用者；rootful 以 /data 属主运行。
ENTRYPOINT ["godoword"]