use crate::protos::seaweed_filer_client::SeaweedFilerClient;
use crate::protos::seaweed_identity_access_management_client::SeaweedIdentityAccessManagementClient;
use crate::protos::{CreateUserRequest, GetConfigurationRequest, GetFilerConfigurationRequest, GetUserRequest, Identity, ListUsersRequest};
use std::fmt::Display;
use tonic::codegen::http::uri::InvalidUri;
use tonic::transport::Channel;

#[derive(Debug)]
pub struct UserInfo {
    // TODO
}

#[derive(Debug, thiserror::Error)]
pub enum SeaweedfsClientError {
    #[error("invalid uri: {0}")]
    InvalidUri(#[from] InvalidUri),
    #[error("failed to connect to gRPC endpoint: {0}")]
    ConnectError(#[source] tonic::transport::Error),
    #[error("failed to query gRPC endpoint: {0}")]
    CallError(#[from] tonic::Status),
    #[error("requested user does not exists")]
    UserDoesNotExist
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

    /// Get the list of users
    pub async fn users_list(&self) -> Res<Vec<String>> {
        let users = self
            .iam_client()
            .await?
            .list_users(tonic::Request::new(ListUsersRequest {}))
            .await?;
        Ok(users.into_inner().usernames)
    }

    /// Get a user information
    pub async fn user_info(&self) -> Res<Identity> {
        let res = self
            .iam_client()
            .await?
            .get_user(tonic::Request::new(GetUserRequest {
                username: self.name.clone(),
            }))
            .await?;
        
        let Some(identity)=res.into_inner().identity else {
            return Err(SeaweedfsClientError::UserDoesNotExist);
        };

        Ok(identity)
    }

    /// Create or update user information
    pub async fn users_apply(&self, info: UserInfo) -> Result<(), SeaweedfsClientError> {
        todo!()
    }

    /*/// Get the list of buckets
    pub async fn buckets_list(&self) -> Result<Vec<BucketEntry>, SeaweedfsClientError> {
        todo!()
    }

    /// Apply bucket desired configuration. If bucket already exists, it is not dropped
    pub async fn bucket_apply(&self, b: &BucketSpecs) -> Result<(), SeaweedfsClientError> {
        todo!()
    }*/
}

#[cfg(test)]
mod test {
    use crate::seaweedfs_test_server::SeaweedfsTestServer;

    const TEST_BUCKET_NAME: &str = "mybucket";
    const TEST_POLICY_NAME: &str = "mypolicy";

    #[tokio::test]
    #[test_log::test]
    async fn list_users_empty_instance() {
        let srv = SeaweedfsTestServer::start().await.unwrap();
        let users = srv.as_instance().users_list().await.unwrap();
        assert!(users.is_empty());
    }
}
