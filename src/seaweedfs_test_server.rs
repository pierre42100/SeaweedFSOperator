//! # Seaweedfs server controller
//!
//! Used for testing only

use crate::seaweedfs_client::SeaweedfsInstance;
use rand::RngExt;
use std::ops::Range;
use std::process::{Child, Command};
use std::time::Duration;

#[derive(thiserror::Error, Debug)]
pub enum SeaweedfsTestServerError {
    #[error("temp dir error: {0}")]
    TempDir(#[source] std::io::Error),
    #[error("spawn process error: {0}")]
    SpawnProcess(#[source] std::io::Error),
    #[error("seaweedfs failed to start in time!")]
    StartTimeout,
}

pub struct SeaweedfsTestServer {
    #[allow(dead_code)]
    storage_base_dir: temp_dir::TempDir,
    child: Child,
    filer_grpc_port: u16,
    s3_port: u16,
}

impl SeaweedfsTestServer {
    pub async fn start() -> Result<Self, SeaweedfsTestServerError> {
        let storage_dir = temp_dir::TempDir::new().map_err(SeaweedfsTestServerError::TempDir)?;

        let filer_grpc_port = rand::rng().random_range::<u16, Range<u16>>(2000..40000);
        let s3_port = rand::rng().random_range::<u16, Range<u16>>(2000..40000);

        assert_ne!(filer_grpc_port, s3_port);

        let weed_binary = std::env::var("WEED_BINARY").unwrap_or("weed".to_string());
        tracing::debug!("weed_binary: {weed_binary}");
        let child = Command::new(weed_binary)
            .current_dir(storage_dir.path())
            .arg("mini")
            .arg(format!("-dir={}", storage_dir.path().to_string_lossy()))
            .arg(format!("-filer.port.grpc={filer_grpc_port}"))
            .arg(format!("-s3.port={s3_port}"))
            .arg("-admin.ui=false")
            .arg("-webdav=false")
            .arg("-disableHttp")
            .arg("-filer.allowedOrigins='127.0.0.1'")
            .arg("-filer.disableDirListing")
            .arg("-filer.ui.deleteDir=false")
            .arg("-master.telemetry=false")
            .spawn()
            .map_err(SeaweedfsTestServerError::SpawnProcess)?;

        let srv = Self {
            storage_base_dir: storage_dir,
            child,
            filer_grpc_port,
            s3_port,
        };

        // Wait for Seaweedfs to become ready
        tokio::time::sleep(Duration::from_millis(500)).await;
        for _ in 1..100 {
            tokio::time::sleep(Duration::from_millis(500)).await;

            if let Ok(true) = srv.as_instance().is_ready().await {
                return Ok(srv);
            }
        }

        tracing::error!("Seaweedfs failed to respond properly in time!");
        Err(SeaweedfsTestServerError::StartTimeout)
    }

    /// Get filer gRPC url of this test server
    pub fn filer_grpc_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.filer_grpc_port)
    }

    /// Get s3 url of this test server
    pub fn s3_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.s3_port)
    }

    /// Get a Seaweedfs instance of this temporary server
    pub fn as_instance(&self) -> SeaweedfsInstance {
        SeaweedfsInstance::new(self.filer_grpc_url())
    }
}

impl Drop for SeaweedfsTestServer {
    fn drop(&mut self) {
        tracing::info!("killing process {}", self.child.id());
        if let Err(e) = self.child.kill() {
            tracing::error!("Failed to kill child server! {}", e);
        }
    }
}

#[cfg(test)]
mod test {
    use crate::seaweedfs_test_server::SeaweedfsTestServer;

    #[tokio::test]
    #[test_log::test]
    async fn start_minio() {
        let server = SeaweedfsTestServer::start().await.unwrap();
        let instance = server.as_instance();
        println!("{instance:#?}");

        assert!(instance.is_ready().await.unwrap());

        drop(server);
        instance.is_ready().await.unwrap_err();
    }
}
