use crate::protos::seaweed_filer_client::SeaweedFilerClient;
use crate::protos::seaweed_identity_access_management_client::SeaweedIdentityAccessManagementClient;
use crate::protos::{
    CreateEntryRequest, CreateUserRequest, Credential, Entry, FuseAttributes,
    GetConfigurationRequest, GetFilerConfigurationRequest, GetUserRequest, Identity,
    ListEntriesRequest, ListUsersRequest, LookupDirectoryEntryRequest, UpdateEntryRequest,
    UpdateUserRequest,
};
use rand::distr::{Alphanumeric, SampleString};
use std::collections::HashMap;
use std::fmt::Display;
use tonic::Code;
use tonic::codegen::http::uri::InvalidUri;
use tonic::codegen::tokio_stream::StreamExt;
use tonic::transport::Channel;

/// https://pkg.go.dev/io/fs#ModeDir
const OS_MODE_DIR: u32 = 2147483648;

/// Generated s3 access key length
const S3_ACCESS_KEY_LEN: usize = 20;

/// Generated s3 secret key length
const S3_SECRET_KEY_LEN: usize = 30;

/// Seaweed versioning attribute key
const EXT_ATTR_KEY_SEAWEED_VERSIONING: &str = "Seaweed-X-Amz-Versioning";
/// Seaweed locking attribute key
const EXT_ATTR_KEY_SEAWEED_LOCK: &str = "Seaweed-X-Amz-Object-Lock-Enabled";
/// When an attribute is enabled
const EXT_ATTR_KEY_ENABLED: &str = "Enabled";
/// Name of the anonymous user used to provide public access to buckets
const ANONYMOUS_USER: &str = "anonymous";

#[derive(Debug, Clone)]
pub struct UserInfo {
    pub username: String,
    pub access_key: String,
    pub secret_key: String,
}

impl UserInfo {
    pub fn gen_random(prefix: &str) -> Self {
        Self {
            username: format!("{prefix}_usr"),
            access_key: format!(
                "{prefix}_akey_{}",
                Alphanumeric.sample_string(&mut rand::rng(), S3_ACCESS_KEY_LEN)
            ),
            secret_key: Alphanumeric.sample_string(&mut rand::rng(), S3_SECRET_KEY_LEN),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Eq, PartialEq)]
pub struct BucketSpecs {
    /// The name of the bucket
    pub name: String,
    /// Render bucket publicly available
    #[serde(default)]
    pub anonymous_read_access: bool,
    /// Whether versionning should be enabled on bucket
    #[serde(default)]
    pub versioning: bool,
    /// Bucket storage quota
    pub quota: Option<i64>,
    /// Bucket lock
    #[serde(default)]
    pub lock: bool,
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
    #[error("requested bucket does not exists")]
    BucketDoesNotExist,
}

type Res<E> = Result<E, SeaweedfsClientError>;

/// Client for Seaweedfs operations
#[derive(Debug)]
pub struct SeaweedfsInstance {
    url: String,
}

impl SeaweedfsInstance {
    /// Create a new Seaweedfs client instance
    pub fn new<U: Display>(url: U) -> Self {
        Self {
            url: url.to_string(),
        }
    }

    /// Connect to gRPC endpoint
    async fn connect(&self) -> Res<Channel> {
        Channel::from_shared(self.url.as_bytes().to_vec())?
            .connect()
            .await
            .map_err(SeaweedfsClientError::ConnectError)
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
            .await;

        match res {
            Ok(res) => {
                let Some(identity) = res.into_inner().identity else {
                    return Err(SeaweedfsClientError::UserDoesNotExist);
                };

                Ok(identity)
            }
            Err(e) if e.code() == Code::NotFound => Err(SeaweedfsClientError::UserDoesNotExist),
            Err(e) => Err(SeaweedfsClientError::CallError(e)),
        }
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
            .get_filer_configuration(tonic::Request::new(GetFilerConfigurationRequest {}))
            .await?
            .into_inner();

        let mut stream = filer_client
            .list_entries(tonic::Request::new(ListEntriesRequest {
                directory: filer_config.dir_buckets,
                prefix: "".to_string(),
                start_from_file_name: "".to_string(),
                inclusive_start_from: false,
                limit: u32::MAX,
                snapshot_ts_ns: 0,
            }))
            .await?
            .into_inner();

        let mut list = Vec::new();
        while let Some(item) = stream.next().await {
            let item = item?;
            if let Some(entry) = item.entry.clone()
                && entry.is_directory
            {
                tracing::debug!("Found bucket entry: {:?}", entry);
                list.push(entry);
            } else {
                tracing::debug!("Skipping bucket entry: {item:?}");
            }
        }

        Ok(list)
    }

