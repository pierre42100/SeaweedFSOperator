use crate::seaweedfs_client::BucketSpecs;
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(CustomResource, Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
#[kube(group = "communiquons.org", version = "v1", kind = "SeaweedFSInstance")]
pub struct SeaweedFSInstanceSpec {
    pub filergrpc: String,
}

#[derive(CustomResource, Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
#[kube(
    group = "communiquons.org",
    version = "v1",
    kind = "SeaweedFSBucket",
    namespaced
)]
pub struct SeaweedFSBucketSpec {
    pub instance: String,
    pub name: String,
    pub secret: String,
    #[serde(default)]
    pub anonymous_read_access: bool,
    #[serde(default)]
    pub versioning: bool,
    pub quota: Option<i64>,
    #[serde(default)]
    pub lock: bool,
}

impl From<&SeaweedFSBucket> for BucketSpecs {
    fn from(bucket: &SeaweedFSBucket) -> Self {
        Self {
            name: bucket.spec.name.to_string(),
            anonymous_read_access: bucket.spec.anonymous_read_access,
            versioning: bucket.spec.versioning,
            quota: bucket.spec.quota,
            lock: bucket.spec.lock,
        }
    }
}
