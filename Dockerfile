# Stage 1: 構建階段
FROM rust:1.91-bookworm AS builder

# 安裝構建依賴
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libavahi-compat-libdnssd-dev \
    libasound2-dev \
    cmake \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# 複製源代碼
COPY . .

# 構建應用
RUN cargo build --release --bin aoede

# Stage 2: 運行時階段
FROM debian:bookworm-slim AS runtime

# 安裝運行時依賴
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libavahi-compat-libdnssd1 \
    libasound2 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# 複製二進制文件
COPY --from=builder /app/target/release/aoede /usr/local/bin/aoede

# 創建非 root 用戶
RUN useradd -r -u 1000 -m -d /data -s /bin/bash aoede && \
    chown -R aoede:aoede /app /data

USER aoede

ENV CACHE_DIR=/data

ENTRYPOINT ["/usr/local/bin/aoede"]