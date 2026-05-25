# Seaweedfs K8s operator

Automatically create SeaweedFS buckets based on K8S Custom Resources.

One deployed, this tool will allow you to automatically create SeaweedFS accounts associated with buckets, simply by creating Kubernetes resources.


* [Setup for development](./docs/SETUP_DEV.md)
* [Setup for production](./docs/SETUP_PROD.md)

Run testsuite:

```bash
cargo test -- --test-threads 1
```