"use client";

import { useEffect } from "react";
import { useRouter } from "next/navigation";

export default function ModelsPage() {
  const router = useRouter();
  useEffect(() => {
    router.replace("/settings?tab=models");
  }, [router]);
  return null;
}


