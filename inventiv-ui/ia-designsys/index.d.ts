import * as React from "react";
import * as DialogPrimitive from "@radix-ui/react-dialog";
import * as SelectPrimitive from "@radix-ui/react-select";
import * as SwitchPrimitive from "@radix-ui/react-switch";
import * as TabsPrimitive from "@radix-ui/react-tabs";

export declare function cn(...inputs: any[]): string;

// Button (shadcn-like)
export type ButtonProps = React.ButtonHTMLAttributes<HTMLButtonElement> & {
  asChild?: boolean;
  variant?: string;
  size?: string;
};
export declare const Button: React.ForwardRefExoticComponent<ButtonProps & React.RefAttributes<HTMLButtonElement>>;
export declare const buttonVariants: (...args: any[]) => string;

// Alert
export declare function Alert(props: React.HTMLAttributes<HTMLDivElement> & { variant?: string }): React.ReactElement;
export declare function AlertTitle(props: React.HTMLAttributes<HTMLHeadingElement>): React.ReactElement;
export declare function AlertDescription(props: React.HTMLAttributes<HTMLParagraphElement>): React.ReactElement;

// Card
export declare function Card(props: React.HTMLAttributes<HTMLDivElement>): React.ReactElement;
export declare function CardHeader(props: React.HTMLAttributes<HTMLDivElement>): React.ReactElement;
export declare function CardTitle(props: React.HTMLAttributes<HTMLHeadingElement>): React.ReactElement;
export declare function CardContent(props: React.HTMLAttributes<HTMLDivElement>): React.ReactElement;

// Input / Badge / Label
export declare function Input(props: React.InputHTMLAttributes<HTMLInputElement> & { className?: string }): React.ReactElement;
export declare function Badge(props: React.HTMLAttributes<HTMLDivElement> & { variant?: string }): React.ReactElement;
export declare const badgeVariants: (...args: any[]) => string;
export declare function Label(props: React.LabelHTMLAttributes<HTMLLabelElement>): React.ReactElement;

// Dialog (Radix wrappers)
export declare function Dialog(props: React.ComponentProps<typeof DialogPrimitive.Root>): React.ReactElement;
export declare function DialogTrigger(props: React.ComponentProps<typeof DialogPrimitive.Trigger>): React.ReactElement;
export declare function DialogPortal(props: React.ComponentProps<typeof DialogPrimitive.Portal>): React.ReactElement;
export declare function DialogClose(props: React.ComponentProps<typeof DialogPrimitive.Close>): React.ReactElement;
export declare function DialogOverlay(props: React.ComponentProps<typeof DialogPrimitive.Overlay>): React.ReactElement;
export declare function DialogContent(
  props: React.ComponentProps<typeof DialogPrimitive.Content> & { showCloseButton?: boolean }
): React.ReactElement;
export declare function DialogHeader(props: React.HTMLAttributes<HTMLDivElement>): React.ReactElement;
export declare function DialogFooter(props: React.HTMLAttributes<HTMLDivElement>): React.ReactElement;
export declare function DialogTitle(props: React.ComponentProps<typeof DialogPrimitive.Title>): React.ReactElement;
export declare function DialogDescription(props: React.ComponentProps<typeof DialogPrimitive.Description>): React.ReactElement;

// Switch
export declare function Switch(props: React.ComponentProps<typeof SwitchPrimitive.Root>): React.ReactElement;

// Select (Radix wrappers)
export declare function Select(props: React.ComponentProps<typeof SelectPrimitive.Root>): React.ReactElement;
export declare function SelectGroup(props: React.ComponentProps<typeof SelectPrimitive.Group>): React.ReactElement;
export declare function SelectValue(props: React.ComponentProps<typeof SelectPrimitive.Value>): React.ReactElement;
export declare function SelectTrigger(props: React.ComponentProps<typeof SelectPrimitive.Trigger>): React.ReactElement;
export declare function SelectContent(props: React.ComponentProps<typeof SelectPrimitive.Content>): React.ReactElement;
export declare function SelectLabel(props: React.ComponentProps<typeof SelectPrimitive.Label>): React.ReactElement;
export declare function SelectItem(props: React.ComponentProps<typeof SelectPrimitive.Item>): React.ReactElement;
export declare function SelectSeparator(props: React.ComponentProps<typeof SelectPrimitive.Separator>): React.ReactElement;
export declare function SelectScrollUpButton(props: React.ComponentProps<typeof SelectPrimitive.ScrollUpButton>): React.ReactElement;
export declare function SelectScrollDownButton(props: React.ComponentProps<typeof SelectPrimitive.ScrollDownButton>): React.ReactElement;

// Tabs (Radix wrappers)
export declare function Tabs(props: React.ComponentProps<typeof TabsPrimitive.Root>): React.ReactElement;
export declare function TabsList(props: React.ComponentProps<typeof TabsPrimitive.List>): React.ReactElement;
export declare function TabsTrigger(props: React.ComponentProps<typeof TabsPrimitive.Trigger>): React.ReactElement;
export declare function TabsContent(props: React.ComponentProps<typeof TabsPrimitive.Content>): React.ReactElement;

// IA aliases
export declare const IAAlert: typeof Alert;
export declare const IAAlertTitle: typeof AlertTitle;
export declare const IAAlertDescription: typeof AlertDescription;


