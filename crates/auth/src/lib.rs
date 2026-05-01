use std::{
    error::Error,
    fmt,
    time::{SystemTime, UNIX_EPOCH},
};

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

const REFRESH_TOKEN_BYTES: usize = 32;
pub const ACCESS_TOKEN_TTL_SECS: i64 = 15 * 60;
pub const REFRESH_TOKEN_TTL_SECS: i64 = 30 * 24 * 60 * 60;

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

#[derive(Clone)]
pub struct AccessTokenService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    issuer: String,
}

impl AccessTokenService {
    pub fn new(secret: impl AsRef<[u8]>, issuer: impl Into<String>) -> Result<Self, AuthError> {
        let secret = secret.as_ref();
        if secret.len() < 32 {
            return Err(AuthError::JwtSecretTooShort);
        }

        Ok(Self {
            encoding_key: EncodingKey::from_secret(secret),
            decoding_key: DecodingKey::from_secret(secret),
            issuer: issuer.into(),
        })
    }

    pub fn issue(&self, input: AccessTokenInput) -> Result<String, AuthError> {
        let now = unix_timestamp();
        let claims = JwtClaims {
            sub: input.user_id.to_string(),
            tenant_id: input.tenant_id.to_string(),
            session_id: input.session_id.to_string(),
            device_id: input.device_id.map(|id| id.to_string()),
            roles: sorted_unique(input.roles),
            permissions: sorted_unique(input.permissions),
            token_version: input.token_version,
            iss: self.issuer.clone(),
            iat: now,
            exp: now + ACCESS_TOKEN_TTL_SECS,
        };

        encode(&Header::new(Algorithm::HS256), &claims, &self.encoding_key)
            .map_err(|_| AuthError::JwtIssueFailed)
    }

    pub fn verify(&self, token: &str) -> Result<JwtClaims, AuthError> {
        if token.trim().is_empty() {
            return Err(AuthError::JwtInvalid);
        }

        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&[self.issuer.as_str()]);
        decode::<JwtClaims>(token, &self.decoding_key, &validation)
            .map(|data| data.claims)
            .map_err(|_| AuthError::JwtInvalid)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessTokenInput {
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub session_id: Uuid,
    pub device_id: Option<Uuid>,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
    pub token_version: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JwtClaims {
    pub sub: String,
    pub tenant_id: String,
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
    pub token_version: i32,
    pub iss: String,
    pub iat: i64,
    pub exp: i64,
}

#[derive(Clone, Default)]
pub struct OtpService;

impl OtpService {
    pub fn issue(&self) -> OtpCode {
        OtpCode::generate()
    }

    pub fn hash(&self, code: &OtpCode) -> String {
        hash_secret(code.expose())
    }

    pub fn verify(&self, candidate: &str, expected_hash: &str) -> bool {
        hash_secret(candidate) == expected_hash
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtpCode {
    code: String,
}

impl OtpCode {
    pub fn generate() -> Self {
        use argon2::password_hash::rand_core::RngCore;

        let value = OsRng.next_u32() % 1_000_000;
        Self {
            code: format!("{value:06}"),
        }
    }

    pub fn from_code(code: impl Into<String>) -> Result<Self, AuthError> {
        let code = code.into();
        if code.len() != 6 || !code.chars().all(|ch| ch.is_ascii_digit()) {
            return Err(AuthError::OtpInvalid);
        }
        Ok(Self { code })
    }

    pub fn expose(&self) -> &str {
        &self.code
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
    OtpInvalid,
    JwtSecretTooShort,
    JwtIssueFailed,
    JwtInvalid,
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PasswordTooShort => f.write_str("password must be at least 12 characters"),
            Self::PasswordHashFailed => f.write_str("failed to hash password"),
            Self::PasswordHashInvalid => f.write_str("stored password hash is invalid"),
            Self::RefreshTokenInvalid => f.write_str("refresh token is invalid"),
            Self::OtpInvalid => f.write_str("otp code is invalid"),
            Self::JwtSecretTooShort => f.write_str("jwt secret must be at least 32 bytes"),
            Self::JwtIssueFailed => f.write_str("failed to issue jwt"),
            Self::JwtInvalid => f.write_str("jwt is invalid"),
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

fn sorted_unique(values: Vec<String>) -> Vec<String> {
    let mut values = values
        .into_iter()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{
        hash_secret, AccessTokenInput, AccessTokenService, OtpCode, OtpService, PasswordService,
        RefreshToken, RefreshTokenService, ACCESS_TOKEN_TTL_SECS,
    };

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

    #[test]
    fn issues_and_hashes_otp_codes() {
        let service = OtpService;
        let code = service.issue();
        let hash = service.hash(&code);

        assert_eq!(code.expose().len(), 6);
        assert!(code.expose().chars().all(|ch| ch.is_ascii_digit()));
        assert!(service.verify(code.expose(), &hash));
        assert!(!service.verify("000000", &hash) || code.expose() == "000000");
    }

    #[test]
    fn rejects_malformed_otp_codes() {
        assert!(OtpCode::from_code("12345").is_err());
        assert!(OtpCode::from_code("abcdef").is_err());
        assert!(OtpCode::from_code("123456").is_ok());
    }

    #[test]
    fn issues_and_verifies_jwt_claims() {
        let service = AccessTokenService::new("01234567890123456789012345678901", "placeonix-api")
            .expect("jwt service builds");
        let tenant_id = uuid::Uuid::from_u128(1);
        let user_id = uuid::Uuid::from_u128(2);
        let session_id = uuid::Uuid::from_u128(3);
        let device_id = uuid::Uuid::from_u128(4);

        let token = service
            .issue(AccessTokenInput {
                tenant_id,
                user_id,
                session_id,
                device_id: Some(device_id),
                roles: vec!["student".to_owned(), "student".to_owned()],
                permissions: vec!["assessments:submit".to_owned(), "courses:read".to_owned()],
                token_version: 7,
            })
            .expect("token issues");
        let claims = service.verify(&token).expect("token verifies");

        assert_eq!(claims.sub, user_id.to_string());
        assert_eq!(claims.tenant_id, tenant_id.to_string());
        assert_eq!(claims.session_id, session_id.to_string());
        assert_eq!(
            claims.device_id.as_deref(),
            Some(device_id.to_string().as_str())
        );
        assert_eq!(claims.roles, ["student"]);
        assert_eq!(
            claims.permissions,
            ["assessments:submit".to_owned(), "courses:read".to_owned()]
        );
        assert_eq!(claims.token_version, 7);
        assert_eq!(claims.exp - claims.iat, ACCESS_TOKEN_TTL_SECS);
    }

    #[test]
    fn rejects_short_jwt_secret() {
        let error = match AccessTokenService::new("short", "placeonix-api") {
            Ok(_) => panic!("short secret is rejected"),
            Err(error) => error,
        };

        assert_eq!(error.to_string(), "jwt secret must be at least 32 bytes");
    }
}
