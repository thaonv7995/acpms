use anyhow::{Context, Result};
use aws_config::BehaviorVersion;
use aws_sdk_s3::config::Credentials;
use aws_sdk_s3::presigning::{PresignedRequest, PresigningConfig};
use aws_sdk_s3::{config::Region, Client};
use std::env;
use std::time::Duration;

#[derive(Clone)]
pub struct StorageService {
    client: Client,
    public_client: Client, // For presigned URLs with public endpoint
    bucket: String,
    public_endpoint: String, // Store for get_public_url()
}

impl StorageService {
    pub async fn new() -> Result<Self> {
        let endpoint = env::var("S3_ENDPOINT").context("S3_ENDPOINT must be set")?;
        let public_endpoint_env =
            env::var("S3_PUBLIC_ENDPOINT").unwrap_or_else(|_| endpoint.clone());
        // Presigned URL path must match what the proxy forwards to MinIO (path = /bucket/key).
        // If public URL uses /s3 prefix (e.g. https://app.example.com/s3), strip it so the SDK
        // produces URLs like https://app.example.com/acpms-media/avatars/... and the proxy route
        // /:bucket/*path forwards the same path to MinIO so the signature validates.
        let public_endpoint = if let Some(stripped) = public_endpoint_env
            .trim_end_matches('/')
            .strip_suffix("/s3")
        {
            format!("{}/", stripped)
        } else if !public_endpoint_env.ends_with('/') {
            format!("{}/", public_endpoint_env)
        } else {
            public_endpoint_env
        };
        let region = env::var("S3_REGION").unwrap_or_else(|_| "us-east-1".to_string());
        let access_key = env::var("S3_ACCESS_KEY").unwrap_or_default();
        let secret_key = env::var("S3_SECRET_KEY").unwrap_or_default();

        tracing::info!(
            "Initializing StorageService with endpoint: {} (public: {})",
            endpoint,
            public_endpoint
        );

        // Internal client for backend operations
        let config = aws_config::defaults(BehaviorVersion::latest())
            .region(Region::new(region.clone()))
            .endpoint_url(&endpoint)
            .credentials_provider(Credentials::new(
                access_key.clone(),
                secret_key.clone(),
                None,
                None,
                "static",
            ))
            .load()
            .await;

        let s3_config = aws_sdk_s3::config::Builder::from(&config)
            .force_path_style(true)
            .build();
        let client = Client::from_conf(s3_config);

        // Public client for presigned URLs (accessible from browser)
        let public_config = aws_config::defaults(BehaviorVersion::latest())
            .region(Region::new(region))
            .endpoint_url(&public_endpoint)
            .credentials_provider(Credentials::new(
                access_key, secret_key, None, None, "static",
            ))
            .load()
            .await;

        let public_s3_config = aws_sdk_s3::config::Builder::from(&public_config)
            .force_path_style(true)
            .build();
        let public_client = Client::from_conf(public_s3_config);

        let bucket = env::var("S3_BUCKET_NAME").context("S3_BUCKET_NAME must be set")?;

        let service = Self {
            client,
            public_client,
            bucket,
            public_endpoint: public_endpoint.clone(),
        };

        // Ensure bucket exists with retry logic
        service
            .ensure_bucket_exists_with_retry(10, Duration::from_secs(3))
            .await?;

        // Configure CORS - critical for browser uploads. Skip when S3_SKIP_CORS_CONFIG=1 (e.g. local MinIO).
        let skip_cors = env::var("S3_SKIP_CORS_CONFIG")
            .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false);
        if !skip_cors {
            if let Err(e) = service.configure_cors().await {
                tracing::debug!(
                    "Failed to configure CORS: {}. Set S3_SKIP_CORS_CONFIG=1 for local MinIO.",
                    e
                );
            }
        }

        // Configure Public Read - for avatars and public assets
        if let Err(e) = service.configure_public_read().await {
            tracing::warn!("Failed to configure public read access: {}", e);
        }

