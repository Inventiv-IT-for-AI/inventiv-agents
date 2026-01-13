import { useCallback, useState } from "react";
import { apiUrl } from "@/lib/api";
import type { EnhancedActionLog, StateTransition, PhaseSummary, ExportSummary, Instance } from "@/lib/types";

/**
 * Re-export types from lib/types for convenience
 */
export type { EnhancedActionLog, StateTransition, PhaseSummary, ExportSummary };

/**
 * Instance export response from backend
 */
export interface InstanceExportResponse {
  instance: Instance;
  storages: any[]; // Storage type from backend
  actions: EnhancedActionLog[];
  state_transitions: StateTransition[];
  summary: ExportSummary;
}

/**
 * Hook for exporting instance data with enhanced action logs
 */
export function useInstanceExport() {
  const [exporting, setExporting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const exportInstance = useCallback(async (instanceId: string): Promise<InstanceExportResponse | null> => {
    if (exporting) return null;

    setExporting(true);
    setError(null);

    try {
      const response = await fetch(apiUrl(`instances/${instanceId}/export`), {
        credentials: "include",
      });

      if (!response.ok) {
        const errorText = await response.text();
        throw new Error(`Export failed: ${response.status} ${errorText}`);
      }

      const data: InstanceExportResponse = await response.json();
      return data;
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      setError(errorMessage);
      console.error("Failed to export instance:", err);
      return null;
    } finally {
      setExporting(false);
    }
  }, [exporting]);

  const copyExportToClipboard = useCallback(async (instanceId: string): Promise<boolean> => {
    const exportData = await exportInstance(instanceId);
    if (!exportData) return false;

    try {
      const jsonString = JSON.stringify(exportData, null, 2);
      await navigator.clipboard.writeText(jsonString);
      return true;
    } catch (err) {
      console.error("Failed to copy export to clipboard:", err);
      setError("Failed to copy to clipboard");
      return false;
    }
  }, [exportInstance]);

  return {
    exportInstance,
    copyExportToClipboard,
    exporting,
    error,
  };
}
