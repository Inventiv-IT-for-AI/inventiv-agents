"use client";

import { useState } from "react";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { CheckCircle } from "lucide-react";
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
    const [terminateStep, setTerminateStep] = useState<"confirm" | "submitting" | "success">("confirm");

    const handleConfirmTerminate = async () => {
        if (!instanceId) return;
        setTerminateStep("submitting");

        try {
            const res = await fetch(apiUrl(`instances/${instanceId}`), { method: "DELETE" });
            if (res.ok) {
                setTerminateStep("success");
                setTimeout(() => {
                    handleClose();
                    onSuccess();
                }, 1500);
            } else {
                alert("Failed to terminate.");
                handleClose();
            }
        } catch (e) {
            console.error(e);
            alert("Error terminating instance.");
            handleClose();
        }
    };

    const handleClose = () => {
        setTerminateStep("confirm");
        onClose();
    };

    return (
        <Dialog open={open} onOpenChange={handleClose}>
            <DialogContent showCloseButton={false} className="sm:max-w-[425px]">
                <DialogHeader>
                    <DialogTitle>Terminer l&apos;instance</DialogTitle>
                    <DialogDescription>
                        Voulez-vous vraiment terminer cette instance ? Cette action est irréversible.
                    </DialogDescription>
                </DialogHeader>

                {terminateStep === "success" ? (
                    <div className="flex flex-col items-center justify-center py-6 space-y-4 text-red-600 animate-in fade-in zoom-in duration-300">
                        <CheckCircle className="h-16 w-16" />
                        <span className="text-xl font-bold">Demande de terminaison prise en compte</span>
                    </div>
                ) : (
                    <div className="py-4 text-sm text-muted-foreground">
                        Instance ID: <span className="font-mono text-foreground">{instanceId}</span>
                    </div>
                )}

                <DialogFooter>
                    {terminateStep === "success" ? (
                        <Button variant="outline" onClick={handleClose}>
                            Fermer
                        </Button>
                    ) : (
                        <div className="flex w-full flex-col-reverse gap-2 sm:flex-row sm:justify-between">
                            <Button
                                variant="outline"
                                onClick={handleClose}
                                disabled={terminateStep === "submitting"}
                            >
                                Annuler
                            </Button>
                            <Button
                                variant="destructive"
                                onClick={handleConfirmTerminate}
                                disabled={terminateStep === "submitting"}
                            >
                                {terminateStep === "submitting" ? "Terminer..." : "Terminer"}
                            </Button>
                        </div>
                    )}
                </DialogFooter>
            </DialogContent>
        </Dialog>
    );
}
