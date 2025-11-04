FROM rust:1.91-alpine AS dependencies

# 安裝基礎構建依賴
RUN apk add --no-cache \
    alpine-sdk \
    cmake \
    automake \
    autoconf \
    libtool \
    musl-dev \
    pkgconfig \
    openssl-dev \
    openssl-libs-static \
    perl \
    linux-headers

RUN cargo install cargo-chef

FROM dependencies AS planner
WORKDIR /app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM dependencies AS cacher
WORKDIR /app

# 設置環境變數讓 OpenSSL 靜態連結
ENV OPENSSL_STATIC=1 \
    OPENSSL_LIB_DIR=/usr/lib \
    OPENSSL_INCLUDE_DIR=/usr/include

COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

FROM dependencies AS builder
WORKDIR /app

# 設置環境變數
ENV OPENSSL_STATIC=1 \
    OPENSSL_LIB_DIR=/usr/lib \
    OPENSSL_INCLUDE_DIR=/usr/include

# 複製緩存的依賴
COPY --from=cacher /app/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo

# 複製源代碼並構建
COPY . .
RUN cargo build --release --bin aoede

FROM alpine:3.21 AS runtime

# 安裝運行時依賴
RUN apk add --no-cache \
    libgcc \
    ca-certificates

WORKDIR /app

# 複製二進制文件
COPY --from=builder /app/target/release/aoede /usr/local/bin/aoede

# 創建數據目錄
RUN mkdir -p /data && \
    addgroup -g 1000 aoede && \
    adduser -D -u 1000 -G aoede aoede && \
    chown -R aoede:aoede /app /data

# 切換到非 root 用戶
USER aoede

ENV CACHE_DIR=/data

ENTRYPOINT ["/usr/local/bin/aoede"]
