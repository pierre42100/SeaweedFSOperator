use futures::TryStreamExt;
use kube::Api;
use kube::runtime::{WatchStreamExt, watcher};
use seaweedfs_k8s_operator::crd::SeaweedFSBucket;
use seaweedfs_k8s_operator::k8s_operations;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    tracing::info!("starting operator");

    let client = kube::Client::try_default()
        .await
        .expect("Kubernetes client could not be initialized");

    let buckets: Api<SeaweedFSBucket> = Api::all(client.clone());

    // Listen for events => buckets creation or update (deletion is not supported)
    let wc = watcher::Config::default();
    let bw = watcher(buckets, wc).applied_objects();
    futures::pin_mut!(bw);

    while let Some(b) = bw
        .try_next()
        .await
        .expect("unable to follow buckets stream!")
    {
        if let Err(e) = k8s_operations::apply_bucket(&b, &client).await {
            tracing::error!(
                "Failed to apply desired configuration for applied bucket {} : {}",
                b.spec.name,
                e
            )
        }
    }
}
