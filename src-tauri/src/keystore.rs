use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use async_trait::async_trait;
use tauri::Manager;
use russh_keys::PublicKeyBase64;
use zeroize::Zeroizing;
#[cfg(target_os = "android")]
use log::info;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KeyIdentity {
    pub id: String,
    pub label: String,
    pub public_key: String, // OpenSSH format
    pub created_at: i64,
}

#[async_trait]
pub trait KeyManager: Send + Sync {
    /// Generate a new hardware-backed key pair (if supported) or a standard one.
    /// Returns the public key in OpenSSH format.
    async fn generate_key(&self, label: String) -> Result<KeyIdentity, String>;

    /// List all identities managed by this manager.
    async fn list_identities(&self) -> Result<Vec<KeyIdentity>, String>;

    /// Delete an identity.
    async fn delete_identity(&self, id: String) -> Result<(), String>;

    /// Sign data using the private key associated with the ID.
    /// This may trigger a biometric prompt on mobile.
    async fn sign(&self, id: String, data: &[u8], reason: String) -> Result<Vec<u8>, String>;

    /// Load a private key in a format compatible with russh.
    async fn get_russh_key(&self, id: &str) -> Result<russh::keys::PrivateKey, String>;
}

pub struct StandardKeyManager {
    base_path: std::path::PathBuf,
}

impl StandardKeyManager {
    pub fn new(app_handle: &tauri::AppHandle) -> Self {
        let base_path = app_handle.path().app_local_data_dir().unwrap().join("keys");
        std::fs::create_dir_all(&base_path).ok();
        Self { base_path }
    }

    pub fn new_with_path(base_path: std::path::PathBuf) -> Self {
        std::fs::create_dir_all(&base_path).ok();
        Self { base_path }
    }

    /// Load or generate the master passphrase used to encrypt keys at rest.
    fn master_passphrase(&self) -> Result<Zeroizing<String>, String> {
        let passphrase_path = self.base_path.join("master.key");
        if passphrase_path.exists() {
            let raw = std::fs::read_to_string(&passphrase_path).map_err(|e| e.to_string())?;
            Ok(Zeroizing::new(raw.trim().to_string()))
        } else {
            let passphrase = uuid::Uuid::new_v4().to_string();
            std::fs::write(&passphrase_path, &passphrase).map_err(|e| e.to_string())?;
            // Restrict file permissions on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&passphrase_path, std::fs::Permissions::from_mode(0o600)).ok();
            }
            Ok(Zeroizing::new(passphrase))
        }
    }

    /// Decrypt a private key from an OpenSSH-format file using the master passphrase.
    fn load_decrypted_key(&self, id: &str) -> Result<ssh_key::PrivateKey, String> {
        let priv_path = self.base_path.join(format!("{}.priv", id));
        let key_str = Zeroizing::new(
            std::fs::read_to_string(&priv_path)
                .map_err(|e| format!("Failed to read key {}: {}", id, e))?
        );
        let encrypted_key = ssh_key::PrivateKey::from_openssh(&*key_str)
            .map_err(|e| format!("Failed to parse key: {}", e))?;

        if encrypted_key.is_encrypted() {
            let passphrase = self.master_passphrase()?;
            encrypted_key.decrypt(passphrase.as_bytes())
                .map_err(|e| format!("Failed to decrypt key: {}", e))
        } else {
            Ok(encrypted_key)
        }
    }
}

