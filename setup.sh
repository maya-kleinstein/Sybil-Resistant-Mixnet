#!/bin/bash

# Setting my code directory on local machine
LOCAL_CODE_DIR='C:\\university\\Thesis\\bbs'
LOCAL_ELF_DIR='C:\\university\\Thesis\\bbs\\target\\x86_64-unknown-linux-gnu\\'
CONTAINER_ELF_DIR = 'code/target/x86_64-unknown-linux-gnu/'


##### STEP 1: Update Project (git) + Compile the Rust code using Docker #####
docker run --rm -v $LOCAL_CODE_DIR:/code rust_compiler bash -c "cargo build --target x86_64-unknown-linux-gnu && tail -f /dev/null"

# If compilation fails, exit
if [ $? -ne 0 ]; then
    echo "Compilation failed!"
	read -p "Press any key to continue . . ."
    exit 1
fi

# Notify user
echo "Code compiled successfully!"


##### STEP 2: Extract compiled rust code to output dir #####

# Get the first running container ID
CONTAINER_ID=$(docker ps --format "{{.ID}}" | head -n 1)

# Define your local destination directory
LOCAL_ELF_DIR="./my_local_directory"  # Replace with your actual path

# Copy the file from the container to the local directory
docker cp ${CONTAINER_ID}:$CONTAINER_ELF_DIR $LOCAL_ELF_DIR


##### STEP 3: Cleanup. Shut down container. #####
docker stop ${CONTAINER_ID}

read -p "Press any key to continue . . ."


