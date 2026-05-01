use std::sync::Arc;

use api_contracts::AppError;
use axum::{body::Body, extract::State, middleware::Next, response::Response};
use http::{HeaderMap, Request};
use uuid::Uuid;

const USER_ID_HEADER: &str = "x-user-id";
const PERMISSIONS_HEADER: &str = "x-permissions";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Principal {
    user_id: Option<Uuid>,
    permissions: Arc<[String]>,
}

impl Principal {
    pub fn new(user_id: Option<Uuid>, permissions: impl IntoIterator<Item = String>) -> Self {
        let mut permissions = permissions.into_iter().collect::<Vec<_>>();
        permissions.sort();
        permissions.dedup();
        Self {
            user_id,
            permissions: permissions.into(),
        }
    }

    pub fn from_headers(headers: &HeaderMap) -> Result<Option<Self>, AppError> {
        let permissions = match headers.get(PERMISSIONS_HEADER) {
            Some(value) => value
                .to_str()
                .map_err(|_| AppError::bad_request("permissions header is invalid"))?,
            None => return Ok(None),
        };

        let user_id = headers
            .get(USER_ID_HEADER)
            .map(|value| {
                value
                    .to_str()
                    .ok()
                    .and_then(|value| Uuid::parse_str(value).ok())
                    .ok_or_else(|| AppError::bad_request("user id header is invalid"))
            })
            .transpose()?;

        Ok(Some(Self::new(user_id, parse_permissions(permissions))))
    }

    pub fn user_id(&self) -> Option<Uuid> {
        self.user_id
    }

    pub fn permissions(&self) -> &[String] {
        &self.permissions
    }

    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.iter().any(|current| current == permission)
    }
}

#[derive(Debug, Clone)]
pub struct PermissionRequirement {
    required: Arc<[String]>,
}

impl PermissionRequirement {
    pub fn all<const N: usize>(required: [&str; N]) -> Self {
        Self {
            required: required
                .into_iter()
                .map(str::to_owned)
                .collect::<Vec<_>>()
                .into(),
        }
    }

    pub fn required(&self) -> &[String] {
        &self.required
    }

    pub fn is_satisfied_by(&self, principal: &Principal) -> bool {
        self.required
            .iter()
            .all(|permission| principal.has_permission(permission))
    }
}

pub async fn enforce_permissions(
    State(requirement): State<PermissionRequirement>,
    mut request: Request<Body>,
    next: Next,
) -> Result<Response, AppError> {
    let principal = match request.extensions().get::<Principal>().cloned() {
        Some(principal) => principal,
        None => Principal::from_headers(request.headers())?
            .ok_or_else(|| AppError::unauthorized("principal is required"))?,
    };

    if !requirement.is_satisfied_by(&principal) {
        return Err(AppError::forbidden("missing required permission"));
    }

    request.extensions_mut().insert(principal);
    Ok(next.run(request).await)
}

fn parse_permissions(value: &str) -> impl Iterator<Item = String> + '_ {
    value
        .split(',')
        .map(str::trim)
        .filter(|permission| !permission.is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use http::HeaderMap;

    use super::{PermissionRequirement, Principal};

    #[test]
    fn parses_permissions_from_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("x-permissions", "admin:read, admin:write".parse().unwrap());

        let principal = Principal::from_headers(&headers)
            .expect("header parses")
            .expect("principal exists");

        assert!(principal.has_permission("admin:read"));
        assert!(principal.has_permission("admin:write"));
    }

    #[test]
    fn deduplicates_permissions() {
        let principal = Principal::new(None, ["admin:read".to_owned(), "admin:read".to_owned()]);

        assert_eq!(principal.permissions(), ["admin:read"]);
    }

    #[test]
    fn evaluates_all_required_permissions() {
        let requirement = PermissionRequirement::all(["admin:read", "admin:write"]);
        let principal = Principal::new(None, ["admin:read".to_owned()]);

        assert!(!requirement.is_satisfied_by(&principal));
    }
}