#[async_trait]
impl KeyManager for StandardKeyManager {
    async fn generate_key(&self, label: String) -> Result<KeyIdentity, String> {
        let id = uuid::Uuid::new_v4().to_string();
        let key = ssh_key::PrivateKey::random(&mut rand::rngs::OsRng, ssh_key::Algorithm::Ed25519)
            .map_err(|e| format!("Key generation failed: {}", e))?;

        let priv_path = self.base_path.join(format!("{}.priv", id));
        let meta_path = self.base_path.join(format!("{}.json", id));

        // Encrypt the private key at rest with the master passphrase
        let passphrase = self.master_passphrase()?;
        let encrypted_key = key.encrypt(&mut rand::rngs::OsRng, passphrase.as_bytes())
            .map_err(|e| format!("Failed to encrypt private key: {}", e))?;
        let pem = encrypted_key.to_openssh(ssh_key::LineEnding::LF)
            .map_err(|e| format!("Failed to encode private key: {}", e))?;
        std::fs::write(&priv_path, pem.as_bytes()).map_err(|e| e.to_string())?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&priv_path, std::fs::Permissions::from_mode(0o600)).ok();
        }

        let identity = KeyIdentity {
            id: id.clone(),
            label,
            public_key: key.public_key().public_key_base64(),
            created_at: chrono::Utc::now().timestamp(),
        };

        let meta_json = serde_json::to_string(&identity).map_err(|e| e.to_string())?;
        std::fs::write(meta_path, meta_json).map_err(|e| e.to_string())?;

        Ok(identity)
    }

    async fn list_identities(&self) -> Result<Vec<KeyIdentity>, String> {
        let mut identities = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.base_path) {
            for entry in entries.flatten() {
                if entry.path().extension().map_or(false, |ext| ext == "json") {
                    if let Ok(content) = std::fs::read_to_string(entry.path()) {
                        if let Ok(identity) = serde_json::from_str::<KeyIdentity>(&content) {
                            identities.push(identity);
                        }
                    }
                }
            }
        }
        Ok(identities)
    }

    async fn delete_identity(&self, id: String) -> Result<(), String> {
        let priv_path = self.base_path.join(format!("{}.priv", id));
        let meta_path = self.base_path.join(format!("{}.json", id));
        std::fs::remove_file(priv_path).ok();
        std::fs::remove_file(meta_path).ok();
        Ok(())
    }

    async fn sign(&self, id: String, data: &[u8], _reason: String) -> Result<Vec<u8>, String> {
        let key = self.load_decrypted_key(&id)?;

        match key.key_data() {
            ssh_key::private::KeypairData::Ed25519(kp) => {
                use ed25519_dalek::Signer;
                let signing_key: ed25519_dalek::SigningKey = kp.try_into()
                    .map_err(|e: ssh_key::Error| format!("Key conversion failed: {}", e))?;
                let signature = signing_key.sign(data);
                Ok(signature.to_bytes().to_vec())
            }
            _ => Err("Only ed25519 keys are supported for signing".to_string()),
        }
    }

    async fn get_russh_key(&self, id: &str) -> Result<russh::keys::PrivateKey, String> {
        let priv_path = self.base_path.join(format!("{}.priv", id));
        let key_str = Zeroizing::new(
            std::fs::read_to_string(&priv_path)
                .map_err(|e| format!("Failed to read key file: {}", e))?
        );
        let passphrase = self.master_passphrase()?;
        russh::keys::decode_secret_key(&key_str, Some(passphrase.as_str()))
            .map_err(|e| format!("Failed to decode key: {}", e))
    }
}

/// Request sent from Rust to frontend to trigger biometric authentication.
#[derive(Debug, Serialize, Clone)]
pub struct BiometricRequest {
    pub request_id: String,
    pub key_id: String,
    pub title: String,
    pub subtitle: String,
}

/// Response sent from frontend back to Rust after biometric authentication.
#[derive(Debug, Deserialize, Clone)]
pub struct BiometricResponse {
    pub request_id: String,
    pub success: bool,
    pub error: Option<String>,
}

/// Global registry of pending biometric requests, keyed by request_id.
/// When a biometric request completes, the frontend emits a "biometric-result" event
/// and we resolve the corresponding oneshot sender.
static BIOMETRIC_PENDING: std::sync::LazyLock<
    std::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<BiometricResponse>>>
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));

/// Register a pending biometric request and return a receiver to await the result.
pub fn register_biometric_request(request_id: String) -> tokio::sync::oneshot::Receiver<BiometricResponse> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    BIOMETRIC_PENDING.lock().unwrap().insert(request_id, tx);
    rx
}

/// Complete a pending biometric request (called from the Tauri event handler).
pub fn complete_biometric_request(response: BiometricResponse) {
    if let Some(tx) = BIOMETRIC_PENDING.lock().unwrap().remove(&response.request_id) {
        let _ = tx.send(response);
    }
}

#[cfg(target_os = "android")]
pub struct AndroidKeyManager {
    app_handle: tauri::AppHandle,
    base_manager: StandardKeyManager,
}

#[cfg(target_os = "android")]
impl AndroidKeyManager {
    pub fn new(app_handle: &tauri::AppHandle) -> Self {
        Self {
            app_handle: app_handle.clone(),
            base_manager: StandardKeyManager::new(app_handle),
        }
    }

