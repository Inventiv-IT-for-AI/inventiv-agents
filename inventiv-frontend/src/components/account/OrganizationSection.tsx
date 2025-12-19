"use client";

import type { Organization } from "@/lib/types";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";

export type WorkspaceMe = {
  current_organization_id?: string | null;
};

export type OrganizationSectionProps = {
  me: WorkspaceMe | null;
  orgs: Organization[];
  orgLoading: boolean;
  orgError: string | null;
  onSelectOrg: (id: string) => void;
  onSelectPersonal: () => void;
  onOpenCreateOrg: () => void;
  fullWidthTrigger?: boolean;
};

export function OrganizationSection({
  me,
  orgs,
  orgLoading,
  orgError,
  onSelectOrg,
  onSelectPersonal,
  onOpenCreateOrg,
  fullWidthTrigger,
}: OrganizationSectionProps) {
  const value = me?.current_organization_id ? me.current_organization_id : "__personal__";

  return (
    <div className="grid gap-2">
      <div className="text-sm font-medium">Workspace</div>
      <Select
        value={value}
        onValueChange={(v: string) => {
          if (v === "__personal__") onSelectPersonal();
          else onSelectOrg(v);
        }}
        disabled={orgLoading}
      >
        <SelectTrigger className={fullWidthTrigger ? "w-full" : undefined}>
          <SelectValue placeholder={orgs.length === 0 ? "Aucune organisation" : "Sélectionner..."} />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="__personal__">Personal (sans organisation)</SelectItem>
          {orgs.map((o) => (
            <SelectItem key={o.id} value={o.id}>
              {o.name} {o.slug ? `(${o.slug})` : ""}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      {orgError ? <div className="text-sm text-red-600">{orgError}</div> : null}
      <div className="flex gap-2">
        <Button variant="outline" onClick={onOpenCreateOrg}>
          Créer une org
        </Button>
      </div>
    </div>
  );
}


