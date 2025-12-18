"use client";

import { useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { CheckCircle } from "lucide-react";
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
  const [step, setStep] = useState<"confirm" | "submitting" | "success">("confirm");

  const handleConfirm = async () => {
    if (!instanceId) return;
    setStep("submitting");

    try {
      const res = await fetch(apiUrl(`instances/${instanceId}/reinstall`), { method: "POST" });
      if (res.ok) {
        setStep("success");
        setTimeout(() => {
          handleClose();
          onSuccess();
        }, 1500);
      } else {
        const txt = await res.text().catch(() => "");
        alert(txt || "Failed to reinstall.");
        handleClose();
      }
    } catch (e) {
      console.error(e);
      alert("Error reinstalling instance.");
      handleClose();
    }
  };

  const handleClose = () => {
    setStep("confirm");
    onClose();
  };

  return (
    <Dialog open={open} onOpenChange={handleClose}>
      <DialogContent showCloseButton={false} className="sm:max-w-[460px]">
        <DialogHeader>
          <DialogTitle>Réinstaller l&apos;instance</DialogTitle>
          <DialogDescription>
            Cette action relance l&apos;installation du Worker (SSH bootstrap) et redémarre les services
            (vLLM/agent). À utiliser pour réparer une instance déjà allouée et joignable.
          </DialogDescription>
        </DialogHeader>

        {step === "success" ? (
          <div className="flex flex-col items-center justify-center py-6 space-y-4 text-sky-600 animate-in fade-in zoom-in duration-300">
            <CheckCircle className="h-16 w-16" />
            <span className="text-xl font-bold">Demande de réinstallation prise en compte</span>
          </div>
        ) : (
          <div className="py-4 text-sm text-muted-foreground">
            Instance ID: <span className="font-mono text-foreground">{instanceId}</span>
          </div>
        )}

        <DialogFooter>
          {step === "success" ? (
            <Button variant="outline" onClick={handleClose}>
              Fermer
            </Button>
          ) : (
            <div className="flex w-full flex-col-reverse gap-2 sm:flex-row sm:justify-between">
              <Button variant="outline" onClick={handleClose} disabled={step === "submitting"}>
                Annuler
              </Button>
              <Button onClick={handleConfirm} disabled={step === "submitting"}>
                {step === "submitting" ? "Réinstaller..." : "Réinstaller"}
              </Button>
            </div>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}


