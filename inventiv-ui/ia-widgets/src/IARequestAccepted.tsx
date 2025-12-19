"use client";

import { CheckCircle } from "lucide-react";
import { cn } from "./utils/cn";

export type IARequestAcceptedTone = "success" | "info" | "danger";

export type IARequestAcceptedProps = {
  title: string;
  tone?: IARequestAcceptedTone;
  className?: string;
};

const toneClass: Record<IARequestAcceptedTone, string> = {
  success: "text-green-600",
  info: "text-sky-600",
  danger: "text-red-600",
};

export function IARequestAccepted({ title, tone = "success", className }: IARequestAcceptedProps) {
  return (
    <div
      className={cn(
        "flex flex-col items-center justify-center py-6 space-y-4 animate-in fade-in zoom-in duration-300",
        toneClass[tone],
        className
      )}
    >
      <CheckCircle className="h-16 w-16" />
      <span className="text-xl font-bold text-center">{title}</span>
    </div>
  );
}


