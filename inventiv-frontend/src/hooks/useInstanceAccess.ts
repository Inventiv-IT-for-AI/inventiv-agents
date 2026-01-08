import { useState, useEffect, useMemo } from "react";
import { apiRequest, apiUrl } from "@/lib/api";
import type { Provider } from "@/lib/types";
import { parseRole, canModifyInstances, type OrgRole } from "@/lib/rbac";

export type InstanceAccessCheck = {
  canAccess: boolean;
  canProvision: boolean;
  reasons: string[];
  hasActiveProviders: boolean;
  hasConfiguredProviders: boolean;
};

type Me = {
  current_organization_id?: string | null;
  current_organization_role?: string | null;
};

type ProviderParams = {
  provider_id: string;
  provider_name: string;
  provider_code: string;
};

type ProviderConfigStatus = {
  provider_id: string;
  provider_code: string;
  provider_name: string;
  is_configured: boolean;
  missing_config: string[];
};

/**
 * Hook to check if user can access and provision instances
 * Requirements:
 * - Must be in an organization workspace
 * - Must have Owner or Admin role
 * - Must have active providers
 * - Must have configured providers (credentials, SSH keys, etc.)
 */
export function useInstanceAccess() {
  const [me, setMe] = useState<Me | null>(null);
  const [providers, setProviders] = useState<Provider[]>([]);
  const [providerParams, setProviderParams] = useState<ProviderParams[]>([]);
  const [providerConfigStatus, setProviderConfigStatus] = useState<ProviderConfigStatus[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const fetchData = async () => {
      setLoading(true);
      try {
        // Fetch current user info
        const meRes = await apiRequest("/auth/me");
        if (meRes.ok) {
          const meData = (await meRes.json()) as Me;
          setMe(meData);
        }

        // Fetch providers
        const providersRes = await fetch(apiUrl("/providers"));
        if (providersRes.ok) {
          const providersData = (await providersRes.json()) as Provider[];
          setProviders(providersData.filter((p) => p.is_active));
        }

        // Fetch provider params (configuration)
        const paramsRes = await fetch(apiUrl("/providers/params"));
        if (paramsRes.ok) {
          const paramsData = (await paramsRes.json()) as ProviderParams[];
          setProviderParams(paramsData);
        }

        // Fetch provider config status (credentials check)
        const configStatusRes = await fetch(apiUrl("/providers/config-status"));
        if (configStatusRes.ok) {
          const configStatusData = (await configStatusRes.json()) as ProviderConfigStatus[];
          setProviderConfigStatus(configStatusData);
        }
      } catch (e) {
        console.error("Failed to fetch instance access data:", e);
      } finally {
        setLoading(false);
      }
    };

    void fetchData();
  }, []);

  const check = useMemo((): InstanceAccessCheck => {
    const reasons: string[] = [];
    let canAccess = false;
    let canProvision = false;

    // Check 1: Must be in an organization workspace
    if (!me?.current_organization_id) {
      reasons.push("Vous devez être dans un workspace organisation pour accéder aux instances");
      return { canAccess: false, canProvision: false, reasons, hasActiveProviders: false, hasConfiguredProviders: false };
    }

    // Check 2: Must have Owner or Admin role
    const orgRole = parseRole(me.current_organization_role);
    if (!orgRole || !canModifyInstances(orgRole)) {
      reasons.push("Seuls les rôles Owner et Admin peuvent gérer les instances (opérations techniques)");
      return { canAccess: false, canProvision: false, reasons, hasActiveProviders: false, hasConfiguredProviders: false };
    }

    // If we get here, user can access instances
    canAccess = true;

    // Check 3: Must have active providers
    const activeProviders = providers.filter((p) => p.is_active);
    const hasActiveProviders = activeProviders.length > 0;
    if (!hasActiveProviders) {
      reasons.push("Aucun provider actif disponible pour votre organisation");
      return { canAccess: true, canProvision: false, reasons, hasActiveProviders: false, hasConfiguredProviders: false };
    }

    // Check 4: Must have configured providers (credentials check)
    const configuredProviders = activeProviders.filter((p) => {
      const configStatus = providerConfigStatus.find((cs) => cs.provider_id === p.id);
      return configStatus?.is_configured ?? false;
    });

    const hasConfiguredProviders = configuredProviders.length > 0;
    if (!hasConfiguredProviders) {
      const missingConfigs = providerConfigStatus
        .filter((cs) => !cs.is_configured && activeProviders.some((p) => p.id === cs.provider_id))
        .map((cs) => {
          const missing = cs.missing_config.length > 0 
            ? `: ${cs.missing_config.join(", ")}`
            : "";
          return `${cs.provider_name}${missing}`;
        });
      
      if (missingConfigs.length > 0) {
        reasons.push(
          `Aucun provider n'est correctement configuré. Veuillez configurer les credentials dans les paramètres. Providers non configurés: ${missingConfigs.join("; ")}`
        );
      } else {
        reasons.push(
          "Aucun provider n'est correctement configuré. Veuillez configurer les credentials (Access Key, Secret Key, SSH Keys) dans les paramètres."
        );
      }
      return { canAccess: true, canProvision: false, reasons, hasActiveProviders: true, hasConfiguredProviders: false };
    }

    // All checks passed
    canProvision = true;
    return { canAccess: true, canProvision: true, reasons: [], hasActiveProviders: true, hasConfiguredProviders: true };
  }, [me, providers, providerParams, providerConfigStatus]);

  return {
    ...check,
    loading,
    me,
    activeProviders: providers.filter((p) => p.is_active),
  };
}

