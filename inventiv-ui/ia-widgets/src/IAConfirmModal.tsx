"use client";

import * as React from "react";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "./ui/dialog";
import { Button } from "./ui/button";
import { IARequestAccepted, type IARequestAcceptedTone } from "./IARequestAccepted";
import { cn } from "./utils/cn";

export type IAConfirmModalState = "confirm" | "submitting" | "success";

export type IAConfirmModalProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;

  title: string;
  description?: string;

  /** Optional detail block shown under the header in confirm/submitting states (e.g. Instance ID). */
  details?: React.ReactNode;

  confirmLabel: string;
  confirmingLabel?: string;
  cancelLabel?: string;
  closeLabel?: string;

  /** Default: "default" */
  confirmVariant?: "default" | "destructive" | "secondary" | "outline" | "ghost" | "link";

  /** Called when user confirms. If it resolves, we show success state. If it throws, we keep confirm state. */
  onConfirm: () => Promise<void>;

  /** Success screen */
  successTitle: string;
  successTone?: IARequestAcceptedTone;
  autoCloseMs?: number;

  /** Dialog sizing */
  contentClassName?: string;
};

export function IAConfirmModal({
  open,
  onOpenChange,
  title,
  description,
  details,
  confirmLabel,
  confirmingLabel,
  cancelLabel = "Annuler",
  closeLabel = "Fermer",
  confirmVariant = "default",
  onConfirm,
  successTitle,
  successTone = "success",
  autoCloseMs = 1500,
  contentClassName,
}: IAConfirmModalProps) {
  const [state, setState] = React.useState<IAConfirmModalState>("confirm");

  const close = React.useCallback(() => {
    setState("confirm");
    onOpenChange(false);
  }, [onOpenChange]);

  React.useEffect(() => {
    if (!open) setState("confirm");
  }, [open]);

  const handleConfirm = React.useCallback(async () => {
    setState("submitting");
    try {
      await onConfirm();
      setState("success");
      if (autoCloseMs > 0) {
        window.setTimeout(() => close(), autoCloseMs);
      }
    } catch {
      setState("confirm");
    }
  }, [autoCloseMs, close, onConfirm]);

  return (
    <Dialog open={open} onOpenChange={(v) => (v ? onOpenChange(true) : close())}>
      <DialogContent showCloseButton={false} className={cn("sm:max-w-[460px]", contentClassName)}>
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          {description ? <DialogDescription>{description}</DialogDescription> : null}
        </DialogHeader>

        {state === "success" ? (
          <IARequestAccepted title={successTitle} tone={successTone} />
        ) : details ? (
          <div className="py-4 text-sm text-muted-foreground">{details}</div>
        ) : null}

        <DialogFooter>
          {state === "success" ? (
            <Button variant="outline" onClick={close}>
              {closeLabel}
            </Button>
          ) : (
            <div className="flex w-full flex-col-reverse gap-2 sm:flex-row sm:justify-between">
              <Button variant="outline" onClick={close} disabled={state === "submitting"}>
                {cancelLabel}
              </Button>
              <Button variant={confirmVariant} onClick={handleConfirm} disabled={state === "submitting"}>
                {state === "submitting" ? confirmingLabel ?? `${confirmLabel}...` : confirmLabel}
              </Button>
            </div>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}


