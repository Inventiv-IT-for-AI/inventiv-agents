"use client";

import { IAConfirmModal } from "ia-widgets";
import { apiUrl } from "@/lib/api";

type TerminateInstanceModalProps = {
    open: boolean;
    onClose: () => void;
    instanceId: string | null;
    onSuccess: () => void;
};

export function TerminateInstanceModal({
    open,
    onClose,
    instanceId,
    onSuccess,
}: TerminateInstanceModalProps) {
    return (
        <IAConfirmModal
            open={open}
            onOpenChange={(v) => {
                if (!v) {
                    onClose();
                }
            }}
            title="Terminer l&apos;instance"
            description="Voulez-vous vraiment terminer cette instance ? Cette action est irréversible."
            details={
                <div>
                    Instance ID: <span className="font-mono text-foreground">{instanceId}</span>
                </div>
            }
            confirmLabel="Terminer"
            confirmingLabel="Terminer..."
            confirmVariant="destructive"
            successTitle="Demande de terminaison prise en compte"
            successTone="danger"
            onConfirm={async () => {
                if (!instanceId) return;
                const res = await fetch(apiUrl(`instances/${instanceId}`), { method: "DELETE" });
                if (!res.ok) throw new Error("terminate failed");
                onSuccess();
            }}
            autoCloseMs={1500}
        />
    );
}
