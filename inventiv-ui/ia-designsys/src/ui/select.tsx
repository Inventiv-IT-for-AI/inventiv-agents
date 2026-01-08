"use client";

import * as React from "react";
import * as SelectPrimitive from "@radix-ui/react-select";
import { CheckIcon, ChevronDownIcon, ChevronUpIcon } from "lucide-react";

import { cn } from "../utils/cn";

// Radix types currently don't expose common React DOM props (children/className) reliably with our React/TS setup.
// We keep call-sites strict but intentionally loosen the primitive surface here.
type PrimitiveProps = {
  [key: string]: unknown;
  className?: string;
  children?: React.ReactNode;
};

const SelectRootPrimitive = SelectPrimitive.Root as unknown as React.ComponentType<PrimitiveProps>;
const SelectGroupPrimitive = SelectPrimitive.Group as unknown as React.ComponentType<PrimitiveProps>;
const SelectValuePrimitive = SelectPrimitive.Value as unknown as React.ComponentType<PrimitiveProps>;
const SelectTriggerPrimitive = SelectPrimitive.Trigger as unknown as React.ComponentType<PrimitiveProps>;
const SelectIconPrimitive = SelectPrimitive.Icon as unknown as React.ComponentType<PrimitiveProps>;
const SelectPortalPrimitive = SelectPrimitive.Portal as unknown as React.ComponentType<PrimitiveProps>;
const SelectContentPrimitive = SelectPrimitive.Content as unknown as React.ComponentType<PrimitiveProps>;
const SelectViewportPrimitive = SelectPrimitive.Viewport as unknown as React.ComponentType<PrimitiveProps>;
const SelectLabelPrimitive = SelectPrimitive.Label as unknown as React.ComponentType<PrimitiveProps>;
const SelectItemPrimitive = SelectPrimitive.Item as unknown as React.ComponentType<PrimitiveProps>;
const SelectItemIndicatorPrimitive = SelectPrimitive.ItemIndicator as unknown as React.ComponentType<PrimitiveProps>;
const SelectItemTextPrimitive = SelectPrimitive.ItemText as unknown as React.ComponentType<PrimitiveProps>;
const SelectSeparatorPrimitive = SelectPrimitive.Separator as unknown as React.ComponentType<PrimitiveProps>;
const SelectScrollUpButtonPrimitive = SelectPrimitive.ScrollUpButton as unknown as React.ComponentType<PrimitiveProps>;
const SelectScrollDownButtonPrimitive = SelectPrimitive.ScrollDownButton as unknown as React.ComponentType<PrimitiveProps>;

type SelectRootProps = PrimitiveProps;
type SelectGroupProps = PrimitiveProps;
type SelectValueProps = PrimitiveProps;
type SelectTriggerProps = PrimitiveProps & { size?: "sm" | "default" };
type SelectContentProps = PrimitiveProps & { position?: "item-aligned" | "popper"; align?: "start" | "center" | "end" };
type SelectLabelProps = PrimitiveProps;
type SelectItemProps = PrimitiveProps;

export function Select({ ...props }: SelectRootProps) {
  return <SelectRootPrimitive data-slot="select" {...props} />;
}

export function SelectGroup({ ...props }: SelectGroupProps) {
  return <SelectGroupPrimitive data-slot="select-group" {...props} />;
}

export function SelectValue({ ...props }: SelectValueProps) {
  return <SelectValuePrimitive data-slot="select-value" {...props} />;
}

export function SelectTrigger({ className, size = "default", children, ...props }: SelectTriggerProps) {
  return (
    <SelectTriggerPrimitive
      data-slot="select-trigger"
      data-size={size}
      className={cn(
        "border-input data-[placeholder]:text-muted-foreground [&_svg:not([class*='text-'])]:text-muted-foreground flex w-fit items-center justify-between gap-2 rounded-lg border bg-background/70 px-3 py-2 text-sm whitespace-nowrap shadow-sm transition-[color,box-shadow] outline-none",
        "focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px]",
        "dark:bg-input/30 dark:hover:bg-input/50",
        "disabled:cursor-not-allowed disabled:opacity-50 data-[size=default]:h-9 data-[size=sm]:h-8",
        "*:data-[slot=select-value]:line-clamp-1 *:data-[slot=select-value]:flex *:data-[slot=select-value]:items-center *:data-[slot=select-value]:gap-2",
        "[&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
        className
      )}
      {...props}
    >
      {children}
      <SelectIconPrimitive asChild>
        <ChevronDownIcon className="size-4 opacity-70" />
      </SelectIconPrimitive>
    </SelectTriggerPrimitive>
  );
}

