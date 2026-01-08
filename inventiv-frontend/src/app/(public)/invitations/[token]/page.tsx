"use client";

import { useEffect, useState } from "react";
import { useParams, useRouter } from "next/navigation";
import { useOrganizationInvitations } from "@/hooks/useOrganizationInvitations";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { IAAlert, IAAlertDescription, IAAlertTitle } from "ia-designsys";
import { Building2, CheckCircle2, AlertCircle, Clock, Mail } from "lucide-react";
import Link from "next/link";
import { apiRequest } from "@/lib/api";

type InvitationDetails = {
  id: string;
  organization_id: string;
  organization_name: string;
  email: string;
  role: string;
  expires_at: string;
  accepted_at?: string | null;
  created_at: string;
};

export default function AcceptInvitationPage() {
  const params = useParams();
  const router = useRouter();
  const token = params.token as string;
  const { acceptInvitation, loading, error } = useOrganizationInvitations();

  const [invitation, setInvitation] = useState<InvitationDetails | null>(null);
  const [fetching, setFetching] = useState(true);
  const [accepting, setAccepting] = useState(false);
  const [currentUserEmail, setCurrentUserEmail] = useState<string | null>(null);
  const [isLoggedIn, setIsLoggedIn] = useState(false);

  useEffect(() => {
    // Check if user is logged in
    const checkAuth = async () => {
      try {
        const res = await apiRequest("/auth/me");
        if (res.ok) {
          const data = await res.json();
          setCurrentUserEmail(data.email || null);
          setIsLoggedIn(true);
        } else {
          setIsLoggedIn(false);
        }
      } catch {
        setIsLoggedIn(false);
      }
    };
    void checkAuth();
  }, []);

  useEffect(() => {
    const fetchInvitation = async () => {
      if (!token) return;
      setFetching(true);
      try {
        // We need to fetch invitation details by token
        // Since we don't have a public endpoint, we'll try to accept it directly
        // But first, let's try to get details via a workaround
        // Actually, we should create a public endpoint for this, but for now we'll handle it in the accept flow
        setFetching(false);
      } catch (e) {
        console.error(e);
        setFetching(false);
      }
    };
    void fetchInvitation();
  }, [token]);

  const handleAccept = async () => {
    if (!token) return;
    setAccepting(true);
    try {
      await acceptInvitation(token);
      // Redirect to organizations page after a short delay
      setTimeout(() => {
        router.push("/organizations");
      }, 2000);
    } catch (e) {
      console.error(e);
      setAccepting(false);
    }
  };

  const isExpired = invitation ? new Date(invitation.expires_at) < new Date() : false;
  const isAccepted = invitation?.accepted_at !== null && invitation?.accepted_at !== undefined;
  const emailMatches = currentUserEmail?.toLowerCase() === invitation?.email.toLowerCase();

  // For now, we'll show a simple UI that tries to accept the invitation
  // In a real implementation, we'd fetch invitation details first via a public endpoint

  return (
    <div className="min-h-screen flex items-center justify-center bg-background p-4">
      <Card className="w-full max-w-md">
        <CardHeader className="text-center">
          <div className="mx-auto mb-4 flex h-12 w-12 items-center justify-center rounded-full bg-primary/10">
            <Building2 className="h-6 w-6 text-primary" />
          </div>
          <CardTitle>Invitation à rejoindre une organisation</CardTitle>
          <CardDescription>
            {fetching ? "Chargement..." : "Vous avez été invité à rejoindre une organisation"}
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          {fetching ? (
            <div className="text-center text-sm text-muted-foreground">Vérification de l&apos;invitation...</div>
          ) : error ? (
            <IAAlert variant="destructive">
              <IAAlertTitle>Erreur</IAAlertTitle>
              <IAAlertDescription>{error}</IAAlertDescription>
            </IAAlert>
          ) : isAccepted ? (
            <IAAlert>
              <IAAlertTitle>Invitation déjà acceptée</IAAlertTitle>
              <IAAlertDescription>
                Cette invitation a déjà été acceptée. Vous pouvez accéder à l&apos;organisation depuis votre tableau de bord.
              </IAAlertDescription>
            </IAAlert>
          ) : isExpired ? (
            <IAAlert variant="destructive">
              <IAAlertTitle>Invitation expirée</IAAlertTitle>
              <IAAlertDescription>
                Cette invitation a expiré. Veuillez demander une nouvelle invitation à l&apos;administrateur de
                l&apos;organisation.
              </IAAlertDescription>
            </IAAlert>
          ) : !isLoggedIn ? (
            <IAAlert>
              <IAAlertTitle>Connexion requise</IAAlertTitle>
              <IAAlertDescription>
                Vous devez être connecté pour accepter cette invitation. Veuillez vous connecter avec l&apos;email
                associé à cette invitation.
              </IAAlertDescription>
            </IAAlert>
          ) : !emailMatches && invitation ? (
            <IAAlert variant="destructive">
              <IAAlertTitle>Email ne correspond pas</IAAlertTitle>
              <IAAlertDescription>
                Cette invitation est destinée à <strong>{invitation.email}</strong>, mais vous êtes connecté avec{" "}
                <strong>{currentUserEmail}</strong>. Veuillez vous connecter avec le bon compte.
              </IAAlertDescription>
            </IAAlert>
          ) : (
            <>
              {invitation && (
                <div className="space-y-2">
                  <div className="flex items-center justify-between">
                    <span className="text-sm text-muted-foreground">Organisation:</span>
                    <span className="text-sm font-medium">{invitation.organization_name}</span>
                  </div>
                  <div className="flex items-center justify-between">
                    <span className="text-sm text-muted-foreground">Rôle:</span>
                    <Badge variant="secondary">{invitation.role}</Badge>
                  </div>
                  <div className="flex items-center justify-between">
                    <span className="text-sm text-muted-foreground">Expire le:</span>
                    <span className="text-sm">{new Date(invitation.expires_at).toLocaleDateString("fr-FR")}</span>
                  </div>
                </div>
              )}
              <Button
                onClick={handleAccept}
                disabled={accepting || !isLoggedIn || (invitation && !emailMatches)}
                className="w-full"
              >
                {accepting ? (
                  <>
                    <Clock className="mr-2 h-4 w-4 animate-spin" />
                    Acceptation...
                  </>
                ) : (
                  <>
                    <CheckCircle2 className="mr-2 h-4 w-4" />
                    Accepter l&apos;invitation
                  </>
                )}
              </Button>
            </>
          )}

          {!isLoggedIn && (
            <div className="text-center">
              <Button variant="outline" asChild className="w-full">
                <Link href={`/login?redirect=/invitations/${token}`}>Se connecter</Link>
              </Button>
            </div>
          )}

          {isLoggedIn && (
            <div className="text-center">
              <Button variant="ghost" asChild>
                <Link href="/organizations">Retour aux organisations</Link>
              </Button>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
}

