"use client";

import { useState } from "react";
import { Copy, Check } from "lucide-react";
import { Button } from "./ui/button";

export type IACopyButtonProps = {
  text: string;
  className?: string;
};

export function IACopyButton({ text, className = "" }: IACopyButtonProps) {
  const [copied, setCopied] = useState(false);

  const onCopy = async () => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 2000);
    } catch {
      // ignore (clipboard may be denied); keep UX minimal
    }
  };

  return (
    <Button variant="ghost" size="icon" className={`h-6 w-6 ${className}`} onClick={onCopy}>
      {copied ? <Check className="h-3 w-3 text-green-500" /> : <Copy className="h-3 w-3 text-muted-foreground" />}
    </Button>
  );
}


