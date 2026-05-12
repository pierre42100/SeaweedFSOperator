use crate::crd;
use crate::seaweedfs_client::SeaweedfsInstance;
use kube::{Api, Client};
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum K8sOperationError {
    #[error("failed to get instance information")]
    GetSeaweedFSInstanceInfo(#[source] kube::Error),
    #[error("seaweedfs instance has no name!")]
    GetSeaweedFSInstanceHasNoName,
    #[error("seaweedfs instance is not ready!")]
    SeaweedFSInstanceNotReady,
}

type Res<R> = Result<R, K8sOperationError>;

/// Apply a bucket
#[tracing::instrument(fields(bucket=bucket.spec.name), skip(client, bucket))]
pub async fn apply_bucket(bucket: &crd::SeaweedFSBucket, client: &Client) -> Res<()> {
    tracing::info!("apply bucket {} configuration", bucket.spec.name);

    // Get instance information
    let instances: Api<crd::SeaweedFSInstance> = Api::all(client.clone());
    let instance = instances
        .get(&bucket.spec.instance)
        .await
        .map_err(K8sOperationError::GetSeaweedFSInstanceInfo)?;

    let Some(seaweedfs_instance_name) = instance.metadata.name else {
        return Err(K8sOperationError::GetSeaweedFSInstanceHasNoName);
    };

    tracing::debug!(
        "found seaweedfs instance {} (grpc url={})",
        seaweedfs_instance_name,
        instance.spec.filergrpc
    );

    // Check if Seaweedfs is responding; try multiple time before giving up
    let instance = SeaweedfsInstance::new(&instance.spec.filergrpc);
    wait_seaweedfs_ready(&instance, &seaweedfs_instance_name).await?;

    // TODO : continue here

    Ok(())
}

/// Wait for seaweedfs to become ready
async fn wait_seaweedfs_ready(instance: &SeaweedfsInstance, name: &str) -> Res<()> {
    let mut attempts = 10;
    loop {
        tracing::debug!("seaweedfs instance check #{attempts}");
        match instance.is_ready().await {
            Ok(true) => {
                tracing::debug!("seaweedfs instance {name} reported to be ready");
                return Ok(());
            }
            Ok(false) => {
                tracing::warn!("seaweedfs instance {name}  reported not to be ready");
            }

            Err(e) => {
                tracing::error!("could not check if seaweedfs instance is ready! (error {e})");
            }
        }

        attempts -= 1;

        // Check if counter is ended
        if attempts == 0 {
            return Err(K8sOperationError::SeaweedFSInstanceNotReady);
        }

        tracing::warn!(
            "Seaweedfs instance is not responding yet, will try again to connect soon (attempt {attempts})..."
        );
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}
