#!/bin/bash
cargo build --release

TEMP_DIR=$(mktemp -d)
cp target/release/seaweedfs-k8s-operator "$TEMP_DIR"

docker build -f Dockerfile "$TEMP_DIR" -t pierre42100/seaweedfs_k8s_operator

rm -r $TEMP_DIR

