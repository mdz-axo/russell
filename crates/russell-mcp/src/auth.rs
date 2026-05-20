// SPDX-License-Identifier: MIT OR Apache-2.0
//! Token provider for hKask MCP authentication.
//!
//! Provides automatic token refresh from a file-based token store.
//! hKask's `stack-keystore` or a rotation script updates the token file;
//! Russell polls for changes before expiry.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::error::{McpError, Result};

/// Default token refresh buffer (refresh this far before expiry).
const REFRESH_BUFFER: Duration = Duration::from_secs(86400); // 24 hours

/// Token file structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenFile {
    /// The bearer token.
    pub token: String,
    /// When the token was issued (ISO 8601).
    pub issued_at: String,
    /// When the token expires (ISO 8601).
    pub expires_at: String,
    /// Token scope (e.g., "user", "admin").
    #[serde(default)]
    pub scope: String,
    /// Principal name (e.g., "russell").
    #[serde(default)]
    pub principal: String,
}

/// Token provider trait.
#[async_trait::async_trait]
pub trait TokenProvider: Send + Sync {
    /// Get the current token, refreshing if necessary.
    async fn get_token(&self) -> Result<String>;

    /// Check if the token is valid (not expired).
    async fn is_valid(&self) -> bool;
}

/// Static token provider — uses a fixed token from environment.
///
/// This is the backward-compatible provider for users who haven't
/// set up token rotation. Token does not refresh automatically.
pub struct StaticTokenProvider {
    token: String,
}

impl StaticTokenProvider {
    /// Create a new static token provider.
    pub fn new(token: String) -> Self {
        Self { token }
    }

    /// Create from environment variable.
    pub fn from_env(var_name: &str) -> Option<Self> {
        std::env::var(var_name)
            .ok()
            .filter(|s| !s.is_empty())
            .map(Self::new)
    }
}

#[async_trait::async_trait]
impl TokenProvider for StaticTokenProvider {
    async fn get_token(&self) -> Result<String> {
        Ok(self.token.clone())
    }

    async fn is_valid(&self) -> bool {
        !self.token.is_empty()
    }
}

/// File-based token provider with automatic refresh.
///
/// Reads token from a JSON file and caches it in memory. Before each
/// use, checks if the cached token is near expiry and refreshes if needed.
///
/// The token file is expected to be updated by an external process
/// (e.g., hKask's `stack-keystore` or a rotation script).
pub struct FileTokenProvider {
    token_path: PathBuf,
    cached: RwLock<Option<CachedToken>>,
}

struct CachedToken {
    token: String,
    expires_at: chrono::DateTime<chrono::Utc>,
}

impl FileTokenProvider {
    /// Create a new file-based token provider.
    ///
    /// # Arguments
    /// * `token_path` — Path to the token JSON file
    pub fn new<P: AsRef<Path>>(token_path: P) -> Self {
        Self {
            token_path: token_path.as_ref().to_path_buf(),
            cached: RwLock::new(None),
        }
    }

    /// Default token path: `~/.local/state/hkask/mcp-token.json`
    pub fn default_path() -> Result<PathBuf> {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map_err(|_| McpError::Config("Cannot determine home directory".into()))?;

        Ok(PathBuf::from(home).join(".local/state/hkask/mcp-token.json"))
    }

    /// Create with default path.
    pub fn with_default_path() -> Result<Self> {
        Ok(Self::new(Self::default_path()?))
    }

    /// Read and parse the token file.
    fn read_token_file(&self) -> Result<TokenFile> {
        if !self.token_path.exists() {
            return Err(McpError::Config(format!(
                "token file not found: {}",
                self.token_path.display()
            )));
        }

        let content = std::fs::read_to_string(&self.token_path)
            .map_err(|e| McpError::Config(format!("failed to read token file: {e}")))?;

        let token_file: TokenFile = serde_json::from_str(&content)
            .map_err(|e| McpError::Config(format!("invalid token file format: {e}")))?;

        Ok(token_file)
    }

    /// Parse ISO 8601 timestamp to DateTime<Utc>.
    fn parse_timestamp(ts: &str) -> Result<chrono::DateTime<chrono::Utc>> {
        chrono::DateTime::parse_from_rfc3339(ts)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|e| McpError::Config(format!("invalid timestamp '{ts}': {e}")))
    }

    /// Check if cached token needs refresh.
    fn needs_refresh(cached: &CachedToken) -> bool {
        let now = chrono::Utc::now();
        let expiry_with_buffer = cached.expires_at - REFRESH_BUFFER;
        now >= expiry_with_buffer
    }
}