    /// Request biometric authentication via the frontend and wait for the result.
    async fn request_biometric(&self, key_id: &str, reason: &str) -> Result<(), String> {
        use tauri::Emitter;

        let request_id = uuid::Uuid::new_v4().to_string();
        let rx = register_biometric_request(request_id.clone());

        let request = BiometricRequest {
            request_id: request_id.clone(),
            key_id: key_id.to_string(),
            title: "Unlock SSH Key".to_string(),
            subtitle: reason.to_string(),
        };

        self.app_handle.emit("biometric-request", &request)
            .map_err(|e| format!("Failed to emit biometric request: {}", e))?;

        let response = rx.await
            .map_err(|_| "Biometric request was cancelled".to_string())?;

        if response.success {
            Ok(())
        } else {
            Err(response.error.unwrap_or_else(|| "Biometric authentication failed".to_string()))
        }
    }
}

#[cfg(target_os = "android")]
#[async_trait]
impl KeyManager for AndroidKeyManager {
    async fn generate_key(&self, label: String) -> Result<KeyIdentity, String> {
        let identity = self.base_manager.generate_key(label).await?;
        Ok(identity)
    }

    async fn list_identities(&self) -> Result<Vec<KeyIdentity>, String> {
        self.base_manager.list_identities().await
    }

    async fn delete_identity(&self, id: String) -> Result<(), String> {
        self.base_manager.delete_identity(id).await
    }

    async fn sign(&self, id: String, data: &[u8], reason: String) -> Result<Vec<u8>, String> {
        info!("Requesting biometric authentication for signing with key: {}", id);

        // Trigger biometric prompt and wait for result
        self.request_biometric(&id, &reason).await?;

        // Biometric succeeded — decrypt and sign using the base manager
        self.base_manager.sign(id, data, reason).await
    }

    async fn get_russh_key(&self, id: &str) -> Result<russh::keys::PrivateKey, String> {
        // Trigger biometric prompt before providing the key
        self.request_biometric(id, "Connect to SSH").await?;
        self.base_manager.get_russh_key(id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_manager(dir: &std::path::Path) -> StandardKeyManager {
        StandardKeyManager::new_with_path(dir.join("keys"))
    }

    #[tokio::test]
    async fn test_generate_key_creates_files() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = make_manager(tmp.path());

        let identity = mgr.generate_key("test-key".to_string()).await.unwrap();

        assert_eq!(identity.label, "test-key");
        assert!(!identity.id.is_empty());
        assert!(!identity.public_key.is_empty());
        assert!(identity.created_at > 0);

        let keys_dir = tmp.path().join("keys");
        let priv_path = keys_dir.join(format!("{}.priv", identity.id));
        let meta_path = keys_dir.join(format!("{}.json", identity.id));
        assert!(priv_path.exists(), ".priv file should exist");
        assert!(meta_path.exists(), ".json file should exist");
    }

    #[tokio::test]
    async fn test_generate_key_produces_valid_ed25519() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = make_manager(tmp.path());

        let identity = mgr.generate_key("ed25519-check".to_string()).await.unwrap();

        // Public key should be base64-decodable
        assert!(!identity.public_key.is_empty());

        // Private key file is encrypted at rest
        let priv_path = tmp.path().join("keys").join(format!("{}.priv", identity.id));
        let key_str = std::fs::read_to_string(&priv_path).unwrap();
        let encrypted = ssh_key::PrivateKey::from_openssh(&key_str).unwrap();
        assert!(encrypted.is_encrypted(), "Key should be encrypted at rest");

        // Should be decodable with the master passphrase
        let passphrase = mgr.master_passphrase().unwrap();
        let decoded = russh::keys::decode_secret_key(&key_str, Some(&passphrase));
        assert!(decoded.is_ok(), "Private key should be decodable with passphrase");
    }

    #[tokio::test]
    async fn test_list_identities_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = make_manager(tmp.path());

