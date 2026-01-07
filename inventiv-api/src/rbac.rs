use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrgRole {
    Owner,
    Admin,
    Manager,
    User,
}

impl OrgRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            OrgRole::Owner => "owner",
            OrgRole::Admin => "admin",
            OrgRole::Manager => "manager",
            OrgRole::User => "user",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "owner" => Some(OrgRole::Owner),
            "admin" => Some(OrgRole::Admin),
            "manager" => Some(OrgRole::Manager),
            "user" => Some(OrgRole::User),
            _ => None,
        }
    }
}

/// Who can invite users to the organization.
pub fn can_invite(role: OrgRole) -> bool {
    matches!(role, OrgRole::Owner | OrgRole::Admin | OrgRole::Manager)
}

/// Double activation (per resource): which flag(s) a role can change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivationFlag {
    Tech,
    Eco,
}

pub fn can_set_activation_flag(role: OrgRole, flag: ActivationFlag) -> bool {
    match (role, flag) {
        (OrgRole::Owner, _) => true,
        (OrgRole::Admin, ActivationFlag::Tech) => true,
        (OrgRole::Manager, ActivationFlag::Eco) => true,
        _ => false,
    }
}

/// Role delegation rules (cible):
/// - Owner can assign any role
/// - Manager can toggle Manager <-> User
/// - Admin can toggle Admin <-> User
pub fn can_assign_role(actor: OrgRole, from: OrgRole, to: OrgRole) -> bool {
    match actor {
        OrgRole::Owner => true,
        OrgRole::Manager => matches!(
            (from, to),
            (OrgRole::User, OrgRole::Manager) | (OrgRole::Manager, OrgRole::User)
        ),
        OrgRole::Admin => matches!(
            (from, to),
            (OrgRole::User, OrgRole::Admin) | (OrgRole::Admin, OrgRole::User)
        ),
        OrgRole::User => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_parse_roundtrip() {
        for (s, r) in [
            ("owner", OrgRole::Owner),
            ("admin", OrgRole::Admin),
            ("manager", OrgRole::Manager),
            ("user", OrgRole::User),
        ] {
            assert_eq!(OrgRole::parse(s), Some(r));
            assert_eq!(OrgRole::parse(&s.to_uppercase()), Some(r));
            assert_eq!(r.as_str(), s);
        }
        assert_eq!(OrgRole::parse("unknown"), None);
    }

    #[test]
    fn invite_rules() {
        assert!(can_invite(OrgRole::Owner));
        assert!(can_invite(OrgRole::Admin));
        assert!(can_invite(OrgRole::Manager));
        assert!(!can_invite(OrgRole::User));
    }

    #[test]
    fn activation_flag_rules() {
        assert!(can_set_activation_flag(
            OrgRole::Owner,
            ActivationFlag::Tech
        ));
        assert!(can_set_activation_flag(OrgRole::Owner, ActivationFlag::Eco));
        assert!(can_set_activation_flag(
            OrgRole::Admin,
            ActivationFlag::Tech
        ));
        assert!(!can_set_activation_flag(
            OrgRole::Admin,
            ActivationFlag::Eco
        ));
        assert!(can_set_activation_flag(
            OrgRole::Manager,
            ActivationFlag::Eco
        ));
        assert!(!can_set_activation_flag(
            OrgRole::Manager,
            ActivationFlag::Tech
        ));
        assert!(!can_set_activation_flag(
            OrgRole::User,
            ActivationFlag::Tech
        ));
        assert!(!can_set_activation_flag(OrgRole::User, ActivationFlag::Eco));
    }

    #[test]
    fn delegation_rules() {
        // Owner can do all.
        assert!(can_assign_role(
            OrgRole::Owner,
            OrgRole::User,
            OrgRole::Owner
        ));

        // Manager: only manager<->user
        assert!(can_assign_role(
            OrgRole::Manager,
            OrgRole::User,
            OrgRole::Manager
        ));
        assert!(can_assign_role(
            OrgRole::Manager,
            OrgRole::Manager,
            OrgRole::User
        ));
        assert!(!can_assign_role(
            OrgRole::Manager,
            OrgRole::User,
            OrgRole::Admin
        ));

        // Admin: only admin<->user
        assert!(can_assign_role(
            OrgRole::Admin,
            OrgRole::User,
            OrgRole::Admin
        ));
        assert!(can_assign_role(
            OrgRole::Admin,
            OrgRole::Admin,
            OrgRole::User
        ));
        assert!(!can_assign_role(
            OrgRole::Admin,
            OrgRole::User,
            OrgRole::Manager
        ));

        // User: none
        assert!(!can_assign_role(
            OrgRole::User,
            OrgRole::User,
            OrgRole::Admin
        ));
    }
}
