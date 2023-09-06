FROM rust:1.67

COPY . /code

# Update, Upgrade, Install necessary libraries and tools for cross-compilation, and clean up
RUN apt-get update && \
    apt upgrade -y && \
    apt-get install -y gcc-multilib g++-multilib unzip curl && \
    rustup target add x86_64-unknown-linux-gnu && \
    rm -rf /var/lib/apt/lists/*

# Install protoc
RUN PROTOC_ZIP=protoc-3.3.0-linux-x86_64.zip && \
    curl -OL https://github.com/google/protobuf/releases/download/v3.3.0/$PROTOC_ZIP && \
    unzip -o $PROTOC_ZIP -d /usr/local bin/protoc && \
    rm -f $PROTOC_ZIP


WORKDIR /code

CMD ["cargo", "build", "--target", "x86_64-unknown-linux-gnu"]