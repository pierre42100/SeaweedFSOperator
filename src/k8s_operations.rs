use crate::crd::SeaweedFSBucket;
use kube::Client;

#[derive(Debug, thiserror::Error)]
pub enum K8sOperationError {}

type Res<R> = Result<R, K8sOperationError>;

/// Apply a bucket
#[tracing::instrument(fields(bucket=bucket.spec.name), skip(client, bucket))]
pub async fn apply_bucket(bucket: &SeaweedFSBucket, client: &Client) -> Res<()> {
    tracing::info!("apply bucket {} configuration", bucket.spec.name);

    Ok(())
}
