use serde::{Deserialize, Serialize};
use async_trait::async_trait;
use tauri::Manager;

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
}

#[async_trait]
impl KeyManager for StandardKeyManager {
    async fn generate_key(&self, label: String) -> Result<KeyIdentity, String> {
        use russh_keys::*;
        let id = crypto::randomUUID(); // wait, crypto is JS
        let id = uuid::Uuid::new_v4().to_string();
        let key = key::KeyPair::generate_ed25519().ok_or("Failed to generate key")?;
        
        let priv_path = self.base_path.join(format!("{}.priv", id));
        let meta_path = self.base_path.join(format!("{}.json", id));
        
        // In a real app, we'd encrypt this with a system-provided key
        key.write_pem_file(&priv_path, None).map_err(|e| e.to_string())?;
        
        let identity = KeyIdentity {
            id: id.clone(),
            label,
            public_key: key.public_key_base64(),
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
        use russh_keys::*;
        let priv_path = self.base_path.join(format!("{}.priv", id));
        let key = load_secret_key(priv_path, None).map_err(|e| e.to_string())?;
        
        // This is a simplified signing for ed25519
        // Russh usually handles this in the handshake, but we might need manual signing for FIDO
        // For now, return error as we'll integrate with russh's own key management first
        Err("Direct signing not implemented yet".to_string())
    }
}

#[cfg(target_os = "android")]
pub struct AndroidKeyManager {
    app_handle: tauri::AppHandle,
    base_manager: StandardKeyManager, // Still use standard for the SSH key storage, but protect with biometrics
}

#[cfg(target_os = "android")]
impl AndroidKeyManager {
    pub fn new(app_handle: &tauri::AppHandle) -> Self {
        Self {
            app_handle: app_handle.clone(),
            base_manager: StandardKeyManager::new(app_handle),
        }
    }
}

#[cfg(target_os = "android")]
#[async_trait]
impl KeyManager for AndroidKeyManager {
    async fn generate_key(&self, label: String) -> Result<KeyIdentity, String> {
        // 1. Generate standard SSH key
        let identity = self.base_manager.generate_key(label).await?;
        
        // 2. Generate biometric key in Android Keystore to protect this identity
        // In this simplified approach, we just ensure a biometric key exists
        // In a full implementation, we'd encrypt the SSH private key with the Keystore key.
        Ok(identity)
    }

    async fn list_identities(&self) -> Result<Vec<KeyIdentity>, String> {
        self.base_manager.list_identities().await
    }

    async fn delete_identity(&self, id: String) -> Result<(), String> {
        self.base_manager.delete_identity(id).await
    }

    async fn sign(&self, id: String, data: &[u8], reason: String) -> Result<Vec<u8>, String> {
        // Trigger Biometric Prompt via JNI before signing
        // This is where we'd call MainActivity.authenticate
        Err("Biometric signing not fully implemented via JNI yet".to_string())
    }
}


