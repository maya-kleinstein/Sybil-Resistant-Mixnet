FROM rust:latest

COPY . .

RUN apt update && apt upgrade -y
RUN apt install -y g++-aarch64-linux-gnu libc6-dev-arm64-cross

RUN rustup target add aarch64-unknown-linux-gnu
RUN rustup toolchain install stable-aarch64-unknown-linux-gnu

# Install protoc
RUN apt-get update && apt-get install -y unzip \
    && PROTOC_ZIP=protoc-3.3.0-linux-x86_64.zip \
    && curl -OL https://github.com/google/protobuf/releases/download/v3.3.0/$PROTOC_ZIP \
    && unzip -o $PROTOC_ZIP -d /usr/local bin/protoc \
    && rm -f $PROTOC_ZIP

WORKDIR /app

ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
    CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc \
    CXX_aarch64_unknown_linux_gnu=aarch64-linux-gnu-g++


CMD ["cargo", "build", "--target", "aarch64-unknown-linux-gnu"]