        let list = mgr.list_identities().await.unwrap();
        assert!(list.is_empty());
    }

    #[tokio::test]
    async fn test_list_identities_returns_generated_keys() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = make_manager(tmp.path());

        mgr.generate_key("key-a".to_string()).await.unwrap();
        mgr.generate_key("key-b".to_string()).await.unwrap();

        let list = mgr.list_identities().await.unwrap();
        assert_eq!(list.len(), 2);

        let labels: Vec<&str> = list.iter().map(|k| k.label.as_str()).collect();
        assert!(labels.contains(&"key-a"));
        assert!(labels.contains(&"key-b"));
    }

    #[tokio::test]
    async fn test_list_identities_skips_corrupt_json() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = make_manager(tmp.path());

        mgr.generate_key("good-key".to_string()).await.unwrap();

        // Write a corrupt JSON file
        let keys_dir = tmp.path().join("keys");
        std::fs::write(keys_dir.join("corrupt.json"), "not valid json").unwrap();

        let list = mgr.list_identities().await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].label, "good-key");
    }

    #[tokio::test]
    async fn test_delete_identity_removes_files() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = make_manager(tmp.path());

        let identity = mgr.generate_key("to-delete".to_string()).await.unwrap();
        let keys_dir = tmp.path().join("keys");
        assert!(keys_dir.join(format!("{}.priv", identity.id)).exists());
        assert!(keys_dir.join(format!("{}.json", identity.id)).exists());

        mgr.delete_identity(identity.id.clone()).await.unwrap();

        assert!(!keys_dir.join(format!("{}.priv", identity.id)).exists());
        assert!(!keys_dir.join(format!("{}.json", identity.id)).exists());
    }

    #[tokio::test]
    async fn test_delete_identity_nonexistent_is_ok() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = make_manager(tmp.path());

        // Should not error when deleting a key that doesn't exist
        let result = mgr.delete_identity("nonexistent-id".to_string()).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_key_identity_serde_roundtrip() {
        let identity = KeyIdentity {
            id: "test-uuid".to_string(),
            label: "my label".to_string(),
            public_key: "AAAAC3NzaC1lZDI1NTE5".to_string(),
            created_at: 1700000000,
        };

        let json = serde_json::to_string(&identity).unwrap();
        let deserialized: KeyIdentity = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, identity.id);
        assert_eq!(deserialized.label, identity.label);
        assert_eq!(deserialized.public_key, identity.public_key);
        assert_eq!(deserialized.created_at, identity.created_at);
    }

    #[tokio::test]
    async fn test_sign_produces_valid_signature() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = make_manager(tmp.path());

        let identity = mgr.generate_key("sign-key".to_string()).await.unwrap();
        let data = b"hello world";
        let sig_bytes = mgr.sign(identity.id.clone(), data, "test".to_string()).await.unwrap();

        // ed25519 signatures are 64 bytes
        assert_eq!(sig_bytes.len(), 64);

        // Verify the signature using the decrypted key's public component
        let decrypted = mgr.load_decrypted_key(&identity.id).unwrap();
        if let ssh_key::private::KeypairData::Ed25519(kp) = decrypted.key_data() {
            use ed25519_dalek::Verifier;
            let verifying_key = ed25519_dalek::VerifyingKey::from_bytes(kp.public.as_ref()).unwrap();
            let signature = ed25519_dalek::Signature::from_bytes(&sig_bytes.try_into().unwrap());
            assert!(verifying_key.verify(data, &signature).is_ok());
        } else {
            panic!("Expected Ed25519 key");
        }
    }

    #[tokio::test]
    async fn test_sign_nonexistent_key_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = make_manager(tmp.path());

        let result = mgr.sign("no-such-key".to_string(), b"data", "test".to_string()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to read key"));
    }

    #[tokio::test]
    async fn test_ssh_config_key_auth_serde() {
        let config = crate::ssh::SshConfig {
            host: "example.com".to_string(),
            port: 22,
            username: "user".to_string(),
            auth_method: crate::ssh::AuthMethod::Key,
            password: None,
            private_key_path: None,
            private_key_passphrase: None,
            key_id: Some("my-key-id".to_string()),
            session_name: "main".to_string(),
            project_root: None,
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: crate::ssh::SshConfig = serde_json::from_str(&json).unwrap();

        assert!(matches!(deserialized.auth_method, crate::ssh::AuthMethod::Key));
        assert_eq!(deserialized.key_id, Some("my-key-id".to_string()));
    }

    #[tokio::test]
    async fn test_ssh_config_private_key_auth_serde() {
        let config = crate::ssh::SshConfig {
            host: "example.com".to_string(),
            port: 22,
            username: "user".to_string(),
            auth_method: crate::ssh::AuthMethod::PrivateKey,
            password: None,
            private_key_path: Some("/home/user/.ssh/id_ed25519".to_string()),
            private_key_passphrase: Some("my-passphrase".to_string()),
            key_id: None,
            session_name: "main".to_string(),
            project_root: None,
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: crate::ssh::SshConfig = serde_json::from_str(&json).unwrap();

        assert!(matches!(deserialized.auth_method, crate::ssh::AuthMethod::PrivateKey));
        assert_eq!(deserialized.private_key_path, Some("/home/user/.ssh/id_ed25519".to_string()));
        assert_eq!(deserialized.private_key_passphrase, Some("my-passphrase".to_string()));
    }

    #[tokio::test]
    async fn test_private_key_file_decode_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = make_manager(tmp.path());

        // Generate a key using the manager
        let identity = mgr.generate_key("roundtrip-key".to_string()).await.unwrap();
        let priv_path = tmp.path().join("keys").join(format!("{}.priv", identity.id));

        // Verify the file can be decoded by russh with the passphrase (same path SSH auth uses)
        let key_str = std::fs::read_to_string(&priv_path).unwrap();
        let passphrase = mgr.master_passphrase().unwrap();
        let key = russh::keys::decode_secret_key(&key_str, Some(&passphrase));
        assert!(key.is_ok(), "Generated key should be decodable by russh with passphrase: {:?}", key.err());

        // Without passphrase should fail
        let no_pass = russh::keys::decode_secret_key(&key_str, None);
        assert!(no_pass.is_err(), "Encrypted key should not decode without passphrase");
    }

    #[tokio::test]
    async fn test_master_passphrase_created_and_reused() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = make_manager(tmp.path());

        let pass1 = mgr.master_passphrase().unwrap();
        let pass2 = mgr.master_passphrase().unwrap();
        assert_eq!(*pass1, *pass2, "Same passphrase should be returned on repeated calls");
        assert!(!pass1.is_empty());

        // Verify the file exists
        assert!(tmp.path().join("keys").join("master.key").exists());
    }

    #[tokio::test]
    async fn test_load_decrypted_key_works() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = make_manager(tmp.path());

        let identity = mgr.generate_key("decrypt-test".to_string()).await.unwrap();
        let key = mgr.load_decrypted_key(&identity.id).unwrap();

        assert!(!key.is_encrypted(), "Decrypted key should not be encrypted");
        assert!(matches!(key.key_data(), ssh_key::private::KeypairData::Ed25519(_)));
    }

    #[tokio::test]
    async fn test_biometric_bridge_success() {
        let request_id = "test-req-1".to_string();
        let rx = register_biometric_request(request_id.clone());

        // Simulate frontend completing the biometric request
        complete_biometric_request(BiometricResponse {
            request_id: request_id.clone(),
            success: true,
            error: None,
        });

        let result = rx.await.unwrap();
        assert!(result.success);
        assert!(result.error.is_none());
    }

    #[tokio::test]
    async fn test_biometric_bridge_failure() {
        let request_id = "test-req-2".to_string();
        let rx = register_biometric_request(request_id.clone());

        complete_biometric_request(BiometricResponse {
            request_id: request_id.clone(),
            success: false,
            error: Some("User cancelled".to_string()),
        });

        let result = rx.await.unwrap();
        assert!(!result.success);
        assert_eq!(result.error, Some("User cancelled".to_string()));
    }

    #[tokio::test]
    async fn test_biometric_bridge_unknown_request_ignored() {
        // Completing a request that was never registered should not panic
        complete_biometric_request(BiometricResponse {
            request_id: "nonexistent".to_string(),
            success: true,
            error: None,
        });
    }

    #[test]
    fn test_biometric_request_serde() {
        let req = BiometricRequest {
            request_id: "r1".to_string(),
            key_id: "k1".to_string(),
            title: "Unlock SSH Key".to_string(),
            subtitle: "Connect to server".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("request_id"));
        assert!(json.contains("key_id"));
    }

    #[test]
    fn test_biometric_response_serde() {
        let json = r#"{"request_id":"r1","success":true,"error":null}"#;
        let resp: BiometricResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.request_id, "r1");
        assert!(resp.success);
        assert!(resp.error.is_none());
    }
}
