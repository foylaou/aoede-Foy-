FROM rust:1.91-alpine AS builder

# 安裝所有構建依賴
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
    linux-headers \
    avahi-dev

WORKDIR /app

# 設置環境變數
ENV OPENSSL_STATIC=1 \
    OPENSSL_LIB_DIR=/usr/lib \
    OPENSSL_INCLUDE_DIR=/usr/include

# 直接複製所有源代碼並構建
COPY . .
RUN cargo build --release --bin aoede

FROM alpine:3.21 AS runtime

# 安裝運行時依賴
RUN apk add --no-cache \
    libgcc \
    ca-certificates \
    avahi-compat-libdns_sd

WORKDIR /app

# 複製二進制文件
COPY --from=builder /app/target/release/aoede /usr/local/bin/aoede

# 創建用戶
RUN mkdir -p /data && \
    addgroup -g 1000 aoede && \
    adduser -D -u 1000 -G aoede aoede && \
    chown -R aoede:aoede /app /data

USER aoede

ENV CACHE_DIR=/data

ENTRYPOINT ["/usr/local/bin/aoede"]