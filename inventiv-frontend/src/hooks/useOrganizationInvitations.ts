import { useState, useCallback } from "react";
import { apiRequest } from "@/lib/api";
import type { OrganizationInvitation } from "@/lib/types";

export type CreateInvitationParams = {
  email: string;
  role?: string;
  expires_in_days?: number;
};

export function useOrganizationInvitations() {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const listInvitations = useCallback(async (): Promise<OrganizationInvitation[]> => {
    setError(null);
    setLoading(true);
    try {
      // Ensure we're using the current organization
      const res = await apiRequest("/organizations/current/invitations");
      if (!res.ok) {
        const body = await res.json().catch(() => null);
        const code = body?.error || body?.message;
        const msg =
          code === "no_current_organization"
            ? "Aucune organisation sélectionnée"
            : code === "not_a_member"
              ? "Vous n'êtes pas membre de cette organisation"
              : "Impossible de charger les invitations";
        setError(msg);
        throw new Error(msg);
      }
      const data = (await res.json()) as OrganizationInvitation[];
      return Array.isArray(data) ? data : [];
    } catch (e) {
      const msg = e instanceof Error ? e.message : "Erreur réseau";
      setError(msg);
      throw e;
    } finally {
      setLoading(false);
    }
  }, []);

  const createInvitation = useCallback(
    async (params: CreateInvitationParams): Promise<OrganizationInvitation> => {
      setError(null);
      setLoading(true);
      try {
        // Ensure we're using the current organization
        const res = await apiRequest("/organizations/current/invitations", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            email: params.email.trim().toLowerCase(),
            role: params.role || "user",
            expires_in_days: params.expires_in_days || 7,
          }),
        });
        if (!res.ok) {
          const body = await res.json().catch(() => null);
          const code = body?.error || body?.message;
          const msg =
            code === "no_current_organization"
              ? "Aucune organisation sélectionnée"
              : code === "not_a_member"
                ? "Vous n'êtes pas membre de cette organisation"
                : code === "insufficient_permissions_to_invite"
                  ? "Permissions insuffisantes pour inviter"
                : code === "invalid_email_format"
                  ? "Format d'email invalide"
                : code === "user_already_member"
                  ? "Cet utilisateur est déjà membre"
                : code === "invitation_already_exists"
                  ? "Une invitation existe déjà pour cet email"
                : "Erreur lors de la création de l'invitation";
          setError(msg);
          throw new Error(msg);
        }
        const data = (await res.json()) as OrganizationInvitation;
        return data;
      } catch (e) {
        const msg = e instanceof Error ? e.message : "Erreur réseau";
        setError(msg);
        throw e;
      } finally {
        setLoading(false);
      }
    },
    []
  );

  const acceptInvitation = useCallback(async (token: string): Promise<void> => {
    setError(null);
    setLoading(true);
    try {
      const res = await apiRequest(`/organizations/invitations/${token}/accept`, {
        method: "POST",
      });
      if (!res.ok) {
        const body = await res.json().catch(() => null);
        const code = body?.error || body?.message;
        const msg =
          code === "invitation_not_found"
            ? "Invitation non trouvée"
            : code === "invitation_expired"
              ? "Cette invitation a expiré"
              : code === "invitation_already_accepted"
                ? "Cette invitation a déjà été acceptée"
              : code === "email_mismatch"
                ? "L'email de l'invitation ne correspond pas à votre compte"
                : "Erreur lors de l'acceptation de l'invitation";
        setError(msg);
        throw new Error(msg);
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : "Erreur réseau";
      setError(msg);
      throw e;
    } finally {
      setLoading(false);
    }
  }, []);

  return {
    loading,
    error,
    listInvitations,
    createInvitation,
    acceptInvitation,
  };
}

