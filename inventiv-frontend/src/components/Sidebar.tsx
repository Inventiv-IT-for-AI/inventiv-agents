"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { LayoutDashboard, Settings, Activity, Archive, BarChart3, Server, Users, Terminal, KeyRound, Cpu, MessageSquare, Building2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { useState, useEffect } from "react";
import type { Me } from "@/components/account/AccountSection";
import { AccountSection } from "@/components/account/AccountSection";
import { FRONTEND_VERSION, getBackendVersion, type BackendVersionInfo } from "@/lib/version";
import { useInstanceAccess } from "@/hooks/useInstanceAccess";
import { useAdminDashboardAccess } from "@/hooks/useAdminDashboardAccess";

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
    const [meRole, setMeRole] = useState<string | null>(null);
    const [backendVersion, setBackendVersion] = useState<BackendVersionInfo | null>(null);
    const [showVersionDetails, setShowVersionDetails] = useState(false);
    const isAdmin = meRole === "admin";
    const instanceAccess = useInstanceAccess();
    const adminDashboardAccess = useAdminDashboardAccess();

    useEffect(() => {
        // Fetch backend version on mount
        getBackendVersion().then(setBackendVersion).catch(() => {});
    }, []);

    return (
        <div className="w-64 border-r min-h-screen bg-background text-foreground hidden md:flex flex-col">
            <div className="space-y-4 py-4 flex-1">
                <div className="px-3 py-2">
                    <div className="mb-2 px-4">
                        <h2 className="text-lg font-semibold tracking-tight text-primary">
                            Inventiv Agents
                        </h2>
                        <div className="relative mt-0.5">
                            <button
                                type="button"
                                className="group relative cursor-pointer"
                                onMouseEnter={() => setShowVersionDetails(true)}
                                onMouseLeave={() => setShowVersionDetails(false)}
                                onClick={() => setShowVersionDetails(!showVersionDetails)}
                                aria-label="Version information"
                            >
                                <Badge variant="secondary" className="text-[10px] font-mono px-1.5 py-0.5 h-4 leading-none opacity-70 hover:opacity-100 transition-opacity">
                                    v{FRONTEND_VERSION}
                                </Badge>
                                {showVersionDetails && (
                                    <div className="absolute left-0 top-5 z-50 w-56 rounded-md border bg-popover p-3 text-popover-foreground shadow-md animate-in fade-in-0 zoom-in-95">
                                        <div className="space-y-1.5 text-xs">
                                            <div className="font-semibold text-[10px] uppercase tracking-wide text-muted-foreground mb-2">
                                                Version Info
                                            </div>
                                            <div className="flex justify-between items-center">
                                                <span className="text-muted-foreground">Frontend:</span>
                                                <span className="font-mono font-medium">{FRONTEND_VERSION}</span>
                                            </div>
                                            {backendVersion ? (
                                                <>
                                                    <div className="flex justify-between items-center">
                                                        <span className="text-muted-foreground">Backend:</span>
                                                        <span className="font-mono font-medium">{backendVersion.backend_version}</span>
                                                    </div>
                                                    <div className="flex justify-between items-center pt-1 border-t">
                                                        <span className="text-muted-foreground">Build:</span>
                                                        <span className="font-mono text-[10px]">{backendVersion.build_time}</span>
                                                    </div>
                                                </>
                                            ) : (
                                                <div className="text-muted-foreground text-[10px] italic">
                                                    Loading backend version...
                                                </div>
                                            )}
                                        </div>
                                    </div>
                                )}
                            </button>
                        </div>
                    </div>
                    <div className="space-y-1">
                        <SidebarLink href="/my-dashboard" icon={LayoutDashboard} label="My Dashboard" />
                        <SidebarLink href="/chat" icon={MessageSquare} label="Chat" />
                        {!instanceAccess.loading && instanceAccess.canAccess && (
                            <SidebarLink href="/instances" icon={Server} label="Instances" />
                        )}
                        <SidebarLink href="/models" icon={Activity} label="Models" />
                        <SidebarLink href="/observability" icon={Cpu} label="Observability" />
                        <SidebarLink href="/workbench" icon={Terminal} label="Workbench" />
                        <SidebarLink href="/monitoring" icon={BarChart3} label="Monitoring" />
                        <SidebarLink href="/api-keys" icon={KeyRound} label="API Keys" />
                        <SidebarLink href="/organizations" icon={Building2} label="Organizations" />
                        {!adminDashboardAccess.loading && adminDashboardAccess.canAccess && (
                            <SidebarLink href="/admin-dashboard" icon={LayoutDashboard} label="Admin Dashboard" />
                        )}
                        {isAdmin ? <SidebarLink href="/users" icon={Users} label="Users" /> : null}
                        {isAdmin ? <SidebarLink href="/settings" icon={Settings} label="Settings" /> : null}
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

            <AccountSection onMeChange={(m: Me | null) => setMeRole(m?.role ?? null)} />
        </div>
    );
}
