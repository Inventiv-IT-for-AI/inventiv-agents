"use client";

import { useState, type ChangeEvent } from "react";
import { useRouter } from "next/navigation";
import { apiUrl } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

export default function LoginPage() {
  const router = useRouter();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const onSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setLoading(true);
    try {
      // Use /api/backend proxy so cookies are properly set
      const res = await fetch("/api/backend/auth/login", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        credentials: "include", // Important: include cookies
        body: JSON.stringify({ email, password }),
      });
      if (!res.ok) {
        const errorData = await res.json().catch(() => ({}));
        setError(errorData.message || "Identifiants invalides");
        return;
      }
      // Note: The session cookie is HttpOnly (for security), so it's not accessible
      // via document.cookie. We trust that if the response is 200 OK, the cookie
      // was set by the server. The browser will automatically include it in subsequent requests.
      
      // Verify the response contains user data (indicates successful login)
      const loginData = await res.json().catch(() => null);
      if (!loginData || !loginData.user_id) {
        console.error("[Login] Invalid response data:", loginData);
        setError("Erreur: réponse invalide du serveur");
        return;
      }
      
      console.log("[Login] Login successful for user:", loginData.email);
      
      // Wait a bit for the browser to process the Set-Cookie header
      // Even though we can't read HttpOnly cookies, we give the browser time to set it
      await new Promise((resolve) => setTimeout(resolve, 500));
      
      // Get redirect URL from query params or default to "/"
      const redirectUrl = new URLSearchParams(window.location.search).get("redirect") || "/";
      
      // Use window.location.href instead of router to ensure full page reload
      // This ensures the middleware runs again and sees the new cookie
      // Use replace instead of href to avoid adding to history
      window.location.replace(redirectUrl);
    } catch (e) {
      console.error(e);
      setError("Erreur réseau");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen flex items-center justify-center p-6 bg-background">
      <Card className="w-full max-w-md">
        <CardHeader>
          <CardTitle>Connexion</CardTitle>
        </CardHeader>
        <CardContent>
          <form onSubmit={onSubmit} className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="email">Email ou username</Label>
              <Input
                id="email"
                type="text"
                value={email}
                onChange={(e: ChangeEvent<HTMLInputElement>) => setEmail(e.target.value)}
                placeholder="admin ou admin@inventiv.local"
                autoComplete="username"
                autoFocus
                required
                disabled={loading}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="password">Mot de passe</Label>
              <Input
                id="password"
                type="password"
                value={password}
                onChange={(e: ChangeEvent<HTMLInputElement>) => setPassword(e.target.value)}
                autoComplete="current-password"
                required
                disabled={loading}
              />
            </div>
            {error ? <div className="text-sm text-red-600">{error}</div> : null}
            <Button className="w-full" type="submit" disabled={loading}>
              {loading ? "Connexion..." : "Se connecter"}
            </Button>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}


