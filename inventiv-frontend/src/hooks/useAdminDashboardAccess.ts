import { useState, useEffect, useMemo } from "react";
import { apiRequest } from "@/lib/api";
import { parseRole, type OrgRole } from "@/lib/rbac";

type Me = {
  current_organization_id?: string | null;
  current_organization_role?: string | null;
};

export type AdminDashboardAccessCheck = {
  canAccess: boolean;
  loading: boolean;
  reason?: string;
};

/**
 * Hook to check if user can access Admin Dashboard
 * Requirements:
 * - Must be in an organization workspace
 * - Must have Owner, Admin, or Manager role
 */
export function useAdminDashboardAccess() {
  const [me, setMe] = useState<Me | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const fetchData = async () => {
      try {
        const meRes = await apiRequest("/auth/me");
        if (meRes.ok) {
          const meData = (await meRes.json()) as Me;
          console.log("[useAdminDashboardAccess] Me data:", meData);
          console.log("[useAdminDashboardAccess] current_organization_id:", meData.current_organization_id);
          console.log("[useAdminDashboardAccess] current_organization_role:", meData.current_organization_role);
          setMe(meData);
        } else {
          console.error("[useAdminDashboardAccess] Failed to fetch /auth/me:", meRes.status);
        }
      } catch (e) {
        console.error("Failed to fetch admin dashboard access data:", e);
      } finally {
        setLoading(false);
      }
    };

    // Initial fetch
    setLoading(true);
    void fetchData();

    // Poll every 5 seconds to catch workspace changes
    const interval = setInterval(() => {
      void fetchData();
    }, 5000);

    // Listen for workspace changes (when user switches organization)
    const handleWorkspaceChange = () => {
      setLoading(true);
      void fetchData();
    };
    window.addEventListener("workspace-changed", handleWorkspaceChange);

    return () => {
      clearInterval(interval);
      window.removeEventListener("workspace-changed", handleWorkspaceChange);
    };
  }, []);

  const check = useMemo((): AdminDashboardAccessCheck => {
    if (!me) {
      return { canAccess: false, loading: true };
    }

    // Check 1: Must be in an organization workspace
    if (!me.current_organization_id) {
      console.log("[useAdminDashboardAccess] No organization workspace");
      return {
        canAccess: false,
        loading: false,
        reason: "Vous devez être dans un workspace organisation pour accéder au Admin Dashboard",
      };
    }

    // Check 2: Must have Owner, Admin, or Manager role
    const orgRole = parseRole(me.current_organization_role);
    console.log("[useAdminDashboardAccess] Parsed role:", orgRole, "from:", me.current_organization_role);
    
    if (!orgRole) {
      console.log("[useAdminDashboardAccess] Invalid role");
      return {
        canAccess: false,
        loading: false,
        reason: `Rôle d'organisation invalide: ${me.current_organization_role}`,
      };
    }

    // Owner, Admin, and Manager can access
    const allowedRoles: OrgRole[] = ["owner", "admin", "manager"];
    if (!allowedRoles.includes(orgRole)) {
      console.log("[useAdminDashboardAccess] Role not allowed:", orgRole);
      return {
        canAccess: false,
        loading: false,
        reason: `Seuls les rôles Owner, Admin et Manager peuvent accéder au Admin Dashboard (votre rôle: ${orgRole})`,
      };
    }

    console.log("[useAdminDashboardAccess] Access granted for role:", orgRole);
    return { canAccess: true, loading: false };
  }, [me]);

  return {
    canAccess: check.canAccess,
    loading: loading || check.loading,
    reason: check.reason,
  };
}

