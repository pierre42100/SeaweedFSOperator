FROM debian:trixie-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
  libssl3 \
  && rm -rf /var/lib/apt/lists/*

COPY seaweedfs-k8s-operator /usr/local/bin/seaweedfs-k8s-operator

ENTRYPOINT ["/usr/local/bin/seaweedfs-k8s-operator"]
