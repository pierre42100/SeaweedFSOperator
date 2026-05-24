use k8s_openapi::api::core::v1::Secret;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::api::PostParams;
use kube::{Api, Client};
use std::collections::BTreeMap;
use std::string::FromUtf8Error;

#[derive(thiserror::Error, Debug)]
pub enum SecretError {
    #[error("failed to get secret: {0}")]
    GetSecret(#[source] kube::Error),
    #[error("failed to create secret: {0}")]
    CreateSecret(#[source] kube::Error),
    #[error("secret {0:?} has no data!")]
    MissingDataInSecret(Option<String>),
    #[error("the key '{1}' is not present in the secret {0:?}!")]
    MissingKeyInSecret(Option<String>, String),
    #[error("failed to decode secret value {0} a string: {1:?}!")]
    DecodeSecretValueAsString(String, #[source] FromUtf8Error),
}

type Res<S> = Result<S, SecretError>;

pub struct SecretReader<'a>(&'a Secret);

impl SecretReader<'_> {
    pub fn read(&self, k: &str) -> Res<String> {
        read_secret_str(&self.0, k)
    }
}

/// Get or create a secret if it does not exist
pub async fn read_or_create_secret<E, G, D>(
    client: &Client,
    name: &str,
    namespace: &str,
    generate: G,
    decode: D,
) -> Res<E>
where
    G: FnOnce() -> BTreeMap<String, String>,
    D: Fn(&SecretReader) -> Res<E>,
{
    let secrets: Api<Secret> = Api::namespaced(client.clone(), namespace);
    let secret = match secrets
        .get_opt(name)
        .await
        .map_err(SecretError::GetSecret)?
    {
        Some(s) => s,
        None => {
            tracing::info!("need to create the secret {name}");
            create_secret(&secrets, name, generate()).await?
        }
    };

    decode(&SecretReader(&secret))
}

/// Attempt to read a value contained in a secret. Returns an error in case
/// of failure
fn read_secret_str(s: &Secret, key: &str) -> Res<String> {
    let data = s
        .data
        .as_ref()
        .ok_or(SecretError::MissingDataInSecret(s.metadata.name.clone()))?;

    let value = data.get(key).ok_or(SecretError::MissingKeyInSecret(
        s.metadata.name.clone(),
        key.to_string(),
    ))?;

    Ok(String::from_utf8(value.0.clone())
        .map_err(|e| SecretError::DecodeSecretValueAsString(key.to_string(), e))?)
}

/// Create a secret consisting only of string key / value pairs
async fn create_secret(
    secrets: &Api<Secret>,
    name: &str,
    values: BTreeMap<String, String>,
) -> Res<Secret> {
    Ok(secrets
        .create(
            &PostParams::default(),
            &Secret {
                data: None,
                immutable: None,
                metadata: ObjectMeta {
                    annotations: None,
                    creation_timestamp: None,
                    deletion_grace_period_seconds: None,
                    deletion_timestamp: None,
                    finalizers: None,
                    generate_name: None,
                    generation: None,
                    labels: Some(BTreeMap::from([(
                        "created-by".to_string(),
                        "seaweedfs-k8s-operator".to_string(),
                    )])),
                    managed_fields: None,
                    name: Some(name.to_string()),
                    namespace: None,
                    owner_references: None,
                    resource_version: None,
                    self_link: None,
                    uid: None,
                },
                string_data: Some(values),
                type_: None,
            },
        )
        .await
        .map_err(SecretError::CreateSecret)?)
}
