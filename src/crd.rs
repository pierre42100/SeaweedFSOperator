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
pub struct MinioBucketSpec {
    pub instance: String,
    pub name: String,
    pub secret: String,
    #[serde(default)]
    pub anonymous_read_access: bool,
    #[serde(default)]
    pub versioning: bool,
    pub quota: Option<usize>,
    #[serde(default)]
    pub lock: bool,
}
