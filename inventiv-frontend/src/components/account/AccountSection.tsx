"use client";

import { useCallback, useEffect, useMemo, useState, type ChangeEvent } from "react";
import { useRouter } from "next/navigation";
import type { Organization } from "@/lib/types";
import { apiUrl, apiRequest } from "@/lib/api";

import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

import { OrganizationSection, type WorkspaceMe } from "./OrganizationSection";
import { useSnackbar } from "ia-widgets";
import { OrganizationMembersDialog } from "./OrganizationMembersDialog";

export type Me = WorkspaceMe & {
  user_id: string;
  username: string;
  email: string;
  role: string;
  first_name?: string | null;
  last_name?: string | null;
  current_organization_name?: string | null;
  current_organization_slug?: string | null;
};

export type AccountSectionProps = {
  onMeChange?: (me: Me | null) => void;
};

export function AccountSection({ onMeChange }: AccountSectionProps) {
  const router = useRouter();
  const snackbar = useSnackbar();

  const [menuOpen, setMenuOpen] = useState(false);
  const [profileOpen, setProfileOpen] = useState(false);

  const [me, setMe] = useState<Me | null>(null);

  const [profileForm, setProfileForm] = useState({
    username: "",
    email: "",
    first_name: "",
    last_name: "",
  });

  const [pwdForm, setPwdForm] = useState({
    current_password: "",
    new_password: "",
    confirm_new_password: "",
  });

  const [profileSaving, setProfileSaving] = useState(false);
  const [pwdSaving, setPwdSaving] = useState(false);
  const [profileError, setProfileError] = useState<string | null>(null);
  const [pwdError, setPwdError] = useState<string | null>(null);
  const [pwdSuccess, setPwdSuccess] = useState<string | null>(null);

  // Organizations (multi-tenant MVP)
  const [orgs, setOrgs] = useState<Organization[]>([]);
  const [orgLoading, setOrgLoading] = useState(false);
  const [orgError, setOrgError] = useState<string | null>(null);
  const [createOrgOpen, setCreateOrgOpen] = useState(false);
  const [createOrgForm, setCreateOrgForm] = useState({ name: "", slug: "" });
  const [createOrgSaving, setCreateOrgSaving] = useState(false);
  const [createOrgError, setCreateOrgError] = useState<string | null>(null);
  const [orgMembersOpen, setOrgMembersOpen] = useState(false);

  const currentOrgRole = useMemo(() => {
    const orgId = me?.current_organization_id;
    if (!orgId) return null;
    const o = orgs.find((x) => x.id === orgId);
    return o?.role ?? null;
  }, [me?.current_organization_id, orgs]);

  const displayName = useMemo(() => {
    if (!me) return "User";
    const full = `${me.first_name ?? ""} ${me.last_name ?? ""}`.trim();
    return full || me.username || me.email || "User";
  }, [me]);

  const workspaceLabel = useMemo(() => {
    if (!me?.current_organization_id) return "Personal";
    const name = (me.current_organization_name || "").trim();
    const slug = (me.current_organization_slug || "").trim();
    if (name && slug) return `${name} (${slug})`;
    return name || slug || "Organization";
  }, [me]);

  const initials = useMemo(() => {
    const s = displayName.trim();
    if (!s) return "U";
    const parts = s.split(/\s+/).filter(Boolean);
    if (parts.length === 1) return parts[0].slice(0, 2).toUpperCase();
    return (parts[0][0] + parts[1][0]).toUpperCase();
  }, [displayName]);

  const fetchMe = useCallback(async () => {
    const res = await apiRequest("/auth/me");
    if (!res.ok) {
      // 401 is handled automatically by apiRequest (redirects to /login)
      setMe(null);
      onMeChange?.(null);
      return;
    }
    const data = (await res.json()) as Me;
    setMe(data);
    onMeChange?.(data);
    setProfileForm({
      username: data.username ?? (typeof data.email === "string" ? String(data.email).split("@")[0] : ""),
      email: data.email ?? "",
      first_name: data.first_name ?? "",
      last_name: data.last_name ?? "",
    });
  }, [onMeChange]);

  const fetchOrgs = useCallback(async () => {
    setOrgError(null);
    setOrgLoading(true);
    try {
      const res = await apiRequest("/organizations");
      if (!res.ok) {
        // 401 is handled automatically by apiRequest (redirects to /login)
        setOrgs([]);
        setOrgError("Erreur lors du chargement des organisations");
        return;
      }
      const data = (await res.json()) as Organization[];
      setOrgs(Array.isArray(data) ? data : []);
    } catch (e) {
      console.error(e);
      setOrgError("Erreur réseau");
      setOrgs([]);
    } finally {
      setOrgLoading(false);
    }
  }, []);

  useEffect(() => {
    void fetchMe().catch(() => null);
  }, [fetchMe]);

  useEffect(() => {
    void fetchOrgs().catch(() => null);
  }, [fetchOrgs]);

  const logout = async () => {
    try {
      await apiRequest("/auth/logout", { method: "POST" });
    } finally {
      router.replace("/login");
    }
  };

  const saveProfile = async () => {
    setProfileError(null);
    setProfileSaving(true);
    try {
      const res = await apiRequest("/auth/me", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          username: profileForm.username,
          email: profileForm.email,
          first_name: profileForm.first_name || null,
          last_name: profileForm.last_name || null,
        }),
      });
      if (!res.ok) {
        // 401 is handled automatically by apiRequest (redirects to /login)
        const body = await res.json().catch(() => null);
        const code = body?.error || body?.message;
        const msg =
          code === "conflict" || code === "username_or_email_already_exists"
            ? "Username ou email déjà utilisé"
            : code === "session_invalid"
              ? "Session expirée, veuillez vous reconnecter"
              : "Erreur lors de la mise à jour"
        setProfileError(msg);
        snackbar.error(msg, {
          title: "Profil",
          details: JSON.stringify({ status: res.status, body }, null, 2),
        });
        return;
      }
      const data = (await res.json()) as Me;
      setMe(data);
      onMeChange?.(data);
      snackbar.success("Profil mis à jour", { title: "Profil" });
    } catch (e) {
      console.error(e);
      setProfileError("Erreur réseau");
      snackbar.error("Erreur réseau", {
        title: "Profil",
        details: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setProfileSaving(false);
    }
  };

  const setCurrentOrg = async (organizationId: string) => {
    setOrgError(null);
    setOrgLoading(true);
    try {
      const res = await apiRequest("/organizations/current", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ organization_id: organizationId }),
      });
      if (!res.ok) {
        // 401 is handled automatically by apiRequest (redirects to /login)
        const body = await res.json().catch(() => null);
        const code = body?.error || body?.message;
        const msg =
          code === "not_a_member"
            ? "Vous n’êtes pas membre de cette organisation"
            : "Impossible de changer d’organisation";
        setOrgError(msg);
        snackbar.error(msg, { title: "Workspace", details: JSON.stringify({ status: res.status, body }, null, 2) });
        return;
      }
      await fetchMe();
      await fetchOrgs();
      snackbar.success("Workspace mis à jour", { title: "Workspace" });
    } catch (e) {
      console.error(e);
      setOrgError("Erreur réseau");
      snackbar.error("Erreur réseau", { title: "Workspace", details: e instanceof Error ? e.message : String(e) });
    } finally {
      setOrgLoading(false);
    }
  };

  const setPersonalWorkspace = async () => {
    setOrgError(null);
    setOrgLoading(true);
    try {
      const res = await apiRequest("/organizations/current", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ organization_id: null }),
      });
      if (!res.ok) {
        // 401 is handled automatically by apiRequest (redirects to /login)
        setOrgError("Impossible de revenir en mode Personal");
        const body = await res.text().catch(() => "");
        snackbar.error("Impossible de revenir en mode Personal", {
          title: "Workspace",
          details: JSON.stringify({ status: res.status, body }, null, 2),
        });
        return;
      }
      await fetchMe();
      await fetchOrgs();
      snackbar.success("Workspace: Personal", { title: "Workspace" });
    } catch (e) {
      console.error(e);
      setOrgError("Erreur réseau");
      snackbar.error("Erreur réseau", { title: "Workspace", details: e instanceof Error ? e.message : String(e) });
    } finally {
      setOrgLoading(false);
    }
  };

  const createOrg = async () => {
    setCreateOrgError(null);
    const name = createOrgForm.name.trim();
    const slug = createOrgForm.slug.trim();
    if (!name) {
      setCreateOrgError("Nom requis");
      snackbar.warning("Nom requis", { title: "Organisation" });
      return;
    }
    setCreateOrgSaving(true);
    try {
      const res = await apiRequest("/organizations", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          name,
          slug: slug || null,
          set_as_current: true,
        }),
      });
      if (!res.ok) {
        // 401 is handled automatically by apiRequest (redirects to /login)
        const body = await res.json().catch(() => null);
        const code = body?.error || body?.message;
        const msg =
          code === "organization_slug_already_exists" || code === "conflict"
            ? "Slug déjà utilisé"
            : code === "name_required"
              ? "Nom requis"
              : "Erreur lors de la création"
        setCreateOrgError(msg);
        snackbar.error(msg, { title: "Organisation", details: JSON.stringify({ status: res.status, body }, null, 2) });
        return;
      }
      setCreateOrgOpen(false);
      setCreateOrgForm({ name: "", slug: "" });
      await fetchMe();
      await fetchOrgs();
      snackbar.success("Organisation créée", { title: "Organisation" });
    } catch (e) {
      console.error(e);
      setCreateOrgError("Erreur réseau");
      snackbar.error("Erreur réseau", { title: "Organisation", details: e instanceof Error ? e.message : String(e) });
    } finally {
      setCreateOrgSaving(false);
    }
  };

  const changePassword = async () => {
    setPwdError(null);
    setPwdSuccess(null);
    if (!pwdForm.current_password.trim() || !pwdForm.new_password.trim()) {
      setPwdError("Veuillez remplir tous les champs");
      snackbar.warning("Veuillez remplir tous les champs", { title: "Mot de passe" });
      return;
    }
    if (pwdForm.new_password !== pwdForm.confirm_new_password) {
      setPwdError("La confirmation ne correspond pas");
      snackbar.warning("La confirmation ne correspond pas", { title: "Mot de passe" });
      return;
    }
    setPwdSaving(true);
    try {
      const res = await apiRequest("/auth/me/password", {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          current_password: pwdForm.current_password,
          new_password: pwdForm.new_password,
        }),
      });
      if (!res.ok) {
        // 401 is handled automatically by apiRequest (redirects to /login)
        const body = await res.json().catch(() => null);
        const code = body?.error || body?.message;
        const msg =
          code === "invalid_current_password" || code === "current_password_invalid"
            ? "Mot de passe actuel incorrect"
            : code === "session_invalid"
              ? "Session expirée, veuillez vous reconnecter"
              : "Erreur lors du changement de mot de passe"
        setPwdError(msg);
        snackbar.error(msg, { title: "Mot de passe", details: JSON.stringify({ status: res.status, body }, null, 2) });
        return;
      }
      setPwdForm({ current_password: "", new_password: "", confirm_new_password: "" });
      setPwdSuccess("Mot de passe mis à jour");
      snackbar.success("Mot de passe mis à jour", { title: "Mot de passe" });
    } catch (e) {
      console.error(e);
      setPwdError("Erreur réseau");
      snackbar.error("Erreur réseau", { title: "Mot de passe", details: e instanceof Error ? e.message : String(e) });
    } finally {
      setPwdSaving(false);
    }
  };

  return (
    <>
      {/* User chip (bottom) */}
      <div className="p-3 border-t">
        <Button variant="ghost" className="w-full justify-start" onClick={() => setMenuOpen(true)}>
          <div className="h-8 w-8 rounded-full bg-muted flex items-center justify-center font-semibold text-xs mr-2">
            {initials}
          </div>
          <div className="min-w-0 flex-1 text-left">
            <div className="text-sm font-medium truncate">{displayName}</div>
            <div className="text-xs text-muted-foreground truncate">
              {workspaceLabel} · {me?.role ?? "user"}
            </div>
          </div>
        </Button>
      </div>

      {/* Menu dialog */}
      <Dialog open={menuOpen} onOpenChange={setMenuOpen}>
        <DialogContent showCloseButton={false} className="sm:max-w-[420px]">
          <DialogHeader>
            <DialogTitle>Compte</DialogTitle>
          </DialogHeader>
          <div className="grid gap-4 py-2">
            <OrganizationSection
              me={me}
              orgs={orgs}
              orgLoading={orgLoading}
              orgError={orgError}
              onSelectOrg={(id) => void setCurrentOrg(id)}
              onSelectPersonal={() => void setPersonalWorkspace()}
              onOpenCreateOrg={() => {
                setCreateOrgError(null);
                setCreateOrgForm({ name: "", slug: "" });
                setCreateOrgOpen(true);
              }}
            />

            <div className="grid gap-2">
              <Button
                variant="outline"
                onClick={() => {
                  setMenuOpen(false);
                  setProfileOpen(true);
                  setPwdError(null);
                  setPwdSuccess(null);
                  setProfileError(null);
                }}
              >
                Mon profil
              </Button>
              <Button
                variant="destructive"
                onClick={async () => {
                  setMenuOpen(false);
                  await logout();
                }}
              >
                Se déconnecter
              </Button>
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setMenuOpen(false)}>
              Fermer
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Profile dialog */}
      <Dialog
        open={profileOpen}
        onOpenChange={(o: boolean) => {
          setProfileOpen(o);
          if (o) {
            void fetchMe().catch(() => null);
            void fetchOrgs().catch(() => null);
          }
        }}
      >
        <DialogContent showCloseButton={false} className="sm:max-w-[560px]">
          <DialogHeader>
            <DialogTitle>Mon profil</DialogTitle>
          </DialogHeader>

          <div className="grid gap-6 py-2">
            <div className="grid gap-3">
              <div className="text-sm font-medium">Organisation</div>
              <div className="grid grid-cols-4 items-center gap-4">
                <Label className="text-right">Courante</Label>
                <div className="col-span-3 flex items-center gap-2">
                  <OrganizationSection
                    me={me}
                    orgs={orgs}
                    orgLoading={orgLoading}
                    orgError={orgError}
                    onSelectOrg={(id) => void setCurrentOrg(id)}
                    onSelectPersonal={() => void setPersonalWorkspace()}
                    onOpenCreateOrg={() => {
                      setCreateOrgError(null);
                      setCreateOrgForm({ name: "", slug: "" });
                      setCreateOrgOpen(true);
                    }}
                    fullWidthTrigger={true}
                  />
                </div>
              </div>
              {me?.current_organization_id ? (
                <div className="flex justify-end">
                  <Button variant="outline" onClick={() => setOrgMembersOpen(true)}>
                    Membres & rôles
                  </Button>
                </div>
              ) : null}
              {me?.current_organization_name ? (
                <div className="text-xs text-muted-foreground">
                  Actuelle: {me.current_organization_name}
                  {me.current_organization_slug ? ` (${me.current_organization_slug})` : ""}
                </div>
              ) : null}
            </div>

            <div className="grid gap-3">
              <div className="grid grid-cols-4 items-center gap-4">
                <Label className="text-right">Username</Label>
                <Input className="col-span-3" value={profileForm.username} onChange={(e: ChangeEvent<HTMLInputElement>) => setProfileForm((s) => ({ ...s, username: e.target.value }))} />
              </div>
              <div className="grid grid-cols-4 items-center gap-4">
                <Label className="text-right">Email</Label>
                <Input className="col-span-3" value={profileForm.email} onChange={(e: ChangeEvent<HTMLInputElement>) => setProfileForm((s) => ({ ...s, email: e.target.value }))} />
              </div>
              <div className="grid grid-cols-4 items-center gap-4">
                <Label className="text-right">Prénom</Label>
                <Input className="col-span-3" value={profileForm.first_name} onChange={(e: ChangeEvent<HTMLInputElement>) => setProfileForm((s) => ({ ...s, first_name: e.target.value }))} />
              </div>
              <div className="grid grid-cols-4 items-center gap-4">
                <Label className="text-right">Nom</Label>
                <Input className="col-span-3" value={profileForm.last_name} onChange={(e: ChangeEvent<HTMLInputElement>) => setProfileForm((s) => ({ ...s, last_name: e.target.value }))} />
              </div>
              {profileError ? <div className="text-sm text-red-600">{profileError}</div> : null}
              <div className="flex justify-end">
                <Button onClick={saveProfile} disabled={profileSaving}>
                  {profileSaving ? "Enregistrement..." : "Enregistrer"}
                </Button>
              </div>
            </div>

            <div className="border-t pt-4 grid gap-3">
              <div className="text-sm font-medium">Changer le mot de passe</div>
              <div className="grid grid-cols-4 items-center gap-4">
                <Label className="text-right">Actuel</Label>
                <Input className="col-span-3" type="password" value={pwdForm.current_password} onChange={(e: ChangeEvent<HTMLInputElement>) => setPwdForm((s) => ({ ...s, current_password: e.target.value }))} />
              </div>
              <div className="grid grid-cols-4 items-center gap-4">
                <Label className="text-right">Nouveau</Label>
                <Input className="col-span-3" type="password" value={pwdForm.new_password} onChange={(e: ChangeEvent<HTMLInputElement>) => setPwdForm((s) => ({ ...s, new_password: e.target.value }))} />
              </div>
              <div className="grid grid-cols-4 items-center gap-4">
                <Label className="text-right">Confirmer</Label>
                <Input className="col-span-3" type="password" value={pwdForm.confirm_new_password} onChange={(e: ChangeEvent<HTMLInputElement>) => setPwdForm((s) => ({ ...s, confirm_new_password: e.target.value }))} />
              </div>
              {pwdError ? <div className="text-sm text-red-600">{pwdError}</div> : null}
              {pwdSuccess ? <div className="text-sm text-green-600">{pwdSuccess}</div> : null}
              <div className="flex justify-end">
                <Button onClick={changePassword} disabled={pwdSaving}>
                  {pwdSaving ? "Mise à jour..." : "Mettre à jour"}
                </Button>
              </div>
            </div>
          </div>

          <DialogFooter className="sm:justify-between">
            <Button variant="outline" onClick={() => setProfileOpen(false)}>
              Fermer
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Create org dialog */}
      <Dialog open={createOrgOpen} onOpenChange={setCreateOrgOpen}>
        <DialogContent showCloseButton={false} className="sm:max-w-[520px]">
          <DialogHeader>
            <DialogTitle>Créer une organisation</DialogTitle>
          </DialogHeader>
          <div className="grid gap-3 py-2">
            <div className="grid grid-cols-4 items-center gap-4">
              <Label className="text-right">Nom</Label>
              <Input className="col-span-3" value={createOrgForm.name} onChange={(e: ChangeEvent<HTMLInputElement>) => setCreateOrgForm((s) => ({ ...s, name: e.target.value }))} placeholder="Ex: ACME" />
            </div>
            <div className="grid grid-cols-4 items-center gap-4">
              <Label className="text-right">Slug</Label>
              <Input className="col-span-3" value={createOrgForm.slug} onChange={(e: ChangeEvent<HTMLInputElement>) => setCreateOrgForm((s) => ({ ...s, slug: e.target.value }))} placeholder="Ex: acme (optionnel)" />
            </div>
            {createOrgError ? <div className="text-sm text-red-600">{createOrgError}</div> : null}
          </div>
          <DialogFooter className="sm:justify-between">
            <Button variant="outline" onClick={() => setCreateOrgOpen(false)}>
              Annuler
            </Button>
            <Button onClick={createOrg} disabled={createOrgSaving}>
              {createOrgSaving ? "Création..." : "Créer"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <OrganizationMembersDialog
        open={orgMembersOpen}
        onOpenChange={(o) => {
          setOrgMembersOpen(o);
          if (!o) {
            // after role changes / leave, refresh local state
            void fetchMe().catch(() => null);
            void fetchOrgs().catch(() => null);
          }
        }}
        actorUserId={me?.user_id ?? null}
        actorOrgRole={currentOrgRole}
      />
    </>
  );
}


