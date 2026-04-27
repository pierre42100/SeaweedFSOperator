# Setup SeaweedFS for operator development

## Install Rust
As this project has been written using Rust, you will need to install it prior working on it. Please follow the official instructions: [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install)

## Install Minikube
You will also need to install Minikube on your computer to have a test K8S environment. In order to do this, please follow the official instructions: [https://minikube.sigs.k8s.io/docs/start](https://minikube.sigs.k8s.io/docs/start)

## Start Minikube
You will then need to start Minikube using the following command:

```bash
# Gain docker rights if you don't have them already
sudo -g docker bash

# Start Minikube
 minikube start -d docker
```

You can then make sure that Minikube is working properly:

```
minikube kubectl get nodes
```

You should get a response similar to this one:

```
NAME       STATUS   ROLES           AGE     VERSION
minikube   Ready    control-plane   2m16s   v1.32.0
```


## Clone repository
Clone this repository using:

```bash
git clone https://gitea.communiquons.org/pierre/seaweedfs-k8s-operator
cd seaweedfs-k8s-operator
```

> \[!NOTE]
> If you want to get a Gitea account to make a pull request on this repository, you will need to contact me at: `pierre.git@communiquons.org`

## Expose services locally
Run the following command to expose SeaweedFS services locally:

```bash
minikube tunnel --bind-address '127.0.0.1' 
```

## Deploy Seaweedfs
You will need then to deploy seaweedfs:

```bash
minikube kubectl -- apply -f manifests/seaweedfs-dev-deployment.yaml
```

Wait for the SeaweedFS pod to become ready:

```bash
minikube kubectl -- get pods -w
```

Check for the availability of the service that expose SeaweedFS to your host computer:

```bash
minikube kubectl -- get services
```

You should get a result similar to this one:

```
NAME         TYPE           CLUSTER-IP      EXTERNAL-IP   PORT(S)                                                                                      AGE
kubernetes   ClusterIP      10.96.0.1       <none>        443/TCP                                                                                      118m
seaweedfs    LoadBalancer   10.111.195.29   127.0.0.1     9333:30245/TCP,9340:32694/TCP,8888:32054/TCP,8333:32194/TCP,7333:31460/TCP,23646:30649/TCP   80m
```

> If the IP for `seaweedfs` is reported as `<pending>`, make sure the tunnel is properly started!

If `EXTERNAL-IP` is set to `127.0.0.1` then you are ready to go!

* Master UI: http://localhost:9333
* Volume Server: http://localhost:9340
* Filer UI: http://localhost:8888
* S3 Endpoint: http://localhost:8333
* WebDAV: http://localhost:7333
* Admin UI: http://localhost:23646
  * Username: `user`
  * Password: `admin`


## Deploy CRD
You will need then to deploy the Custom Resource Definitions of SeaweedfsK8SOperator using the following command:

```bash
minikube kubectl -- apply -f manifests/crd.yaml
```

## Run operator
You can then run the project using the following command:

```bash
cargo fmt && cargo clippy && RUST_LOG=debug cargo run --
```
