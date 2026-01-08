"use client";

import * as React from "react";
import * as TabsPrimitive from "@radix-ui/react-tabs";
import { cn } from "../utils/cn";

// Radix types currently don't expose common React DOM props (children/className) reliably with our React/TS setup.
// We keep call-sites strict but intentionally loosen the primitive surface here.
type PrimitiveProps = {
  [key: string]: unknown;
  className?: string;
  children?: React.ReactNode;
};

const TabsRootPrimitive = TabsPrimitive.Root as unknown as React.ComponentType<PrimitiveProps>;
const TabsListPrimitive = TabsPrimitive.List as unknown as React.ComponentType<PrimitiveProps>;
const TabsTriggerPrimitive = TabsPrimitive.Trigger as unknown as React.ComponentType<PrimitiveProps>;
const TabsContentPrimitive = TabsPrimitive.Content as unknown as React.ComponentType<PrimitiveProps>;

type TabsRootProps = PrimitiveProps;
type TabsListProps = PrimitiveProps;
type TabsTriggerProps = PrimitiveProps;
type TabsContentProps = PrimitiveProps;

export function Tabs({ className, ...props }: TabsRootProps) {
  return <TabsRootPrimitive data-slot="tabs" className={cn("flex flex-col gap-2", className)} {...props} />;
}

export function TabsList({ className, ...props }: TabsListProps) {
  return (
    <TabsListPrimitive
      data-slot="tabs-list"
      className={cn(
        "inline-flex h-10 w-fit items-center justify-center rounded-xl border border-border/60 bg-secondary/60 p-1 text-muted-foreground shadow-sm",
        className
      )}
      {...props}
    />
  );
}

export function TabsTrigger({ className, ...props }: TabsTriggerProps) {
  return (
    <TabsTriggerPrimitive
      data-slot="tabs-trigger"
      className={cn(
        "relative inline-flex h-8 flex-1 items-center justify-center gap-1.5 rounded-lg px-3 text-sm font-medium whitespace-nowrap transition-[color,box-shadow]",
        "text-muted-foreground hover:text-foreground",
        "focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] focus-visible:outline-1",
        "disabled:pointer-events-none disabled:opacity-50",
        "data-[state=active]:bg-background data-[state=active]:text-foreground data-[state=active]:shadow-sm",
        "data-[state=active]:after:absolute data-[state=active]:after:bottom-1 data-[state=active]:after:left-3 data-[state=active]:after:right-3 data-[state=active]:after:h-[2px] data-[state=active]:after:rounded-full data-[state=active]:after:bg-brand-secondary",
        "[&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
        className
      )}
      {...props}
    />
  );
}

export function TabsContent({ className, ...props }: TabsContentProps) {
  return <TabsContentPrimitive data-slot="tabs-content" className={cn("flex-1 outline-none", className)} {...props} />;
}


