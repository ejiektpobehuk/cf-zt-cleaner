use serde::Deserialize;

/// User as returned by `CloudFlare` Access Users API
#[derive(Debug, Clone, Deserialize)]
pub struct CloudFlareUser {
    pub id: String,
    pub email: Option<String>,
    /// Whether this user has an active Access seat
    #[serde(default)]
    pub access_seat: bool,
}

impl CloudFlareUser {
    /// Returns true if this user has an active Zero Trust seat
    pub const fn has_active_seat(&self) -> bool {
        self.access_seat
    }
}

/// Unified user representation for comparison
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct User {
    pub id: Option<String>,
    pub email: String,
}

/// Error when converting a `CloudFlare` user with no email
#[derive(Debug)]
pub struct MissingEmailError {
    pub user_id: String,
}

impl TryFrom<CloudFlareUser> for User {
    type Error = MissingEmailError;

    fn try_from(cf_user: CloudFlareUser) -> Result<Self, Self::Error> {
        match cf_user.email {
            Some(email) => Ok(Self {
                id: Some(cf_user.id),
                email,
            }),
            None => Err(MissingEmailError {
                user_id: cf_user.id,
            }),
        }
    }
}

impl User {
    /// Create a User from an email string (for config permanent list)
    pub const fn from_email(email: String) -> Self {
        Self { id: None, email }
    }

    /// Check if this user matches another by email (case-insensitive)
    pub fn matches(&self, other: &Self) -> bool {
        self.email.to_lowercase() == other.email.to_lowercase()
    }

    /// Check if this user's email is in a list of permanent users
    pub fn is_in_permanent_list(&self, permanent_users: &[Self]) -> bool {
        permanent_users.iter().any(|u| self.matches(u))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_matching_case_insensitive() {
        let user1 = User {
            id: Some("123".into()),
            email: "Test@Example.com".into(),
        };
        let user2 = User {
            id: None,
            email: "test@example.com".into(),
        };

        assert!(user1.matches(&user2));
    }

    #[test]
    fn test_user_in_permanent_list() {
        let cf_user = User {
            id: Some("123".into()),
            email: "keep@example.com".into(),
        };

        let permanent = vec![
            User {
                id: None,
                email: "keep@example.com".into(),
            },
            User {
                id: None,
                email: "also-keep@example.com".into(),
            },
        ];

        assert!(cf_user.is_in_permanent_list(&permanent));
    }
}
