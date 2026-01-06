"use client";

import { useState, type ChangeEvent } from "react";
import { useRouter } from "next/navigation";
import { apiRequest } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { IAAlert, IAAlertDescription, IAAlertTitle } from "ia-designsys";
import Link from "next/link";

export default function ForgotPasswordPage() {
  const router = useRouter();
  const [email, setEmail] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);

  const onSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setLoading(true);
    try {
      const res = await apiRequest("/auth/password-reset/request", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ email }),
      });

      if (!res.ok) {
        const errorData = await res.json().catch(() => ({}));
        setError(errorData.message || "Erreur lors de la demande de réinitialisation");
        return;
      }

      const data = await res.json();
      setSuccess(true);
    } catch (e) {
      console.error("Password reset request error:", e);
      const errorMessage = e instanceof Error ? e.message : String(e);
      if (errorMessage.includes("fetch failed") || errorMessage.includes("Failed to fetch")) {
        setError("Impossible de contacter le serveur. Vérifiez que l'API est démarrée et accessible.");
      } else {
        setError(errorMessage || "Erreur réseau");
      }
    } finally {
      setLoading(false);
    }
  };

  if (success) {
    return (
      <div className="min-h-screen flex items-center justify-center p-6 bg-background">
        <Card className="w-full max-w-md">
          <CardHeader>
            <CardTitle>Email envoyé</CardTitle>
            <CardDescription>
              Si cette adresse email existe, un email de réinitialisation a été envoyé.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <IAAlert>
              <IAAlertTitle>Vérifiez votre boîte mail</IAAlertTitle>
              <IAAlertDescription>
                Un email avec un lien de réinitialisation a été envoyé à <strong>{email}</strong>.
                Le lien est valide pendant 1 heure.
              </IAAlertDescription>
            </IAAlert>
            <div className="text-sm text-muted-foreground">
              <p>Si vous ne recevez pas l'email :</p>
              <ul className="list-disc list-inside mt-2 space-y-1">
                <li>Vérifiez votre dossier spam</li>
                <li>Vérifiez que l'adresse email est correcte</li>
                <li>Attendez quelques minutes</li>
              </ul>
            </div>
            <div className="flex gap-2">
              <Button variant="outline" className="flex-1" onClick={() => router.push("/login")}>
                Retour à la connexion
              </Button>
              <Button
                variant="outline"
                className="flex-1"
                onClick={() => {
                  setSuccess(false);
                  setEmail("");
                }}
              >
                Réessayer
              </Button>
            </div>
          </CardContent>
        </Card>
      </div>
    );
  }

  return (
    <div className="min-h-screen flex items-center justify-center p-6 bg-background">
      <Card className="w-full max-w-md">
        <CardHeader>
          <CardTitle>J'ai oublié mon mot de passe</CardTitle>
          <CardDescription>
            Entrez votre adresse email pour recevoir un lien de réinitialisation.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <form onSubmit={onSubmit} className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="email">Email</Label>
              <Input
                id="email"
                type="email"
                value={email}
                onChange={(e: ChangeEvent<HTMLInputElement>) => setEmail(e.target.value)}
                placeholder="votre@email.com"
                autoComplete="email"
                autoFocus
                required
                disabled={loading}
              />
            </div>
            {error ? (
              <IAAlert variant="destructive">
                <IAAlertTitle>Erreur</IAAlertTitle>
                <IAAlertDescription>{error}</IAAlertDescription>
              </IAAlert>
            ) : null}
            <Button className="w-full" type="submit" disabled={loading}>
              {loading ? "Envoi..." : "Envoyer le lien de réinitialisation"}
            </Button>
            <div className="text-center text-sm">
              <Link href="/login" className="text-primary hover:underline">
                Retour à la connexion
              </Link>
            </div>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}