        tracing::info!("StorageService initialized successfully");
        Ok(service)
    }

    /// Ensure bucket exists with retry logic
    async fn ensure_bucket_exists_with_retry(
        &self,
        max_retries: u32,
        delay: Duration,
    ) -> Result<()> {
        for attempt in 1..=max_retries {
            match self.ensure_bucket_exists().await {
                Ok(_) => {
                    tracing::info!("Bucket '{}' verified/created successfully", self.bucket);
                    return Ok(());
                }
                Err(e) => {
                    if attempt < max_retries {
                        tracing::warn!(
                            "Failed to verify/create bucket (attempt {}/{}): {}. Retrying in {:?}...",
                            attempt, max_retries, e, delay
                        );
                        tokio::time::sleep(delay).await;
                    } else {
                        return Err(e.context(format!(
                            "Failed to verify/create bucket after {} attempts",
                            max_retries
                        )));
                    }
                }
            }
        }
        unreachable!()
    }

    /// Generate a presigned URL for uploading a file (PUT)
    pub async fn get_presigned_upload_url(
        &self,
        key: &str,
        content_type: &str,
        expires_in: Duration,
    ) -> Result<String> {
        let presigning_config = PresigningConfig::expires_in(expires_in)?;

        let presigned_request: PresignedRequest = self
            .public_client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .content_type(content_type)
            .presigned(presigning_config)
            .await?;

        Ok(presigned_request.uri().to_string())
    }

    /// Generate a presigned URL for downloading a file (GET)
    pub async fn get_presigned_download_url(
        &self,
        key: &str,
        expires_in: Duration,
    ) -> Result<String> {
        let presigning_config = PresigningConfig::expires_in(expires_in)?;

        let presigned_request: PresignedRequest = self
            .public_client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .presigned(presigning_config)
            .await?;

        Ok(presigned_request.uri().to_string())
    }

    /// Delete a file from storage
    pub async fn delete_file(&self, key: &str) -> Result<()> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to delete file '{}': {:?}", key, e))?;

        tracing::info!("Deleted file '{}' from bucket '{}'", key, self.bucket);
        Ok(())
    }

    /// Get the public URL for a file (without signing, assuming public access)
    pub fn get_public_url(&self, key: &str) -> String {
        let endpoint = self.public_endpoint.trim_end_matches('/');

        // Construct path-style URL: endpoint/bucket/key
        format!("{}/{}/{}", endpoint, self.bucket, key)
    }

    /// Create a bucket if it doesn't exist (Helper for verify/setup)
    pub async fn ensure_bucket_exists(&self) -> Result<()> {
        tracing::debug!("Checking if bucket '{}' exists...", self.bucket);
        let header = self.client.head_bucket().bucket(&self.bucket).send().await;

        match header {
            Ok(_) => {
                tracing::info!("Bucket '{}' already exists", self.bucket);
                Ok(())
            }
            Err(e) => {
                tracing::debug!("Bucket not found ({}), creating...", e);
                let _output = self
                    .client
                    .create_bucket()
                    .bucket(&self.bucket)
                    .send()
                    .await
                    .map_err(|e| {
                        tracing::error!("Failed to create bucket: {:?}", e);
                        anyhow::anyhow!("Failed to create bucket '{}': {:?}", self.bucket, e)
                    })?;
                tracing::info!("Bucket '{}' created successfully", self.bucket);
                Ok(())
            }
        }
    }

    /// Configure CORS for the bucket
    pub async fn configure_cors(&self) -> Result<()> {
        use aws_sdk_s3::types::{CorsConfiguration, CorsRule};

        let rule = CorsRule::builder()
            .allowed_headers("*")
            .allowed_methods("GET")
            .allowed_methods("PUT")
            .allowed_methods("POST")
            .allowed_methods("DELETE")
            .allowed_methods("HEAD")
            .allowed_origins("*")
            .expose_headers("ETag")
            .build(); // No ? here, wait. build() returns Result?

        // checking aws sdk docs: build() on builders usually returns the type directly if validation works, OR Result if it validates.
        // The compiler said: expected struct `CorsRule`, found enum `Result<CorsRule, ...>`
        // So yes, build() returns Result.

        let config = CorsConfiguration::builder()
            .cors_rules(rule?) // Unwrap rule here
            .build(); // returns Result

        let _output = self
            .client
            .put_bucket_cors()
            .bucket(&self.bucket)
            .cors_configuration(config?) // Unwrap config here
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to configure CORS: {}", e))?;

        Ok(())
    }

    /// Configure public read access (ACL)
    pub async fn configure_public_read(&self) -> Result<()> {
        use aws_sdk_s3::types::BucketCannedAcl;

        let _ = self
            .client
            .put_bucket_acl()
            .bucket(&self.bucket)
            .acl(BucketCannedAcl::PublicRead)
            .send()
            .await;

        Ok(())
    }

    /// Upload JSON data directly to S3
    ///
    /// Serializes the data to JSON and uploads it with application/json content type.
    /// Automatically applies gzip compression for payloads larger than 1MB.
    pub async fn upload_json<T: serde::Serialize>(&self, key: &str, data: &T) -> Result<()> {
        use aws_sdk_s3::primitives::ByteStream;

        let json_bytes = serde_json::to_vec(data).context("Failed to serialize data to JSON")?;

        let content_length = json_bytes.len();

        // Apply gzip compression if larger than 1MB
        let (body_bytes, content_encoding) = if content_length > 1_048_576 {
            use flate2::write::GzEncoder;
            use flate2::Compression;
            use std::io::Write;

            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            encoder
                .write_all(&json_bytes)
                .context("Failed to compress JSON")?;
            let compressed = encoder.finish().context("Failed to finish compression")?;

            tracing::debug!(
                "Compressed JSON from {} bytes to {} bytes ({:.1}% reduction)",
                content_length,
                compressed.len(),
                (1.0 - compressed.len() as f64 / content_length as f64) * 100.0
            );

            (compressed, Some("gzip"))
        } else {
            (json_bytes, None)
        };

        let mut put_request = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .content_type("application/json")
            .body(ByteStream::from(body_bytes));

        if let Some(encoding) = content_encoding {
            put_request = put_request.content_encoding(encoding);
        }

        put_request.send().await.map_err(|e| {
            anyhow::anyhow!(
                "Failed to upload JSON to s3://{}/{}: {}",
                self.bucket,
                key,
                e
            )
        })?;

        tracing::info!(
            "Uploaded JSON to s3://{}/{} ({} bytes)",
            self.bucket,
            key,
            content_length
        );
        Ok(())
    }

    /// Download and parse JSON from S3
    ///
    /// Automatically handles gzip decompression if Content-Encoding header is set.
    pub async fn download_json<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<T> {
        let response = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "Failed to download from s3://{}/{}: {}",
                    self.bucket,
                    key,
                    e
                )
            })?;

        // Check if content is gzip compressed
        let content_encoding = response.content_encoding();
        let is_gzipped = content_encoding
            .map(|e| e.contains("gzip"))
            .unwrap_or(false);

        let bytes = response
            .body
            .collect()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to read S3 response body: {}", e))?
            .into_bytes();

        // Decompress if needed
        let json_bytes = if is_gzipped {
            use flate2::read::GzDecoder;
            use std::io::Read;

            let mut decoder = GzDecoder::new(&bytes[..]);
            let mut decompressed = Vec::new();
            decoder
                .read_to_end(&mut decompressed)
                .context("Failed to decompress gzip data")?;

            tracing::debug!(
                "Decompressed {} bytes to {} bytes",
                bytes.len(),
                decompressed.len()
            );

            decompressed
        } else {
            bytes.to_vec()
        };

        let data = serde_json::from_slice(&json_bytes).context("Failed to parse JSON from S3")?;

        tracing::info!(
            "Downloaded and parsed JSON from s3://{}/{}",
            self.bucket,
            key
        );
        Ok(data)
    }

    /// Download raw object bytes from S3.
    pub async fn download_object_bytes(&self, key: &str) -> Result<Vec<u8>> {
        let response = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "Failed to download from s3://{}/{}: {}",
                    self.bucket,
                    key,
                    e
                )
            })?;

        let bytes = response
            .body
            .collect()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to read S3 response body: {}", e))?
            .into_bytes();

        Ok(bytes.to_vec())
    }

    /// Download first `max_bytes` of object from S3 (head). Caps I/O for pagination.
    pub async fn download_object_bytes_head(&self, key: &str, max_bytes: usize) -> Result<Vec<u8>> {
        let range = format!("bytes=0-{}", max_bytes.saturating_sub(1));
        let response = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .range(range)
            .send()
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "Failed to download head from s3://{}/{}: {}",
                    self.bucket,
                    key,
                    e
                )
            })?;

        let bytes = response
            .body
            .collect()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to read S3 response body: {}", e))?
            .into_bytes();

        Ok(bytes.to_vec())
    }

    /// Download last `max_bytes` of object from S3 (tail). Caps I/O for large log files.
    pub async fn download_object_bytes_tail(&self, key: &str, max_bytes: usize) -> Result<Vec<u8>> {
        let range = format!("bytes=-{}", max_bytes);
        let response = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .range(range)
            .send()
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "Failed to download tail from s3://{}/{}: {}",
                    self.bucket,
                    key,
                    e
                )
            })?;

        let bytes = response
            .body
            .collect()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to read S3 response body: {}", e))?
            .into_bytes();

        Ok(bytes.to_vec())
    }

    /// Check if an object exists in S3
    pub async fn object_exists(&self, key: &str) -> bool {
        self.client
            .head_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .is_ok()
    }

    /// Get object metadata (content_length, content_type) via HEAD. Returns None if not found.
    pub async fn head_object_metadata(&self, key: &str) -> Result<Option<(u64, Option<String>)>> {
        let response = self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await;

        match response {
            Ok(r) => {
                let len = r.content_length().unwrap_or(0) as u64;
                let ct = r.content_type().map(String::from);
                Ok(Some((len, ct)))
            }
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("404") || msg.contains("Not Found") || msg.contains("NoSuchKey") {
                    Ok(None)
                } else {
                    Err(e.into())
                }
            }
        }
    }

    /// Upload assistant session JSONL to S3.
    /// Key format: `assistant-logs/{session_id}.jsonl`
    pub async fn upload_assistant_log_jsonl(
        &self,
        session_id: uuid::Uuid,
        content: &[u8],
    ) -> Result<String> {
        use aws_sdk_s3::primitives::ByteStream;

        let key = format!("assistant-logs/{}.jsonl", session_id);

        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .content_type("application/x-ndjson")
            .body(ByteStream::from(content.to_vec()))
            .send()
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "Failed to upload assistant JSONL to s3://{}/{}: {}",
                    self.bucket,
                    key,
                    e
                )
            })?;

        tracing::info!(
            "Uploaded assistant JSONL to s3://{}/{} ({} bytes)",
            self.bucket,
            key,
            content.len()
        );
        Ok(key)
    }

    /// Upload JSONL log file to S3.
    /// Key format: `attempts/{attempt_id}/logs.jsonl`
    pub async fn upload_jsonl(&self, attempt_id: uuid::Uuid, content: &[u8]) -> Result<String> {
        use aws_sdk_s3::primitives::ByteStream;

        let key = format!("attempts/{}/logs.jsonl", attempt_id);

        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .content_type("application/x-ndjson")
            .body(ByteStream::from(content.to_vec()))
            .send()
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "Failed to upload JSONL to s3://{}/{}: {}",
                    self.bucket,
                    key,
                    e
                )
            })?;

        tracing::info!(
            "Uploaded JSONL logs to s3://{}/{} ({} bytes)",
            self.bucket,
            key,
            content.len()
        );
        Ok(key)
    }

    /// Download JSONL log file from S3.
    /// Returns raw bytes; caller parses line-by-line.
    pub async fn get_log_bytes(&self, s3_log_key: &str) -> Result<Vec<u8>> {
        self.download_object_bytes(s3_log_key).await
    }

    /// Download last `max_bytes` of JSONL log from S3 (tail). Caps I/O for large files.
    pub async fn get_log_bytes_tail(&self, s3_log_key: &str, max_bytes: usize) -> Result<Vec<u8>> {
        self.download_object_bytes_tail(s3_log_key, max_bytes).await
    }

    /// Download first `max_bytes` of JSONL log from S3 (head). Caps I/O for pagination.
    pub async fn get_log_bytes_head(&self, s3_log_key: &str, max_bytes: usize) -> Result<Vec<u8>> {
        self.download_object_bytes_head(s3_log_key, max_bytes).await
    }
}

