"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { LayoutDashboard, Settings, Activity, Archive, BarChart3, Server, Users, Terminal, KeyRound, Cpu, MessageSquare } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useState } from "react";
import type { Me } from "@/components/account/AccountSection";
import { AccountSection } from "@/components/account/AccountSection";

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
    const isAdmin = meRole === "admin";

    return (
        <div className="w-64 border-r min-h-screen bg-background text-foreground hidden md:flex flex-col">
            <div className="space-y-4 py-4 flex-1">
                <div className="px-3 py-2">
                    <h2 className="mb-2 px-4 text-lg font-semibold tracking-tight text-primary">
                        Inventiv Agents
                    </h2>
                    <div className="space-y-1">
                        <SidebarLink href="/" icon={LayoutDashboard} label="Dashboard" />
                        <SidebarLink href="/chat" icon={MessageSquare} label="Chat" />
                        <SidebarLink href="/instances" icon={Server} label="Instances" />
                        <SidebarLink href="/models" icon={Activity} label="Models" />
                        <SidebarLink href="/gpu-activity" icon={Cpu} label="GPU Activity" />
                        <SidebarLink href="/workbench" icon={Terminal} label="Workbench" />
                        <SidebarLink href="/monitoring" icon={BarChart3} label="Monitoring" />
                        <SidebarLink href="/api-keys" icon={KeyRound} label="API Keys" />
                        {isAdmin ? <SidebarLink href="/settings" icon={Settings} label="Settings" /> : null}
                        {isAdmin ? <SidebarLink href="/users" icon={Users} label="Users" /> : null}
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
