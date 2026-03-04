#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Serialize, Deserialize};

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

        let storage = StorageService::new().await.expect("Failed to create StorageService");

        // Test data
        let test_data = TestDiffSnapshot {
            id: "test-123".to_string(),
            files: vec!["file1.rs".to_string(), "file2.rs".to_string()],
            total: 42,
        };

        let test_key = "test/diffs/test-123.json";

        // Upload
        storage.upload_json(test_key, &test_data)
            .await
            .expect("Failed to upload JSON");

        println!("✅ JSON uploaded to S3");

        // Download
        let retrieved: TestDiffSnapshot = storage.download_json(test_key)
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
        storage.delete_file(test_key)
            .await
            .expect("Failed to delete test file");

        println!("✅ Test file cleaned up");
        println!("\n🎉 All S3 JSON operations working correctly!");
    }

    #[tokio::test]
    #[ignore = "requires MinIO"]
    async fn test_compression_large_payload() {
        std::env::set_var("S3_ENDPOINT", "http://localhost:9000");
        std::env::set_var("S3_PUBLIC_ENDPOINT", "http://localhost:9000");
        std::env::set_var("S3_ACCESS_KEY", "admin");
        std::env::set_var("S3_SECRET_KEY", "adminpassword123");
        std::env::set_var("S3_REGION", "us-east-1");
        std::env::set_var("S3_BUCKET_NAME", "acpms-media");

        let storage = StorageService::new().await.expect("Failed to create StorageService");

        // Create large payload (>1MB) to trigger compression
        let large_content = "x".repeat(2_000_000); // 2MB of 'x'

        #[derive(Serialize, Deserialize)]
        struct LargeData {
            content: String,
        }

        let large_data = LargeData { content: large_content };
        let test_key = "test/large-compressed.json";

        // Upload (should trigger gzip compression)
        storage.upload_json(test_key, &large_data)
            .await
            .expect("Failed to upload large JSON");

        println!("✅ Large JSON uploaded (with compression)");

        // Download (should auto-decompress)
        let retrieved: LargeData = storage.download_json(test_key)
            .await
            .expect("Failed to download large JSON");

        assert_eq!(large_data.content.len(), retrieved.content.len());
        println!("✅ Large JSON decompressed correctly");

        // Cleanup
        storage.delete_file(test_key).await.expect("Failed to cleanup");
        println!("✅ Compression test passed!");
    }
}
