"use client";

import * as React from "react";
import * as SwitchPrimitive from "@radix-ui/react-switch";
import { cn } from "./utils/cn";

export type AIToggleProps = {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  disabled?: boolean;
  className?: string;
  "aria-label"?: string;
};

/**
 * Compact toggle used across Settings tables.
 * - ON: sky blue (Inventiv accent)
 * - OFF: muted gray
 */
export function AIToggle({ checked, onCheckedChange, disabled, className, ...props }: AIToggleProps) {
  return (
    <SwitchPrimitive.Root
      checked={checked}
      onCheckedChange={onCheckedChange}
      disabled={disabled}
      data-slot="ia-toggle"
      className={cn(
        "inline-flex h-5 w-9 shrink-0 items-center rounded-full border border-transparent shadow-xs transition-colors outline-none",
        "focus-visible:ring-[3px] focus-visible:ring-sky-300/60 focus-visible:border-sky-400",
        "disabled:cursor-not-allowed disabled:opacity-50",
        "data-[state=checked]:bg-sky-500 data-[state=unchecked]:bg-muted",
        className
      )}
      {...props}
    >
      <SwitchPrimitive.Thumb
        data-slot="ia-toggle-thumb"
        className={cn(
          "pointer-events-none block size-4 rounded-full bg-background ring-0 shadow-sm transition-transform",
          "data-[state=checked]:translate-x-4 data-[state=unchecked]:translate-x-0.5"
        )}
      />
    </SwitchPrimitive.Root>
  );
}