export function SelectContent({ className, children, position = "popper", align = "center", ...props }: SelectContentProps) {
  return (
    <SelectPortalPrimitive>
      <SelectContentPrimitive
        data-slot="select-content"
        className={cn(
          "bg-popover/98 text-popover-foreground data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95 data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 relative z-50 max-h-(--radix-select-content-available-height) min-w-[10rem] origin-(--radix-select-content-transform-origin) overflow-x-hidden overflow-y-auto rounded-xl border border-border/60 shadow-2xl backdrop-blur-[2px]",
          position === "popper" &&
            "data-[side=bottom]:translate-y-1 data-[side=left]:-translate-x-1 data-[side=right]:translate-x-1 data-[side=top]:-translate-y-1",
          className
        )}
        position={position}
        align={align}
        {...props}
      >
        <SelectScrollUpButton />
        <SelectViewportPrimitive
          className={cn(
            "p-1.5",
            position === "popper" && "h-[var(--radix-select-trigger-height)] w-full min-w-[var(--radix-select-trigger-width)] scroll-my-1"
          )}
        >
          {children}
        </SelectViewportPrimitive>
        <SelectScrollDownButton />
      </SelectContentPrimitive>
    </SelectPortalPrimitive>
  );
}

export function SelectLabel({ className, ...props }: SelectLabelProps) {
  return (
    <SelectLabelPrimitive
      data-slot="select-label"
      className={cn("text-muted-foreground px-2 py-1.5 text-xs", className)}
      {...props}
    />
  );
}

export function SelectItem({ className, children, ...props }: SelectItemProps) {
  return (
    <SelectItemPrimitive
      data-slot="select-item"
      className={cn(
        "relative flex w-full cursor-default items-center gap-2 rounded-lg py-2 pr-9 pl-2.5 text-sm outline-hidden select-none",
        "focus:bg-secondary focus:text-foreground",
        "data-[disabled]:pointer-events-none data-[disabled]:opacity-50",
        "[&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
        "*:[span]:last:flex *:[span]:last:items-center *:[span]:last:gap-2",
        className
      )}
      {...props}
    >
      <span className="absolute right-2 flex size-3.5 items-center justify-center">
        <SelectItemIndicatorPrimitive>
          <CheckIcon className="size-4 text-brand-secondary" />
        </SelectItemIndicatorPrimitive>
      </span>
      <SelectItemTextPrimitive>{children}</SelectItemTextPrimitive>
    </SelectItemPrimitive>
  );
}

export function SelectSeparator({ className, ...props }: PrimitiveProps) {
  return (
    <SelectSeparatorPrimitive data-slot="select-separator" className={cn("bg-border pointer-events-none -mx-1 my-1 h-px", className)} {...props} />
  );
}

export function SelectScrollUpButton({ className, ...props }: PrimitiveProps) {
  return (
    <SelectScrollUpButtonPrimitive
      data-slot="select-scroll-up-button"
      className={cn("flex cursor-default items-center justify-center py-1", className)}
      {...props}
    >
      <ChevronUpIcon className="size-4" />
    </SelectScrollUpButtonPrimitive>
  );
}

export function SelectScrollDownButton({ className, ...props }: PrimitiveProps) {
  return (
    <SelectScrollDownButtonPrimitive
      data-slot="select-scroll-down-button"
      className={cn("flex cursor-default items-center justify-center py-1", className)}
      {...props}
    >
      <ChevronDownIcon className="size-4" />
    </SelectScrollDownButtonPrimitive>
  );
}