    /// Get a single bucket entry
    async fn bucket_get_single(&self, name: &str) -> Res<Entry> {
        let mut filer_client = self.filer_client().await?;
        let filer_config = filer_client
            .get_filer_configuration(GetFilerConfigurationRequest {})
            .await?
            .into_inner();

        let response = filer_client
            .lookup_directory_entry(LookupDirectoryEntryRequest {
                directory: filer_config.dir_buckets,
                name: name.to_string(),
            })
            .await
            .map_err(|e| match (e.code(), e.message()) {
                (Code::NotFound, _) => SeaweedfsClientError::BucketDoesNotExist,
                (Code::Unknown, s) if s.contains("no entry is found in filer store") => {
                    SeaweedfsClientError::BucketDoesNotExist
                }
                _ => SeaweedfsClientError::CallError(e),
            })?;

        response
            .into_inner()
            .entry
            .ok_or(SeaweedfsClientError::BucketDoesNotExist)
    }

    /// Get a single bucket information
    pub async fn bucket_info(&self, name: &str) -> Res<BucketSpecs> {
        let entry = self.bucket_get_single(name).await?;

        Ok(BucketSpecs {
            name: name.to_string(),
            anonymous_read_access: self.bucket_anonymous_get(name).await?,
            versioning: entry
                .extended
                .get(EXT_ATTR_KEY_SEAWEED_VERSIONING)
                .map(Vec::as_slice)
                .unwrap_or(&[])
                == EXT_ATTR_KEY_ENABLED.as_bytes(),
            quota: match entry.quota > 0 {
                true => Some(entry.quota),
                false => None,
            },
            lock: entry
                .extended
                .get(EXT_ATTR_KEY_SEAWEED_LOCK)
                .map(Vec::as_slice)
                .unwrap_or(&[])
                == EXT_ATTR_KEY_ENABLED.as_bytes(),
        })
    }

