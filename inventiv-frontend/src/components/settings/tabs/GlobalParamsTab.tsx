"use client";

import { Card, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { apiUrl } from "@/lib/api";
import type { ChangeEvent } from "react";

export type GlobalParamsTabProps = {
  globalStaleSeconds: string;
  setGlobalStaleSeconds: (v: string) => void;
  globalSettingsLoading: boolean;
  settingsDefs: Record<string, { min?: number; max?: number; defInt?: number; defBool?: boolean; defText?: string; desc?: string }>;
  onSaved: () => void;
};

export function GlobalParamsTab({
  globalStaleSeconds,
  setGlobalStaleSeconds,
  globalSettingsLoading,
  settingsDefs,
  onSaved,
}: GlobalParamsTabProps) {
  const def = settingsDefs["OPENAI_WORKER_STALE_SECONDS"];
  const min = def?.min ?? 10;
  const max = def?.max ?? 86400;

  return (
    <Card>
      <CardContent className="space-y-4">
        <div className="flex items-center justify-between gap-4">
          <div>
            <div className="font-medium">OPENAI_WORKER_STALE_SECONDS</div>
            <div className="text-sm text-muted-foreground">
              Staleness window used for <code className="font-mono">/v1/models</code> and worker selection.
            </div>
          </div>
          <div className="flex items-center gap-2">
            <Input
              value={globalStaleSeconds}
              onChange={(e: ChangeEvent<HTMLInputElement>) => setGlobalStaleSeconds(e.target.value)}
              placeholder={def?.defInt != null ? `default (${def.defInt}s)` : "default"}
              className="w-48"
            />
            <Button
              size="sm"
              onClick={async () => {
                const v = globalStaleSeconds.trim() === "" ? null : Number(globalStaleSeconds.trim());
                if (v != null && (!Number.isFinite(v) || v < min || v > max)) return;
                const res = await fetch(apiUrl("settings/global"), {
                  method: "PUT",
                  headers: { "Content-Type": "application/json" },
                  body: JSON.stringify({ key: "OPENAI_WORKER_STALE_SECONDS", value_int: v }),
                });
                if (!res.ok) return;
                onSaved();
              }}
              disabled={globalSettingsLoading}
            >
              Enregistrer
            </Button>
          </div>
        </div>
        <div className="text-xs text-muted-foreground">
          Range: {min}..{max}s. Leave empty for default.
        </div>
      </CardContent>
    </Card>
  );
}


