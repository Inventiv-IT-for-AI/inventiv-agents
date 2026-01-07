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
    matches!(
        (role, flag),
        (OrgRole::Owner, _)
            | (OrgRole::Admin, ActivationFlag::Tech)
            | (OrgRole::Manager, ActivationFlag::Eco)
    )
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

// --- Phase 2: Instance Permissions ---

/// Vérifier si un rôle peut voir les instances
pub fn can_view_instances(role: &OrgRole) -> bool {
    matches!(
        role,
        OrgRole::Owner | OrgRole::Admin | OrgRole::Manager | OrgRole::User
    )
}

/// Vérifier si un rôle peut créer/modifier/terminer instances
pub fn can_modify_instances(role: &OrgRole) -> bool {
    matches!(role, OrgRole::Owner | OrgRole::Admin)
}

/// Vérifier si un rôle peut activer techniquement
pub fn can_activate_tech(role: &OrgRole) -> bool {
    matches!(role, OrgRole::Owner | OrgRole::Admin)
}

/// Vérifier si un rôle peut activer économiquement
pub fn can_activate_eco(role: &OrgRole) -> bool {
    matches!(role, OrgRole::Owner | OrgRole::Manager)
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

    #[test]
    fn instance_permissions() {
        // View: all roles can view
        assert!(can_view_instances(&OrgRole::Owner));
        assert!(can_view_instances(&OrgRole::Admin));
        assert!(can_view_instances(&OrgRole::Manager));
        assert!(can_view_instances(&OrgRole::User));

        // Modify: only Owner and Admin
        assert!(can_modify_instances(&OrgRole::Owner));
        assert!(can_modify_instances(&OrgRole::Admin));
        assert!(!can_modify_instances(&OrgRole::Manager));
        assert!(!can_modify_instances(&OrgRole::User));

        // Activate tech: Owner and Admin
        assert!(can_activate_tech(&OrgRole::Owner));
        assert!(can_activate_tech(&OrgRole::Admin));
        assert!(!can_activate_tech(&OrgRole::Manager));
        assert!(!can_activate_tech(&OrgRole::User));

        // Activate eco: Owner and Manager
        assert!(can_activate_eco(&OrgRole::Owner));
        assert!(!can_activate_eco(&OrgRole::Admin));
        assert!(can_activate_eco(&OrgRole::Manager));
        assert!(!can_activate_eco(&OrgRole::User));
    }
}
