use axum::http::HeaderMap;

use crate::{AppError, AppResult};

pub fn require_staff(headers: &HeaderMap, expected_token: &str) -> AppResult<()> {
    let supplied = headers
        .get("x-staff-token")
        .and_then(|value| value.to_str().ok());
    match supplied {
        Some(value) if !expected_token.is_empty() && constant_time_equal(value, expected_token) => {
            Ok(())
        }
        _ => Err(AppError::Unauthorized),
    }
}

fn constant_time_equal(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    let length = left.len().max(right.len());
    let mut difference = left.len() ^ right.len();
    for index in 0..length {
        difference |= usize::from(*left.get(index).unwrap_or(&0) ^ *right.get(index).unwrap_or(&0));
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permits_only_the_configured_staff_token() {
        let mut headers = HeaderMap::new();
        headers.insert("x-staff-token", "dev-staff-token".parse().unwrap());
        assert!(require_staff(&headers, "dev-staff-token").is_ok());
        assert!(matches!(
            require_staff(&headers, "other"),
            Err(AppError::Unauthorized)
        ));
    }

    #[test]
    fn rejects_missing_invalid_and_empty_staff_tokens() {
        let headers = HeaderMap::new();
        assert!(matches!(
            require_staff(&headers, "secret"),
            Err(AppError::Unauthorized)
        ));
        let mut invalid = HeaderMap::new();
        invalid.insert("x-staff-token", "".parse().unwrap());
        assert!(matches!(
            require_staff(&invalid, "secret"),
            Err(AppError::Unauthorized)
        ));
        assert!(matches!(
            require_staff(&invalid, ""),
            Err(AppError::Unauthorized)
        ));
    }

    #[test]
    fn compares_tokens_with_different_lengths() {
        assert!(!constant_time_equal("abc", "abcd"));
        assert!(!constant_time_equal("abd", "abc"));
        assert!(constant_time_equal("same", "same"));
    }
}
