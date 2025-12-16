"use client";

import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import { LayoutDashboard, Box, Settings, Activity, Archive, BarChart3, Server, Users } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { apiUrl } from "@/lib/api";
import { useCallback, useEffect, useMemo, useState } from "react";

interface SidebarLinkProps {
    href: string;
    icon: React.ElementType;
    label: string;
    disabled?: boolean;
}

function SidebarLink({ href, icon: Icon, label, disabled }: SidebarLinkProps) {
    const pathname = usePathname();
    const isActive = pathname === href;

    return (
        <Button
            variant={isActive ? "secondary" : "ghost"}
            className="w-full justify-start"
            asChild
            disabled={disabled}
        >
            <Link href={href}>
                <Icon className="mr-2 h-4 w-4" />
                {label}
            </Link>
        </Button>
    );
}

export function Sidebar() {
    const router = useRouter();
    const [menuOpen, setMenuOpen] = useState(false);
    const [profileOpen, setProfileOpen] = useState(false);
    const [me, setMe] = useState<{
        user_id: string;
        username: string;
        email: string;
        role: string;
        first_name?: string | null;
        last_name?: string | null;
    } | null>(null);

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

    const displayName = useMemo(() => {
        if (!me) return "User";
        const full = `${me.first_name ?? ""} ${me.last_name ?? ""}`.trim();
        return full || me.username || me.email || "User";
    }, [me]);

    const initials = useMemo(() => {
        const s = displayName.trim();
        if (!s) return "U";
        const parts = s.split(/\s+/).filter(Boolean);
        if (parts.length === 1) return parts[0].slice(0, 2).toUpperCase();
        return (parts[0][0] + parts[1][0]).toUpperCase();
    }, [displayName]);

    const fetchMe = useCallback(async () => {
        const res = await fetch(apiUrl("/auth/me"));
        if (!res.ok) {
            // Session invalid/expired (or DB reset) -> redirect to login.
            if (res.status === 401) {
                router.replace("/login");
            }
            setMe(null);
            return;
        }
        const data = await res.json();
        setMe(data);
        setProfileForm({
            username: data.username ?? (typeof data.email === "string" ? String(data.email).split("@")[0] : ""),
            email: data.email ?? "",
            first_name: data.first_name ?? "",
            last_name: data.last_name ?? "",
        });
    }, [router]);

    useEffect(() => {
        void fetchMe().catch(() => null);
    }, [fetchMe]);

    const logout = async () => {
        try {
            await fetch(apiUrl("/auth/logout"), { method: "POST" });
        } finally {
            router.replace("/login");
        }
    };

    const saveProfile = async () => {
        setProfileError(null);
        setProfileSaving(true);
        try {
            const res = await fetch(apiUrl("/auth/me"), {
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
                if (res.status === 401) {
                    router.replace("/login");
                    return;
                }
                const body = await res.json().catch(() => null);
                const code = body?.error || body?.message;
                setProfileError(
                    code === "conflict" || code === "username_or_email_already_exists"
                        ? "Username ou email déjà utilisé"
                        : code === "session_invalid"
                            ? "Session expirée, veuillez vous reconnecter"
                            : "Erreur lors de la mise à jour"
                );
                return;
            }
            const data = await res.json();
            setMe(data);
        } catch (e) {
            console.error(e);
            setProfileError("Erreur réseau");
        } finally {
            setProfileSaving(false);
        }
    };

    const changePassword = async () => {
        setPwdError(null);
        setPwdSuccess(null);
        if (!pwdForm.current_password.trim() || !pwdForm.new_password.trim()) {
            setPwdError("Veuillez remplir tous les champs");
            return;
        }
        if (pwdForm.new_password !== pwdForm.confirm_new_password) {
            setPwdError("La confirmation ne correspond pas");
            return;
        }
        setPwdSaving(true);
        try {
            const res = await fetch(apiUrl("/auth/me/password"), {
                method: "PUT",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                    current_password: pwdForm.current_password,
                    new_password: pwdForm.new_password,
                }),
            });
            if (!res.ok) {
                if (res.status === 401) {
                    router.replace("/login");
                    return;
                }
                const body = await res.json().catch(() => null);
                const code = body?.error || body?.message;
                setPwdError(
                    code === "invalid_current_password" || code === "current_password_invalid"
                        ? "Mot de passe actuel incorrect"
                        : code === "session_invalid"
                            ? "Session expirée, veuillez vous reconnecter"
                            : "Erreur lors du changement de mot de passe"
                );
                return;
            }
            setPwdForm({ current_password: "", new_password: "", confirm_new_password: "" });
            setPwdSuccess("Mot de passe mis à jour");
        } catch (e) {
            console.error(e);
            setPwdError("Erreur réseau");
        } finally {
            setPwdSaving(false);
        }
    };

    return (
        <div className="w-64 border-r min-h-screen bg-background text-foreground hidden md:flex flex-col">
            <div className="space-y-4 py-4 flex-1">
                <div className="px-3 py-2">
                    <h2 className="mb-2 px-4 text-lg font-semibold tracking-tight text-primary">
                        Inventiv Agents
                    </h2>
                    <div className="space-y-1">
                        <SidebarLink href="/" icon={LayoutDashboard} label="Dashboard" />
                        <SidebarLink href="/instances" icon={Server} label="Instances" />
                        <SidebarLink href="/monitoring" icon={BarChart3} label="Monitoring" />
                        <SidebarLink href="/models" icon={Box} label="Models" />
                        <SidebarLink href="/settings" icon={Settings} label="Settings" />
                        <SidebarLink href="/users" icon={Users} label="Users" />
                    </div>
                </div>
                <div className="px-3 py-2">
                    <h2 className="mb-2 px-4 text-xs font-semibold tracking-tight text-muted-foreground uppercase">
                        History
                    </h2>
                    <div className="space-y-1">
                        <SidebarLink href="/traces" icon={Archive} label="Traces" />
                    </div>
                </div>
                <div className="px-3 py-2">
                    <h2 className="mb-2 px-4 text-xs font-semibold tracking-tight text-muted-foreground uppercase">
                        System
                    </h2>
                    <div className="space-y-1">
                        <Button variant="ghost" className="w-full justify-start" disabled>
                            <Activity className="mr-2 h-4 w-4" />
                            System Status
                        </Button>
                    </div>
                </div>
            </div>

            {/* User chip (bottom) */}
            <div className="p-3 border-t">
                <Button
                    variant="ghost"
                    className="w-full justify-start"
                    onClick={() => setMenuOpen(true)}
                >
                    <div className="h-8 w-8 rounded-full bg-muted flex items-center justify-center font-semibold text-xs mr-2">
                        {initials}
                    </div>
                    <div className="min-w-0 flex-1 text-left">
                        <div className="text-sm font-medium truncate">{displayName}</div>
                        <div className="text-xs text-muted-foreground truncate">{me?.role ?? "user"}</div>
                    </div>
                </Button>
            </div>

            {/* Menu dialog */}
            <Dialog open={menuOpen} onOpenChange={setMenuOpen}>
                <DialogContent showCloseButton={false} className="sm:max-w-[420px]">
                    <DialogHeader>
                        <DialogTitle>Compte</DialogTitle>
                    </DialogHeader>
                    <div className="grid gap-2 py-2">
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
                onOpenChange={(o) => {
                    setProfileOpen(o);
                    if (o) void fetchMe().catch(() => null);
                }}
            >
                <DialogContent showCloseButton={false} className="sm:max-w-[560px]">
                    <DialogHeader>
                        <DialogTitle>Mon profil</DialogTitle>
                    </DialogHeader>

                    <div className="grid gap-6 py-2">
                        <div className="grid gap-3">
                            <div className="grid grid-cols-4 items-center gap-4">
                                <Label className="text-right">Username</Label>
                                <Input
                                    className="col-span-3"
                                    value={profileForm.username}
                                    onChange={(e) => setProfileForm((s) => ({ ...s, username: e.target.value }))}
                                />
                            </div>
                            <div className="grid grid-cols-4 items-center gap-4">
                                <Label className="text-right">Email</Label>
                                <Input
                                    className="col-span-3"
                                    value={profileForm.email}
                                    onChange={(e) => setProfileForm((s) => ({ ...s, email: e.target.value }))}
                                />
                            </div>
                            <div className="grid grid-cols-4 items-center gap-4">
                                <Label className="text-right">Prénom</Label>
                                <Input
                                    className="col-span-3"
                                    value={profileForm.first_name}
                                    onChange={(e) => setProfileForm((s) => ({ ...s, first_name: e.target.value }))}
                                />
                            </div>
                            <div className="grid grid-cols-4 items-center gap-4">
                                <Label className="text-right">Nom</Label>
                                <Input
                                    className="col-span-3"
                                    value={profileForm.last_name}
                                    onChange={(e) => setProfileForm((s) => ({ ...s, last_name: e.target.value }))}
                                />
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
                                <Input
                                    className="col-span-3"
                                    type="password"
                                    value={pwdForm.current_password}
                                    onChange={(e) => setPwdForm((s) => ({ ...s, current_password: e.target.value }))}
                                />
                            </div>
                            <div className="grid grid-cols-4 items-center gap-4">
                                <Label className="text-right">Nouveau</Label>
                                <Input
                                    className="col-span-3"
                                    type="password"
                                    value={pwdForm.new_password}
                                    onChange={(e) => setPwdForm((s) => ({ ...s, new_password: e.target.value }))}
                                />
                            </div>
                            <div className="grid grid-cols-4 items-center gap-4">
                                <Label className="text-right">Confirmer</Label>
                                <Input
                                    className="col-span-3"
                                    type="password"
                                    value={pwdForm.confirm_new_password}
                                    onChange={(e) => setPwdForm((s) => ({ ...s, confirm_new_password: e.target.value }))}
                                />
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
        </div>
    );
}
