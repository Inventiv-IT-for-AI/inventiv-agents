"use client";

import { IAConfirmModal } from "ia-widgets";
import { apiUrl } from "@/lib/api";

type ArchiveInstanceModalProps = {
  open: boolean;
  onClose: () => void;
  instanceId: string | null;
  onSuccess: () => void;
};

export function ArchiveInstanceModal({ open, onClose, instanceId, onSuccess }: ArchiveInstanceModalProps) {
  return (
    <IAConfirmModal
      open={open}
      onOpenChange={(v) => {
        if (!v) onClose();
      }}
      title="Archiver l'instance"
      description="L’instance sera archivée et masquée des listes actives. Action réversible en base uniquement (selon outils admin)."
      details={
        <div>
          Instance ID: <span className="font-mono text-foreground">{instanceId}</span>
        </div>
      }
      confirmLabel="Archiver"
      confirmingLabel="Archivage..."
      confirmVariant="destructive"
      successTitle="Demande d’archivage prise en compte"
      successTone="danger"
      onConfirm={async () => {
        if (!instanceId) return;
        const res = await fetch(apiUrl(`instances/${instanceId}/archive`), { method: "PUT" });
        if (!res.ok) throw new Error("archive failed");
        onSuccess();
      }}
      autoCloseMs={1500}
    />
  );
}


