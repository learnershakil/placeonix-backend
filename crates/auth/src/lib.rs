use std::{error::Error, fmt};

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use sha2::{Digest, Sha256};
use uuid::Uuid;

const REFRESH_TOKEN_BYTES: usize = 32;

#[derive(Clone, Default)]
pub struct PasswordService {
    argon2: Argon2<'static>,
}

impl PasswordService {
    pub fn hash_password(&self, password: &str) -> Result<String, AuthError> {
        validate_password(password)?;
        let salt = SaltString::generate(&mut OsRng);
        self.argon2
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|_| AuthError::PasswordHashFailed)
    }

    pub fn verify_password(&self, password: &str, password_hash: &str) -> Result<bool, AuthError> {
        let parsed_hash =
            PasswordHash::new(password_hash).map_err(|_| AuthError::PasswordHashInvalid)?;
        Ok(self
            .argon2
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok())
    }
}

#[derive(Clone, Default)]
pub struct RefreshTokenService;

impl RefreshTokenService {
    pub fn issue(&self) -> RefreshToken {
        RefreshToken::generate()
    }

    pub fn rotate(&self, previous_token_id: Uuid) -> RefreshTokenRotation {
        let token = self.issue();
        RefreshTokenRotation {
            previous_token_id,
            token_hash: token.hash(),
            token,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshToken {
    secret: String,
}

impl RefreshToken {
    pub fn generate() -> Self {
        let bytes: [u8; REFRESH_TOKEN_BYTES] = rand_bytes();
        Self {
            secret: URL_SAFE_NO_PAD.encode(bytes),
        }
    }

    pub fn from_secret(secret: impl Into<String>) -> Result<Self, AuthError> {
        let secret = secret.into();
        if secret.trim().is_empty() {
            return Err(AuthError::RefreshTokenInvalid);
        }
        Ok(Self { secret })
    }

    pub fn expose(&self) -> &str {
        &self.secret
    }

    pub fn hash(&self) -> String {
        hash_secret(&self.secret)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshTokenRotation {
    pub previous_token_id: Uuid,
    pub token: RefreshToken,
    pub token_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    PasswordTooShort,
    PasswordHashFailed,
    PasswordHashInvalid,
    RefreshTokenInvalid,
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PasswordTooShort => f.write_str("password must be at least 12 characters"),
            Self::PasswordHashFailed => f.write_str("failed to hash password"),
            Self::PasswordHashInvalid => f.write_str("stored password hash is invalid"),
            Self::RefreshTokenInvalid => f.write_str("refresh token is invalid"),
        }
    }
}

impl Error for AuthError {}

fn validate_password(password: &str) -> Result<(), AuthError> {
    if password.chars().count() < 12 {
        Err(AuthError::PasswordTooShort)
    } else {
        Ok(())
    }
}

fn hash_secret(secret: &str) -> String {
    let digest = Sha256::digest(secret.as_bytes());
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

fn rand_bytes() -> [u8; REFRESH_TOKEN_BYTES] {
    use argon2::password_hash::rand_core::RngCore;

    let mut bytes = [0; REFRESH_TOKEN_BYTES];
    OsRng.fill_bytes(&mut bytes);
    bytes
}

#[cfg(test)]
mod tests {
    use super::{hash_secret, PasswordService, RefreshToken, RefreshTokenService};

    #[test]
    fn hashes_and_verifies_passwords() {
        let service = PasswordService::default();
        let hash = service
            .hash_password("correct horse battery staple")
            .expect("password hashes");

        assert!(service
            .verify_password("correct horse battery staple", &hash)
            .expect("password verifies"));
        assert!(!service
            .verify_password("wrong horse battery staple", &hash)
            .expect("password verification returns false"));
    }

    #[test]
    fn rejects_short_passwords() {
        let error = PasswordService::default()
            .hash_password("too-short")
            .expect_err("short password rejected");

        assert_eq!(error.to_string(), "password must be at least 12 characters");
    }

    #[test]
    fn hashes_refresh_tokens_stably_without_storing_secret() {
        let token = RefreshToken::from_secret("refresh-secret").expect("token accepted");

        assert_eq!(token.hash(), hash_secret("refresh-secret"));
        assert_ne!(token.hash(), token.expose());
    }

    #[test]
    fn rotates_refresh_tokens_to_new_secret() {
        let rotation =
            RefreshTokenService.rotate(uuid::Uuid::from_u128(0x12345678123456781234567812345678));

        assert_eq!(
            rotation.previous_token_id,
            uuid::Uuid::from_u128(0x12345678123456781234567812345678)
        );
        assert_eq!(rotation.token_hash, rotation.token.hash());
    }
}
