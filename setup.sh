#!/bin/ash

# Setting my code directory on local machine
LOCAL_CODE_DIR='C:\\Thesis\\Sybil-Resistant-Mixnet'
LOCAL_ELF_DIR='C:\\Thesis\\Sybil-Resistant-Mixnet\\target\\x86_64-unknown-linux-gnu\\'
CONTAINER_NAME="compiler_container"
IMAGE_NAME="compiler_image"

##### STEP 1: Update Project (local) + Compile the Rust code using Docker #####

# Start the container
# docker run -d --name $CONTAINER_NAME -v $LOCAL_CODE_DIR:/code $IMAGE_NAME bash -c "while true; do sleep 10; done"
docker start $CONTAINER_NAME

# Execute the cargo build within the running container
docker exec $CONTAINER_NAME bash -c "cargo build --release --target x86_64-unknown-linux-gnu"

# Check the status of the cargo build
if [ $? -ne 0 ]; then
    echo "Compilation failed!"
    docker stop $CONTAINER_NAME
    read -p "Press any key to continue . . ."
    exit 1
fi

echo "Code compiled successfully!"

##### STEP 2: Extract compiled rust code to output dir #####

# Copy the file from the container to the local directory
# docker cp $CONTAINER_NAME:/code/target/x86_64-unknown-linux-gnu $LOCAL_ELF_DIR

echo "Results copied successfully!"

##### STEP 3: Cleanup. Shut down container. #####
docker stop $CONTAINER_NAME
# docker rm $CONTAINER_NAME
read -p "Press any key to continue . . ."
