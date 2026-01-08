"use client";

import { useState } from "react";
import { IAConfirmModal } from "ia-widgets";
import { apiUrl } from "@/lib/api";

type ReinstallInstanceModalProps = {
  open: boolean;
  onClose: () => void;
  instanceId: string | null;
  onSuccess: () => void;
};

export function ReinstallInstanceModal({
  open,
  onClose,
  instanceId,
  onSuccess,
}: ReinstallInstanceModalProps) {
  const [hint, setHint] = useState<string | null>(null);

  return (
    <IAConfirmModal
      open={open}
      onOpenChange={(v) => {
        if (!v) {
          setHint(null);
          onClose();
        }
      }}
      title="Réinstaller l&apos;instance"
      description="Cette action relance l'installation du Worker (SSH bootstrap) et redémarre les services (vLLM/agent). À utiliser pour réparer une instance déjà allouée et joignable."
      details={
        <div className="space-y-2">
          <div>
            Instance ID: <span className="font-mono text-foreground">{instanceId}</span>
          </div>
          {hint ? <div className="text-xs text-muted-foreground">{hint}</div> : null}
        </div>
      }
      confirmLabel="Réinstaller"
      confirmingLabel="Réinstaller..."
      confirmVariant="default"
      successTitle="Demande de réinstallation prise en compte"
      successTone="info"
      onConfirm={async () => {
        if (!instanceId) return;
        const res = await fetch(apiUrl(`instances/${instanceId}/reinstall`), { method: "POST" });
        if (!res.ok) {
          const txt = await res.text().catch(() => "");
          setHint(txt || "Failed to reinstall.");
          throw new Error("reinstall failed");
        }
        onSuccess();
      }}
      autoCloseMs={1500}
    />
  );
}