/// Implement DiffStorageUploader trait so ExecutorOrchestrator can upload
/// diff snapshots without a direct dependency on acpms-services.
#[async_trait::async_trait]
impl acpms_executors::DiffStorageUploader for StorageService {
    async fn upload_diff_snapshot(
        &self,
        key: &str,
        snapshot: &acpms_executors::AttemptDiffSnapshot,
    ) -> anyhow::Result<()> {
        self.upload_json(key, snapshot).await
    }

    async fn download_object_bytes(&self, key: &str) -> anyhow::Result<Vec<u8>> {
        StorageService::download_object_bytes(self, key).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestDiffSnapshot {
        id: String,
        files: Vec<String>,
        total: i32,
    }

    #[tokio::test]
    #[ignore = "requires MinIO"]
    async fn test_upload_download_json() {
        // Set test env vars
        std::env::set_var("S3_ENDPOINT", "http://localhost:9000");
        std::env::set_var("S3_PUBLIC_ENDPOINT", "http://localhost:9000");
        std::env::set_var("S3_ACCESS_KEY", "admin");
        std::env::set_var("S3_SECRET_KEY", "adminpassword123");
        std::env::set_var("S3_REGION", "us-east-1");
        std::env::set_var("S3_BUCKET_NAME", "acpms-media");

        let storage = StorageService::new()
            .await
            .expect("Failed to create StorageService");

        // Test data
        let test_data = TestDiffSnapshot {
            id: "test-123".to_string(),
            files: vec!["file1.rs".to_string(), "file2.rs".to_string()],
            total: 42,
        };

        let test_key = "test/diffs/test-123.json";

        // Upload
        storage
            .upload_json(test_key, &test_data)
            .await
            .expect("Failed to upload JSON");

        println!("✅ JSON uploaded to S3");

        // Download
        let retrieved: TestDiffSnapshot = storage
            .download_json(test_key)
            .await
            .expect("Failed to download JSON");

        println!("✅ JSON downloaded from S3");

        // Verify
        assert_eq!(test_data, retrieved);
        println!("✅ Data matches!");

        // Check exists
        assert!(storage.object_exists(test_key).await);
        println!("✅ Object exists check works");

        // Cleanup
        storage
            .delete_file(test_key)
            .await
            .expect("Failed to delete test file");

        println!("✅ Test file cleaned up");
    }

    #[tokio::test]
    #[ignore = "requires MinIO"]
    async fn test_compression_threshold() {
        std::env::set_var("S3_ENDPOINT", "http://localhost:9000");
        std::env::set_var("S3_PUBLIC_ENDPOINT", "http://localhost:9000");
        std::env::set_var("S3_ACCESS_KEY", "admin");
        std::env::set_var("S3_SECRET_KEY", "adminpassword123");
        std::env::set_var("S3_REGION", "us-east-1");
        std::env::set_var("S3_BUCKET_NAME", "acpms-media");

        let storage = StorageService::new()
            .await
            .expect("Failed to create StorageService");

        // Create large payload (>1MB) to trigger compression
        let large_content = "x".repeat(2_000_000); // 2MB

        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct LargeData {
            content: String,
        }

        let large_data = LargeData {
            content: large_content,
        };
        let test_key = "test/large-compressed.json";

        // Upload (should compress)
        storage
            .upload_json(test_key, &large_data)
            .await
            .expect("Failed to upload large JSON");

        println!("✅ Large JSON uploaded with compression");

        // Download (should decompress)
        let retrieved: LargeData = storage
            .download_json(test_key)
            .await
            .expect("Failed to download JSON");

        assert_eq!(large_data.content.len(), retrieved.content.len());
        println!("✅ Decompression works correctly");

        // Cleanup
        storage.delete_file(test_key).await.ok();
    }
}
