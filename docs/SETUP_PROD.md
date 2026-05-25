# Setup SeaweedFS operator for production

## Pre-requisites
You will need:

* `kubectl` access to the target cluster
* A running SeaweedFS instance, and especially:
    * The URL of the filer gRPC


## Installation
The operator can be installed using the following commands:

```bash
kubectl apply -f https://raw.githubusercontent.com/pierre42100/SeaweedFSOperator/master/manifests/crd.yaml
kubectl apply -f https://raw.githubusercontent.com/pierre42100/SeaweedFSOperator/master/manifests/prod-deployment.yaml
```

> **Note**: Please be aware that the operator has access to all the secrets of the cluster!

## Configure instance
In order to create buckets, the operator needs to know how to reach the SeaweedFS instance.

You can declare a SeaweedFS instance in this way:

```yaml
apiVersion: "communiquons.org/v1"
kind: SeaweedFSInstance
metadata:
  name: my-instance
spec:
  filergrpc: http://instance-path:8889
```

> **Notes**: 
> * Please be aware that SeaweedFS instance are cluster-wide!
> * SeaweedFS can be located outside of the Kubernetes cluster, but this has security implications: the filer gRCP is NOT authenticated!

## Create a bucket
You are now ready to create your first bucket!

Here is a basic bucket example:

```yaml
apiVersion: "communiquons.org/v1"
kind: SeaweedFSBucket
metadata:
  name: first-bucket
  namespace: default
spec:
  # The name of the seaweedfs instance
  instance: my-instance
  # The name of the bucket to create
  name: first-bucket
  # The name of the secret that will be created
  # by the operator which contains credentials to 
  # use to access the bucket
  secret: first-bucket-secret
```

## More complete example
Here is a more complete example that makes use of all the available options:

```yaml
apiVersion: "communiquons.org/v1"
kind: SeaweedFSBucket
metadata:
  name: my-bucket
  namespace: default
spec:
  instance: my-instance
  name: my-bucket
  secret: my-bucket-secret
  # This must be set to true to allow unauthenticated
  # access to the bucket resources. Use this to host a
  # static website for example
  anonymous_read_access: true
  # Enable versioning on the bucket => keep old versions
  # of uploaded files
  versioning: true
  # If specified, a quota will be applied to the bucket, in bytes
  quota: 1000000000
  # Prevent files from being removed from the bucket. This parameter
  # can not be changed, once the bucket has been created
  lock: true
```
