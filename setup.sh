#!/bin/bash

# Setting my code directory on local machine
LOCAL_CODE_DIR='C:\\university\\Thesis\\bbs'
LOCAL_ELF_DIR='C:\\university\\Thesis\\toolchain\\elfs'

# Step 1: Compile the Rust code using Docker
docker run --rm -v $LOCAL_CODE_DIR:/code rust_compiler bash -c "cd /code && cargo build --release"

# If compilation fails, exit
if [ $? -ne 0 ]; then
    echo "Compilation failed!"
	read -p "Press any key to continue . . ."
    exit 1
fi

# Notify user
echo "Code compiled successfully!"

# Step 2: Extract compiled rust code to output dir

docker cp rust_compiler:/code/target/release $LOCAL_ELF_DIR

read -p "Press any key to continue . . ."


