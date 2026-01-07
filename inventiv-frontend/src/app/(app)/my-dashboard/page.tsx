"use client";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useMyDashboard } from "@/hooks/useMyDashboard";
import { formatEur } from "@/lib/utils";
import {
  User,
  Wallet,
  MessageSquare,
  Cpu,
  TrendingUp,
  Building2,
  CreditCard,
  Sparkles,
  Clock,
  ArrowRight,
  Loader2,
} from "lucide-react";
import Link from "next/link";
import { WorkspaceBanner } from "@/components/shared/WorkspaceBanner";
import { IAStatCell } from "ia-widgets";

export default function MyDashboardPage() {
  const data = useMyDashboard();

  if (data.loading) {
    return (
      <div className="p-8 space-y-8">
        <div className="flex items-center justify-center min-h-[400px]">
          <div className="flex flex-col items-center gap-3">
            <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
            <p className="text-sm text-muted-foreground">Chargement de vos données...</p>
          </div>
        </div>
      </div>
    );
  }

  if (data.error) {
    return (
      <div className="p-8 space-y-8">
        <div className="rounded-lg border border-destructive bg-destructive/10 p-4">
          <p className="text-sm text-destructive">{data.error}</p>
        </div>
      </div>
    );
  }

  return (
    <div className="p-8 space-y-8">
      {/* Header */}
      <div>
        <h1 className="text-3xl font-bold tracking-tight">Mon Dashboard</h1>
        <p className="text-muted-foreground">
          Vue d&apos;ensemble de votre compte et de votre activité
        </p>
      </div>

      <WorkspaceBanner />

      {/* Account & Subscription Overview */}
      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
        <IAStatCell
          title="Mon Compte"
          value={data.accountPlan === "subscriber" ? "Abonné" : "Gratuit"}
          subtitle={data.walletBalanceEur !== null ? `Solde: ${formatEur(data.walletBalanceEur, { minFrac: 2, maxFrac: 2 })}` : "Plan gratuit"}
          icon={User}
          accent="blue"
        />

        {data.organizationName ? (
          <IAStatCell
            title="Organisation"
            value={data.organizationName.length > 15 ? `${data.organizationName.substring(0, 15)}...` : data.organizationName}
            subtitle={data.organizationPlan === "subscriber" 
              ? (data.organizationWalletBalanceEur !== null 
                  ? `Abonné • ${formatEur(data.organizationWalletBalanceEur, { minFrac: 2, maxFrac: 2 })}`
                  : "Plan abonné")
              : (data.organizationWalletBalanceEur !== null 
                  ? `Gratuit • ${formatEur(data.organizationWalletBalanceEur, { minFrac: 2, maxFrac: 2 })}`
                  : "Plan gratuit")}
            icon={Building2}
            accent="purple"
          />
        ) : (
          <IAStatCell
            title="Organisation"
            value="—"
            subtitle="Aucune organisation"
            icon={Building2}
            accent="purple"
          />
        )}

        <IAStatCell
          title="Sessions de Chat"
          value={String(data.totalChatSessions)}
          subtitle={data.recentChatSessions.length > 0 ? `${data.recentChatSessions.length} récentes` : "Aucune session"}
          icon={MessageSquare}
          accent="green"
        />

        <IAStatCell
          title="Models Accessibles"
          value={String(data.accessibleModels.length)}
          subtitle={data.accessibleModels.length > 0 ? "Prêts à utiliser" : "Aucun modèle"}
          icon={Cpu}
          accent="amber"
        />
      </div>

      {/* Recent Chat Sessions */}
      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <CardTitle className="flex items-center gap-2">
              <MessageSquare className="h-5 w-5" />
              Sessions de Chat Récentes
            </CardTitle>
            <Button variant="outline" size="sm" asChild>
              <Link href="/chat">
                Voir tout <ArrowRight className="ml-2 h-4 w-4" />
              </Link>
            </Button>
          </div>
        </CardHeader>
        <CardContent>
          {data.recentChatSessions.length === 0 ? (
            <div className="text-center py-8 text-muted-foreground">
              <MessageSquare className="h-12 w-12 mx-auto mb-3 opacity-50" />
              <p className="text-sm">Aucune session de chat</p>
              <Button variant="outline" size="sm" className="mt-4" asChild>
                <Link href="/chat">Créer une session</Link>
              </Button>
            </div>
          ) : (
            <div className="space-y-3">
              {data.recentChatSessions.map((session) => (
                <div
                  key={session.id}
                  className="flex items-center justify-between p-4 border rounded-lg hover:bg-muted/50 transition-colors group"
                >
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2">
                      <p className="font-medium text-sm truncate">
                        {session.title || "Session sans titre"}
                      </p>
                      {session.model_id && (
                        <Badge variant="outline" className="text-xs">
                          {session.model_id}
                        </Badge>
                      )}
                    </div>
                    <div className="flex items-center gap-3 mt-1 text-xs text-muted-foreground">
                      <span className="flex items-center gap-1">
                        <Clock className="h-3 w-3" />
                        {new Date(session.updated_at).toLocaleDateString("fr-FR", {
                          day: "numeric",
                          month: "short",
                          hour: "2-digit",
                          minute: "2-digit",
                        })}
                      </span>
                    </div>
                  </div>
                  <Button variant="ghost" size="sm" asChild className="opacity-0 group-hover:opacity-100 transition-opacity">
                    <Link href={`/chat?run=${session.id}`}>
                      Ouvrir <ArrowRight className="ml-1 h-3 w-3" />
                    </Link>
                  </Button>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      {/* Accessible Models */}
      <Card>
        <CardHeader>
          <div className="flex items-center justify-between">
            <CardTitle className="flex items-center gap-2">
              <Cpu className="h-5 w-5" />
              Models Accessibles
            </CardTitle>
            <Button variant="outline" size="sm" asChild>
              <Link href="/models">
                Explorer <ArrowRight className="ml-2 h-4 w-4" />
              </Link>
            </Button>
          </div>
        </CardHeader>
        <CardContent>
          {data.accessibleModels.length === 0 ? (
            <div className="text-center py-8 text-muted-foreground">
              <Cpu className="h-12 w-12 mx-auto mb-3 opacity-50" />
              <p className="text-sm">Aucun modèle disponible</p>
              <p className="text-xs mt-1">Les modèles apparaîtront ici lorsqu&apos;ils seront disponibles</p>
            </div>
          ) : (
            <div className="grid gap-3 md:grid-cols-2 lg:grid-cols-3">
              {data.accessibleModels.slice(0, 6).map((model) => (
                <div
                  key={model.model}
                  className="p-4 border rounded-lg hover:bg-muted/50 transition-colors"
                >
                  <div className="flex items-start justify-between">
                    <div className="flex-1 min-w-0">
                      <p className="font-medium text-sm truncate">{model.label}</p>
                      <p className="text-xs text-muted-foreground mt-1 truncate">{model.model}</p>
                      {model.scope && (
                        <Badge variant="outline" className="mt-2 text-xs">
                          {model.scope === "public" ? "Public" : model.scope === "org" ? "Organisation" : model.scope}
                        </Badge>
                      )}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      {/* Quick Actions */}
      <div className="grid gap-4 md:grid-cols-3">
        <Card className="border-2 hover:border-primary/50 transition-colors cursor-pointer">
          <CardContent className="p-6">
            <Link href="/chat" className="block">
              <div className="flex items-center gap-3">
                <div className="p-3 rounded-lg bg-primary/10">
                  <MessageSquare className="h-6 w-6 text-primary" />
                </div>
                <div>
                  <p className="font-semibold">Nouvelle Session</p>
                  <p className="text-xs text-muted-foreground">Démarrer un chat</p>
                </div>
              </div>
            </Link>
          </CardContent>
        </Card>

        <Card className="border-2 hover:border-primary/50 transition-colors cursor-pointer">
          <CardContent className="p-6">
            <Link href="/models" className="block">
              <div className="flex items-center gap-3">
                <div className="p-3 rounded-lg bg-primary/10">
                  <Cpu className="h-6 w-6 text-primary" />
                </div>
                <div>
                  <p className="font-semibold">Explorer les Models</p>
                  <p className="text-xs text-muted-foreground">Découvrir les modèles disponibles</p>
                </div>
              </div>
            </Link>
          </CardContent>
        </Card>

        <Card className="border-2 hover:border-primary/50 transition-colors cursor-pointer">
          <CardContent className="p-6">
            <Link href="/organizations" className="block">
              <div className="flex items-center gap-3">
                <div className="p-3 rounded-lg bg-primary/10">
                  <Building2 className="h-6 w-6 text-primary" />
                </div>
                <div>
                  <p className="font-semibold">Organisations</p>
                  <p className="text-xs text-muted-foreground">Gérer vos organisations</p>
                </div>
              </div>
            </Link>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}

