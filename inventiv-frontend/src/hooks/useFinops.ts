import { useEffect, useState } from "react";
import { apiUrl } from "@/lib/api";
import {
    FinopsCostsDashboardSummaryResponse,
    FinopsCostsDashboardWindowResponse,
} from "@/lib/types";

export function useFinopsCosts() {
    const [summary, setSummary] = useState<FinopsCostsDashboardSummaryResponse | null>(null);
    const [window, setWindow] = useState<string>("hour");
    const [breakdown, setBreakdown] = useState<FinopsCostsDashboardWindowResponse | null>(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);

    const fetchAll = async () => {
        try {
            setLoading(true);
            const [summaryRes, breakdownRes] = await Promise.all([
                fetch(apiUrl("finops/dashboard/costs/summary?limit_instances=20")),
                fetch(apiUrl(`finops/dashboard/costs/window?window=${encodeURIComponent(window)}&limit_instances=20`)),
            ]);

            if (summaryRes.ok) {
                const data: FinopsCostsDashboardSummaryResponse = await summaryRes.json();
                setSummary(data);
            }
            if (breakdownRes.ok) {
                const data: FinopsCostsDashboardWindowResponse = await breakdownRes.json();
                setBreakdown(data);
            }

            if (!summaryRes.ok || !breakdownRes.ok) {
                setError("Failed to fetch FinOps data");
            } else {
                setError(null);
            }
        } catch (err) {
            console.error("FinOps fetch error:", err);
            setError(err instanceof Error ? err.message : "Unknown error");
        } finally {
            setLoading(false);
        }
    };

    useEffect(() => {
        fetchAll(); // initial
        const interval = setInterval(fetchAll, 10000); // every 10s
        return () => clearInterval(interval);
    }, [window]);

    return {
        summary,
        window,
        setWindow,
        breakdown,
        loading,
        error,
        refresh: fetchAll,
    };
}

