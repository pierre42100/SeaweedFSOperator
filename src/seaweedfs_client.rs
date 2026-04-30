use crate::protos::seaweed_filer_client::SeaweedFilerClient;
use crate::protos::seaweed_identity_access_management_client::SeaweedIdentityAccessManagementClient;
use crate::protos::{GetConfigurationRequest, GetFilerConfigurationRequest};
use std::fmt::Display;
use tonic::codegen::http::uri::InvalidUri;
use tonic::transport::Channel;

#[derive(Debug, thiserror::Error)]
pub enum SeaweedfsClientError {
    #[error("invalid uri: {0}")]
    InvalidUri(#[from] InvalidUri),
    #[error("failed to connect to gRPC endpoint: {0}")]
    ConnectError(#[source] tonic::transport::Error),
    #[error("failed to query gRPC endpoint: {0}")]
    CallError(#[from] tonic::Status),
}

type Res<E> = Result<E, SeaweedfsClientError>;

/// Client for Seaweedfs operations
#[derive(Debug)]
pub struct SeaweedfsInstance {
    name: String,
    url: String,
}

impl SeaweedfsInstance {
    /// Create a new Seaweedfs client instance
    pub fn new<N: Display, U: Display>(name: N, url: U) -> Self {
        Self {
            name: name.to_string(),
            url: url.to_string(),
        }
    }

    /// Connect to gRPC endpoint
    async fn connect(&self) -> Res<tonic::transport::Channel> {
        Ok(Channel::from_shared(self.url.as_bytes().to_vec())?
            .connect()
            .await
            .map_err(SeaweedfsClientError::ConnectError)?)
    }

    /// Get IAM client
    async fn iam_client(&self) -> Res<SeaweedIdentityAccessManagementClient<Channel>> {
        Ok(SeaweedIdentityAccessManagementClient::new(
            self.connect().await?,
        ))
    }

    /// Get Filer client
    async fn filer_client(&self) -> Res<SeaweedFilerClient<Channel>> {
        Ok(SeaweedFilerClient::new(self.connect().await?))
    }

    /// Check if Seaweedfs is ready to service our requests
    pub async fn is_ready(&self) -> Res<bool> {
        let filer_res = self
            .filer_client()
            .await?
            .get_filer_configuration(tonic::Request::new(GetFilerConfigurationRequest {}))
            .await?;

        tracing::debug!("Seaweedfs filer configuration: {:?}", filer_res.get_ref());

        let iam_res = self
            .iam_client()
            .await?
            .get_configuration(tonic::Request::new(GetConfigurationRequest {}))
            .await?;

        tracing::debug!("Seaweedfs iam configuration: {:?}", iam_res.get_ref());

        Ok(!filer_res.get_ref().version.is_empty())
    }

    /*/// Get the list of users
    pub async fn users_list(&self) -> Result<Vec<UserInfo>, SeaweedfsClientError> {
        todo!()
    }

    /// Create a new user
    pub async fn users_create(&self) -> Result<(), SeaweedfsClientError> {
        todo!()
    }

    /// Get the list of buckets
    pub async fn buckets_list(&self) -> Result<Vec<BucketEntry>, SeaweedfsClientError> {
        todo!()
    }

    /// Apply bucket desired configuration. If bucket already exists, it is not dropped
    pub async fn bucket_apply(&self, b: &BucketSpecs) -> Result<(), SeaweedfsClientError> {
        todo!()
    }*/
}
