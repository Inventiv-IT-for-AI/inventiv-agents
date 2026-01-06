"use client";

import { useState, useEffect, type ChangeEvent } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { apiRequest } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { IAAlert, IAAlertDescription, IAAlertTitle } from "ia-designsys";
import Link from "next/link";

export default function ResetPasswordPage() {
  const router = useRouter();
  const searchParams = useSearchParams();
  const [token, setToken] = useState("");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);

  useEffect(() => {
    const tokenParam = searchParams.get("token");
    if (tokenParam) {
      // Decode URL-encoded token (handles +, /, = characters from base64)
      const decodedToken = decodeURIComponent(tokenParam);
      setToken(decodedToken);
    } else {
      setError("Token manquant dans l'URL");
    }
  }, [searchParams]);

  const onSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    if (password.length < 8) {
      setError("Le mot de passe doit contenir au moins 8 caractères");
      return;
    }

    if (password !== confirmPassword) {
      setError("Les mots de passe ne correspondent pas");
      return;
    }

    if (!token) {
      setError("Token manquant");
      return;
    }

    setLoading(true);
    try {
      const res = await apiRequest("/auth/password-reset/reset", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ token, new_password: password }),
      });

      if (!res.ok) {
        const errorData = await res.json().catch(() => ({}));
        const errorMessage =
          errorData.message ||
          (errorData.error === "invalid_token"
            ? "Token invalide ou expiré"
            : "Erreur lors de la réinitialisation");
        setError(errorMessage);
        return;
      }

      setSuccess(true);
      // Redirect to login after 3 seconds
      setTimeout(() => {
        router.push("/login?message=password_reset_success");
      }, 3000);
    } catch (e) {
      console.error(e);
      setError("Erreur réseau");
    } finally {
      setLoading(false);
    }
  };

  if (success) {
    return (
      <div className="min-h-screen flex items-center justify-center p-6 bg-background">
        <Card className="w-full max-w-md">
          <CardHeader>
            <CardTitle>Mot de passe réinitialisé</CardTitle>
            <CardDescription>Votre mot de passe a été réinitialisé avec succès.</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <IAAlert>
              <IAAlertTitle>Succès</IAAlertTitle>
              <IAAlertDescription>
                Votre mot de passe a été modifié. Vous allez être redirigé vers la page de connexion...
              </IAAlertDescription>
            </IAAlert>
            <Button className="w-full" onClick={() => router.push("/login")}>
              Aller à la connexion
            </Button>
          </CardContent>
        </Card>
      </div>
    );
  }

  return (
    <div className="min-h-screen flex items-center justify-center p-6 bg-background">
      <Card className="w-full max-w-md">
        <CardHeader>
          <CardTitle>Réinitialiser mon mot de passe</CardTitle>
          <CardDescription>Entrez votre nouveau mot de passe.</CardDescription>
        </CardHeader>
        <CardContent>
          <form onSubmit={onSubmit} className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="password">Nouveau mot de passe</Label>
              <Input
                id="password"
                type="password"
                value={password}
                onChange={(e: ChangeEvent<HTMLInputElement>) => setPassword(e.target.value)}
                placeholder="Minimum 8 caractères"
                autoComplete="new-password"
                autoFocus
                required
                disabled={loading || !token}
                minLength={8}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="confirmPassword">Confirmer le mot de passe</Label>
              <Input
                id="confirmPassword"
                type="password"
                value={confirmPassword}
                onChange={(e: ChangeEvent<HTMLInputElement>) => setConfirmPassword(e.target.value)}
                placeholder="Répétez le mot de passe"
                autoComplete="new-password"
                required
                disabled={loading || !token}
                minLength={8}
              />
            </div>
            {error ? (
              <IAAlert variant="destructive">
                <IAAlertTitle>Erreur</IAAlertTitle>
                <IAAlertDescription>{error}</IAAlertDescription>
              </IAAlert>
            ) : null}
            {!token ? (
              <IAAlert variant="destructive">
                <IAAlertTitle>Token manquant</IAAlertTitle>
                <IAAlertDescription>
                  Le lien de réinitialisation est invalide. Veuillez demander un nouveau lien.
                </IAAlertDescription>
              </IAAlert>
            ) : null}
            <Button className="w-full" type="submit" disabled={loading || !token}>
              {loading ? "Réinitialisation..." : "Réinitialiser le mot de passe"}
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