#[async_trait::async_trait]
impl TokenProvider for FileTokenProvider {
    async fn get_token(&self) -> Result<String> {
        // Check cache first.
        {
            let cached = self.cached.read().await;
            if let Some(ref c) = *cached {
                if !Self::needs_refresh(c) {
                    debug!("using cached token (not near expiry)");
                    return Ok(c.token.clone());
                }
            }
        }

        // Need to refresh — read from file.
        let token_file = self.read_token_file()?;
        let expires_at = Self::parse_timestamp(&token_file.expires_at)?;

        // Check if token is already expired.
        if chrono::Utc::now() >= expires_at {
            warn!("token file contains expired token");
            return Err(McpError::Unauthenticated);
        }

        let cached = CachedToken {
            token: token_file.token.clone(),
            expires_at,
        };

        debug!(
            principal = %token_file.principal,
            scope = %token_file.scope,
            expires = %token_file.expires_at,
            "loaded token from file"
        );

        // Update cache.
        let mut write_guard = self.cached.write().await;
        *write_guard = Some(cached);

        Ok(token_file.token)
    }

    async fn is_valid(&self) -> bool {
        let cached = self.cached.read().await;
        match &*cached {
            Some(c) => !Self::needs_refresh(c),
            None => false,
        }
    }
}

/// Token provider that tries file-based first, falls back to env var.
pub struct ChainedTokenProvider {
    file: Option<FileTokenProvider>,
    fallback: Option<StaticTokenProvider>,
}

impl ChainedTokenProvider {
    /// Create a new chained provider.
    pub fn new(file_path: Option<PathBuf>) -> Result<Self> {
        let file = file_path
            .map(FileTokenProvider::new)
            .or_else(|| FileTokenProvider::with_default_path().ok());

        let fallback = StaticTokenProvider::from_env("KASK_MCP_TOKEN");

        Ok(Self { file, fallback })
    }
}

#[async_trait::async_trait]
impl TokenProvider for ChainedTokenProvider {
    async fn get_token(&self) -> Result<String> {
        // Try file-based first.
        if let Some(ref file) = self.file {
            match file.get_token().await {
                Ok(token) => return Ok(token),
                Err(e) => {
                    debug!(error = %e, "file token provider failed, trying fallback");
                }
            }
        }

        // Fall back to env var.
        if let Some(ref fallback) = self.fallback {
            return fallback.get_token().await;
        }

        Err(McpError::Unauthenticated)
    }

    async fn is_valid(&self) -> bool {
        if let Some(ref file) = self.file {
            if file.is_valid().await {
                return true;
            }
        }

        if let Some(ref fallback) = self.fallback {
            return fallback.is_valid().await;
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn make_token_file(expires_in_hours: i64) -> (NamedTempFile, TokenFile) {
        let file = NamedTempFile::new().unwrap();
        let now = chrono::Utc::now();
        let expires = now + chrono::Duration::hours(expires_in_hours);

        let token_data = TokenFile {
            token: "test-token-123".into(),
            issued_at: now.to_rfc3339(),
            expires_at: expires.to_rfc3339(),
            scope: "user".into(),
            principal: "russell".into(),
        };

        let content = serde_json::to_string_pretty(&token_data).unwrap();
        file.as_file().write_all(content.as_bytes()).unwrap();

        (file, token_data)
    }

    #[test]
    fn static_provider_returns_token() {
        let provider = StaticTokenProvider::new("test-token".into());
        let rt = tokio::runtime::Runtime::new().unwrap();
        let token = rt.block_on(provider.get_token()).unwrap();
        assert_eq!(token, "test-token");
    }

    #[tokio::test]
    async fn file_provider_reads_token() {
        let (temp_file, _) = make_token_file(24);
        let provider = FileTokenProvider::new(temp_file.path());

        let token = provider.get_token().await.unwrap();
        assert_eq!(token, "test-token-123");
    }

    #[tokio::test]
    async fn file_provider_detects_expiry() {
        let (temp_file, _) = make_token_file(-1); // Expired 1 hour ago
        let provider = FileTokenProvider::new(temp_file.path());

        let result = provider.get_token().await;
        assert!(matches!(result, Err(McpError::Unauthenticated)));
    }

    #[tokio::test]
    async fn file_provider_refreshes_near_expiry() {
        let (temp_file, _) = make_token_file(12); // Expires in 12 hours (< 24h buffer)
        let provider = FileTokenProvider::new(temp_file.path());

        // First call loads token.
        let _token1 = provider.get_token().await.unwrap();

        // Modify the token file by writing a new file.
        let new_token_data = TokenFile {
            token: "new-token-456".into(),
            issued_at: chrono::Utc::now().to_rfc3339(),
            expires_at: (chrono::Utc::now() + chrono::Duration::hours(48)).to_rfc3339(),
            scope: "user".into(),
            principal: "russell".into(),
        };
        let content = serde_json::to_string_pretty(&new_token_data).unwrap();
        std::fs::write(temp_file.path(), content).unwrap();

        // Second call should refresh (within buffer window).
        let token2 = provider.get_token().await.unwrap();

        assert_eq!(token2, "new-token-456");
    }
}
