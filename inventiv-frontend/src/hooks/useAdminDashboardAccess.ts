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
      setLoading(true);
      try {
        const meRes = await apiRequest("/auth/me");
        if (meRes.ok) {
          const meData = (await meRes.json()) as Me;
          setMe(meData);
        }
      } catch (e) {
        console.error("Failed to fetch admin dashboard access data:", e);
      } finally {
        setLoading(false);
      }
    };

    void fetchData();
  }, []);

  const check = useMemo((): AdminDashboardAccessCheck => {
    // Check 1: Must be in an organization workspace
    if (!me?.current_organization_id) {
      return {
        canAccess: false,
        loading: false,
        reason: "Vous devez être dans un workspace organisation pour accéder au Admin Dashboard",
      };
    }

    // Check 2: Must have Owner, Admin, or Manager role
    const orgRole = parseRole(me.current_organization_role);
    if (!orgRole) {
      return {
        canAccess: false,
        loading: false,
        reason: "Rôle d'organisation invalide",
      };
    }

    // Owner, Admin, and Manager can access
    const allowedRoles: OrgRole[] = ["owner", "admin", "manager"];
    if (!allowedRoles.includes(orgRole)) {
      return {
        canAccess: false,
        loading: false,
        reason: "Seuls les rôles Owner, Admin et Manager peuvent accéder au Admin Dashboard",
      };
    }

    return { canAccess: true, loading: false };
  }, [me]);

  return {
    ...check,
    loading,
  };
}

