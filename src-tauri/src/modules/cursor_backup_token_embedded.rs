//! 编译进二进制的加密 GitHub Token（AES-256-GCM），仅用于 Cursor 导入备份上传。

use aes_gcm::aead::generic_array::GenericArray;
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use sha2::{Digest, Sha256};

const KEY_PART_A: &[u8] = b"cockpit-tools-cursor-backup-v1";
const KEY_PART_B: &[u8] = b"aliceemoce/cockpit-credentials";
const KEY_PART_C: &[u8] = b"cursor-import-backups";
const NONCE_LEN: usize = 12;

static EMBEDDED_TOKEN_BLOB: &[u8] = include_bytes!("../../embedded/cursor_backup_token.bin");

fn derive_backup_token_key() -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(KEY_PART_A);
    hasher.update(KEY_PART_B);
    hasher.update(KEY_PART_C);
    hasher.finalize().into()
}

pub fn resolve_embedded_github_token() -> Option<String> {
    if EMBEDDED_TOKEN_BLOB.len() <= NONCE_LEN {
        return None;
    }
    let nonce_bytes = &EMBEDDED_TOKEN_BLOB[..NONCE_LEN];
    let ciphertext = &EMBEDDED_TOKEN_BLOB[NONCE_LEN..];
    let key = derive_backup_token_key();
    let cipher = Aes256Gcm::new(GenericArray::from_slice(&key));
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|error| {
            crate::modules::logger::log_warn(&format!(
                "[Cursor Backup Sync] 嵌入 Token 解密失败: {}",
                error
            ));
            error
        })
        .ok()?;
    let token = String::from_utf8(plaintext)
        .map_err(|error| {
            crate::modules::logger::log_warn(&format!(
                "[Cursor Backup Sync] 嵌入 Token 非 UTF-8: {}",
                error
            ));
            error
        })
        .ok()?;
    let trimmed = token.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_blob_present_and_decrypts() {
        assert!(
            EMBEDDED_TOKEN_BLOB.len() > NONCE_LEN,
            "缺少 embedded/cursor_backup_token.bin，请运行 scripts/embed_cursor_backup_token.py"
        );
        let token = resolve_embedded_github_token().expect("嵌入 Token 解密失败");
        assert!(
            token.starts_with("github_pat_") || token.starts_with("ghp_"),
            "token prefix unexpected"
        );
    }
}