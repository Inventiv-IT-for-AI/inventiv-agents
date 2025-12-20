"use client";

import * as React from "react";
import { IASnackbarProvider } from "ia-widgets";

export function AppProviders({ children }: { children: React.ReactNode }) {
  return <IASnackbarProvider>{children}</IASnackbarProvider>;
}