    /// Get anonymous access status to bucket
    pub async fn bucket_anonymous_get(&self, bucket: &str) -> Res<bool> {
        match self.user_info(ANONYMOUS_USER).await {
            Ok(u) => Ok(u.actions.contains(&format!("Read:{bucket}"))),
            Err(SeaweedfsClientError::UserDoesNotExist) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Apply anonymous access to bucket
    pub async fn bucket_anonymous_apply(&self, b: &BucketSpecs) -> Res<()> {
        let (new_user, mut identity) = match self.user_info(ANONYMOUS_USER).await {
            Ok(i) => (false, i),
            Err(SeaweedfsClientError::UserDoesNotExist) if !b.anonymous_read_access => {
                return Ok(());
            }
            Err(SeaweedfsClientError::UserDoesNotExist) => (
                true,
                Identity {
                    name: "anonymous".to_string(),
                    credentials: vec![],
                    actions: vec![],
                    account: None,
                    disabled: false,
                    service_account_ids: vec![],
                    policy_names: vec![],
                    is_static: false,
                },
            ),
            Err(e) => return Err(e),
        };

        // Update user permissions
        let perm_suffix = format!(":{}", b.name);
        identity
            .actions
            .retain(|p| !p.ends_with(perm_suffix.as_str()));

        if b.anonymous_read_access {
            identity.actions.push(format!("Read{perm_suffix}"))
        }

        // Update identity
        match new_user {
            false => {
                self.iam_client()
                    .await?
                    .update_user(UpdateUserRequest {
                        username: ANONYMOUS_USER.to_string(),
                        identity: Some(identity),
                    })
                    .await?;
            }
            true => {
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

    /// Apply bucket desired configuration. If bucket already exists, it is not dropped
    pub async fn bucket_apply(
        &self,
        b: &BucketSpecs,
        user: &UserInfo,
    ) -> Result<(), SeaweedfsClientError> {
        let mut filer_client = self.filer_client().await?;
        let filer_config = filer_client
            .get_filer_configuration(tonic::Request::new(GetFilerConfigurationRequest {}))
            .await?
            .into_inner();

        let mut extended = HashMap::from([(
            "s3-identity-id".to_string(),
            user.username.as_bytes().to_vec(),
        )]);

        if b.lock || b.versioning {
            extended.insert(
                EXT_ATTR_KEY_SEAWEED_VERSIONING.to_string(),
                EXT_ATTR_KEY_ENABLED.as_bytes().to_vec(),
            );
        }

        if b.lock {
            extended.insert(
                EXT_ATTR_KEY_SEAWEED_LOCK.to_string(),
                EXT_ATTR_KEY_ENABLED.as_bytes().to_vec(),
            );
        }

        let entry = Entry {
            name: b.name.to_string(),
            is_directory: true,
            chunks: vec![],
            attributes: Some(FuseAttributes {
                file_size: 0,
                mtime: 0,
                file_mode: 0o0777 | OS_MODE_DIR,
                uid: 0,
                gid: 0,
                crtime: 0,
                mime: "".to_string(),
                ttl_sec: 0,
                user_name: "".to_string(),
                group_name: vec![],
                symlink_target: "".to_string(),
                md5: vec![],
                rdev: 0,
                inode: 0,
                ctime: 0,
                mtime_ns: 0,
                ctime_ns: 0,
                crtime_ns: 0,
            }),
            extended,
            hard_link_id: vec![],
            hard_link_counter: 0,
            content: vec![],
            remote_entry: None,
            quota: b.quota.unwrap_or(0),
            worm_enforced_at_ts_ns: 0,
        };

        // Create or update bucket
        match self.bucket_get_single(&b.name).await {
            Ok(_) => {
                tracing::info!("Update bucket {} information", b.name);
                filer_client
                    .update_entry(UpdateEntryRequest {
                        directory: filer_config.dir_buckets,
                        entry: Some(entry),
                        is_from_other_cluster: false,
                        signatures: vec![],
                        expected_extended: Default::default(),
                    })
                    .await?;
            }
            Err(SeaweedfsClientError::BucketDoesNotExist) => {
                tracing::info!("Create bucket {}", b.name);
                filer_client
                    .create_entry(CreateEntryRequest {
                        directory: filer_config.dir_buckets,
                        entry: Some(entry),
                        o_excl: false,
                        is_from_other_cluster: false,
                        signatures: vec![],
                        skip_check_parent_directory: false,
                    })
                    .await?;
            }
            Err(e) => return Err(e),
        }

        self.bucket_anonymous_apply(b).await?;

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::seaweedfs_client::{BucketSpecs, SeaweedfsClientError, UserInfo};
    use crate::seaweedfs_test_server::SeaweedfsTestServer;
    use rand::distr::{Alphanumeric, SampleString};

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

        assert!(matches!(
            inst.user_info(&user).await,
            Err(SeaweedfsClientError::UserDoesNotExist)
        ));

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

    #[tokio::test]
    #[test_log::test]
    async fn create_buckets() {
        let srv = SeaweedfsTestServer::start().await.unwrap();
        let instance = srv.as_instance();

        assert_eq!(instance.buckets_list().await.unwrap(), vec![]);

        let user_1 = UserInfo {
            username: "user1".to_string(),
            access_key: "u1accskey".to_string(),
            secret_key: "u1seckey".to_string(),
        };

        let mut bucket_1 = BucketSpecs {
            name: "firstbucket".to_string(),
            anonymous_read_access: false,
            versioning: false,
            quota: None,
            lock: false,
        };

        instance.user_apply(user_1.clone()).await.unwrap();
        instance.bucket_apply(&bucket_1, &user_1).await.unwrap();

        assert_ne!(instance.buckets_list().await.unwrap(), vec![]);

        assert_eq!(
            instance.bucket_info(&bucket_1.name).await.unwrap(),
            bucket_1
        );

        let bucket_url = format!("{}/{}/random", srv.s3_url(), bucket_1.name);
        assert_eq!(reqwest::get(&bucket_url).await.unwrap().status(), 403);

        // Update bucket information
        bucket_1.anonymous_read_access = true;
        bucket_1.quota = Some(10000);
        instance.bucket_apply(&bucket_1, &user_1).await.unwrap();
        assert_eq!(
            instance.bucket_info(&bucket_1.name).await.unwrap(),
            bucket_1
        );

        // Update bucket information, again
        bucket_1.versioning = true;
        bucket_1.lock = true;
        bucket_1.quota = None;
        instance.bucket_apply(&bucket_1, &user_1).await.unwrap();
        assert_eq!(
            instance.bucket_info(&bucket_1.name).await.unwrap(),
            bucket_1
        );

        assert_eq!(reqwest::get(&bucket_url).await.unwrap().status(), 404);

        // Update bucket information, for the last time
        bucket_1.versioning = false;
        bucket_1.lock = false;
        bucket_1.quota = None;
        bucket_1.anonymous_read_access = false;
        instance.bucket_apply(&bucket_1, &user_1).await.unwrap();
        assert_eq!(
            instance.bucket_info(&bucket_1.name).await.unwrap(),
            bucket_1
        );

        let user_2 = UserInfo {
            username: "user2".to_string(),
            access_key: "u2accskey".to_string(),
            secret_key: "u2seckey".to_string(),
        };

        let bucket_2 = BucketSpecs {
            name: "secondbucket".to_string(),
            anonymous_read_access: false,
            versioning: false,
            quota: None,
            lock: false,
        };

        let bucket_3 = BucketSpecs {
            name: "thirdbucket".to_string(),
            anonymous_read_access: false,
            versioning: false,
            quota: None,
            lock: false,
        };

        instance.user_apply(user_2.clone()).await.unwrap();
        instance.bucket_apply(&bucket_2, &user_2).await.unwrap();
        instance.bucket_apply(&bucket_3, &user_2).await.unwrap();
        assert_eq!(
            instance.bucket_info(&bucket_2.name).await.unwrap(),
            bucket_2
        );
        assert_eq!(
            instance.bucket_info(&bucket_3.name).await.unwrap(),
            bucket_3
        );

        assert_eq!(instance.buckets_list().await.unwrap().len(), 3);
    }
}
