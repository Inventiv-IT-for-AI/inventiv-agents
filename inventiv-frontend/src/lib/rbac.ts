/**
 * RBAC (Role-Based Access Control) rules for organizations
 * These rules mirror the backend RBAC logic in inventiv-api/src/rbac.rs
 */

export type OrgRole = "owner" | "admin" | "manager" | "user";

export function parseRole(s: string | null | undefined): OrgRole | null {
  const v = (s ?? "").toLowerCase().trim();
  if (v === "owner" || v === "admin" || v === "manager" || v === "user") return v;
  return null;
}

/**
 * Who can invite users to the organization
 */
export function canInvite(role: OrgRole): boolean {
  return role === "owner" || role === "admin" || role === "manager";
}

/**
 * Role delegation rules:
 * - Owner can assign any role
 * - Manager can toggle Manager <-> User
 * - Admin can toggle Admin <-> User
 */
export function canAssignRole(actor: OrgRole, from: OrgRole, to: OrgRole): boolean {
  if (actor === "owner") return true;
  if (actor === "manager") {
    return (from === "user" && to === "manager") || (from === "manager" && to === "user");
  }
  if (actor === "admin") {
    return (from === "user" && to === "admin") || (from === "admin" && to === "user");
  }
  return false;
}

/**
 * Who can remove a member:
 * - Owner: anyone (except last-owner invariant)
 * - Admin: admin/user
 * - Manager: manager/user
 * - User: only self (leave)
 */
export function canRemoveMember(actor: OrgRole, target: OrgRole, isSelf: boolean): boolean {
  if (isSelf) return true;
  if (actor === "owner") return true;
  if (actor === "admin") return target === "admin" || target === "user";
  if (actor === "manager") return target === "manager" || target === "user";
  return false;
}

/**
 * Who can invite which roles:
 * - Owner: can invite any role
 * - Admin/Manager: can only invite User/Manager (not Owner/Admin)
 */
export function canInviteRole(actor: OrgRole, inviteRole: OrgRole): boolean {
  if (actor === "owner") return true;
  // Admin/Manager can only invite User/Manager
  return inviteRole === "user" || inviteRole === "manager";
}

/**
 * Check if a role can view instances
 */
export function canViewInstances(role: OrgRole): boolean {
  return true; // All roles can view instances
}

/**
 * Check if a role can modify instances (create/update/delete)
 */
export function canModifyInstances(role: OrgRole): boolean {
  return role === "owner" || role === "admin";
}

