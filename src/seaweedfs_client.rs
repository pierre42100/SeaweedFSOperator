use crate::protos::seaweed_filer_client::SeaweedFilerClient;
use crate::protos::seaweed_identity_access_management_client::SeaweedIdentityAccessManagementClient;
use crate::protos::{CreateUserRequest, Credential, Entry, GetConfigurationRequest, GetFilerConfigurationRequest, GetUserRequest, Identity, ListEntriesRequest, ListUsersRequest, UpdateUserRequest};
use std::fmt::Display;
use tonic::codegen::http::uri::InvalidUri;
use tonic::codegen::tokio_stream::StreamExt;
use tonic::transport::Channel;

#[derive(Debug, Clone)]
pub struct UserInfo {
    username: String,
    access_key: String,
    secret_key: String,
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
    UserDoesNotExist,
}

type Res<E> = Result<E, SeaweedfsClientError>;

/// Client for Seaweedfs operations
#[derive(Debug)]
pub struct SeaweedfsInstance {
    url: String,
}

impl SeaweedfsInstance {
    /// Create a new Seaweedfs client instance
    pub fn new<N: Display, U: Display>(name: N, url: U) -> Self {
        Self {
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

    /// Get a single user information
    pub async fn user_info<U: Display>(&self, username: U) -> Res<Identity> {
        let res = self
            .iam_client()
            .await?
            .get_user(tonic::Request::new(GetUserRequest {
                username: username.to_string(),
            }))
            .await?;

        let Some(identity) = res.into_inner().identity else {
            return Err(SeaweedfsClientError::UserDoesNotExist);
        };

        Ok(identity)
    }

    /// Create or update user information
    pub async fn user_apply(&self, info: UserInfo) -> Result<(), SeaweedfsClientError> {
        let identity = Identity {
            name: info.username,
            credentials: vec![Credential {
                access_key: info.access_key,
                secret_key: info.secret_key,
                status: "Active".to_string(),
            }],
            actions: vec![],
            account: None,
            disabled: false,
            service_account_ids: vec![],
            policy_names: vec![],
            is_static: false,
        };

        // Create or update user information
        match self.users_list().await?.contains(&identity.name) {
            true => {
                self.iam_client()
                    .await?
                    .update_user(UpdateUserRequest {
                        username: identity.name.to_string(),
                        identity: Some(identity),
                    })
                    .await?;
            }
            false => {
                self.iam_client()
                    .await?
                    .create_user(CreateUserRequest {
                        identity: Some(identity),
                    })
                    .await?;
            }
        }

        Ok(())
    }

    /// Get the list of buckets
    pub async fn buckets_list(&self) -> Result<Vec<Entry>, SeaweedfsClientError> {
        let mut filer_client = self.filer_client().await?;
        let filer_config = filer_client
            .get_filer_configuration(tonic::Request::new(GetFilerConfigurationRequest {})).await?.into_inner();

        let mut stream = filer_client.list_entries(tonic::Request::new(ListEntriesRequest {
            directory: filer_config.dir_buckets,
            prefix: "".to_string(),
            start_from_file_name: "".to_string(),
            inclusive_start_from: false,
            limit: u32::MAX,
            snapshot_ts_ns: 0,
        })).await?.into_inner();


        let mut list = Vec::new();
        while let Some(item) = stream.next().await {
            let item = item?;
            if let Some(entry)= item.entry && entry.is_directory{
                list.push(entry);
            }
        }

        Ok(list)
    }

    /*/// Apply bucket desired configuration. If bucket already exists, it is not dropped
    pub async fn bucket_apply(&self, b: &BucketSpecs) -> Result<(), SeaweedfsClientError> {
        todo!()
    }*/
}

#[cfg(test)]
mod test {
    use rand::distr::{Alphanumeric, SampleString};
    use crate::seaweedfs_client::UserInfo;
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

    #[tokio::test]
    #[test_log::test]
    async fn create_update_user() {
        let user = "myuser";

        let srv = SeaweedfsTestServer::start().await.unwrap();
        let inst = srv.as_instance();
        let users = inst.users_list().await.unwrap();
        assert!(users.is_empty());

        // Create user
        let initial_info = UserInfo {
            username: user.to_string(),
            access_key: Alphanumeric.sample_string(&mut rand::rng(), 16),
            secret_key: Alphanumeric.sample_string(&mut rand::rng(), 16),
        };
        inst.user_apply(initial_info.clone()).await.unwrap();

        let users = inst.users_list().await.unwrap();
        assert_eq!(users, &[user.to_string()]);
        let id = inst.user_info(user).await.unwrap();
        assert_eq!(id.name, user);
        assert_eq!(id.credentials.len(), 1);
        assert_eq!(id.credentials[0].access_key, initial_info.access_key);
        assert_eq!(id.credentials[0].secret_key, initial_info.secret_key);

        // Update user
        let new_info = UserInfo {
            username: user.to_string(),
            access_key: Alphanumeric.sample_string(&mut rand::rng(), 16),
            secret_key: Alphanumeric.sample_string(&mut rand::rng(), 16),
        };
        inst.user_apply(new_info.clone()).await.unwrap();

        let users = inst.users_list().await.unwrap();
        assert_eq!(users, &[user.to_string()]);
        let id = inst.user_info(user).await.unwrap();
        assert_eq!(id.name, user);
        assert_eq!(id.credentials.len(), 1);
        assert_eq!(id.credentials[0].access_key, new_info.access_key);
        assert_eq!(id.credentials[0].secret_key, new_info.secret_key);

        // Create second user
        let second_user = UserInfo {
            username: "zsecond".to_string(),
            access_key: Alphanumeric.sample_string(&mut rand::rng(), 16),
            secret_key: Alphanumeric.sample_string(&mut rand::rng(), 16),
        };
        inst.user_apply(second_user.clone()).await.unwrap();

        let users = inst.users_list().await.unwrap();
        assert_eq!(users, &[user.to_string(), second_user.username]);
    }

    #[tokio::test]
    #[test_log::test]
    async fn list_buckets_empty_instance() {
        let srv = SeaweedfsTestServer::start().await.unwrap();
        let buckets = srv.as_instance().buckets_list().await.unwrap();
        assert!(buckets.is_empty());
    }
}
