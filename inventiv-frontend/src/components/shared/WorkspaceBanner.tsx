"use client";

import { useEffect, useMemo, useState } from "react";
import { apiUrl } from "@/lib/api";
import { IAAlert, IAAlertDescription, IAAlertTitle } from "ia-designsys";

type Me = {
  current_organization_id?: string | null;
  current_organization_name?: string | null;
  current_organization_slug?: string | null;
};

export function WorkspaceBanner({ className }: { className?: string }) {
  const [me, setMe] = useState<Me | null>(null);

  useEffect(() => {
    let cancelled = false;
    fetch(apiUrl("/auth/me"))
      .then((r) => (r.ok ? r.json() : Promise.reject()))
      .then((u) => {
        if (cancelled) return;
        setMe(u as Me);
      })
      .catch(() => {
        if (cancelled) return;
        setMe(null);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const workspaceLabel = useMemo(() => {
    if (!me?.current_organization_id) return "Personal (sans organisation)";
    const name = (me.current_organization_name || "").trim();
    const slug = (me.current_organization_slug || "").trim();
    if (name && slug) return `${name} (${slug})`;
    return name || slug || "Organization";
  }, [me]);

  if (!me) return null;

  if (!me.current_organization_id) {
    return (
      <IAAlert className={className}>
        <IAAlertTitle>Workspace: {workspaceLabel}</IAAlertTitle>
        <IAAlertDescription>
          Vous êtes en mode user. Certaines fonctionnalités “infra / publication / coûts” seront rattachées à une organisation.
          Vous pouvez en créer une ou en sélectionner une via <b>Compte → Workspace</b>.
        </IAAlertDescription>
      </IAAlert>
    );
  }

  return (
    <IAAlert className={className}>
      <IAAlertTitle>Workspace: {workspaceLabel}</IAAlertTitle>
      <IAAlertDescription>
        Les actions et paramètres “infra / publication / coûts” seront rattachés à cette organisation.
      </IAAlertDescription>
    </IAAlert>
  );
}


