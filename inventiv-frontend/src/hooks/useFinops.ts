import { useCallback, useEffect, useState } from "react";
import { apiUrl } from "@/lib/api";
import {
    FinopsCostsDashboardSummaryResponse,
    FinopsCostsDashboardWindowResponse,
    FinopsCostsDashboardSeriesPoint,
} from "@/lib/types";

export function useFinopsCosts() {
    const [summary, setSummary] = useState<FinopsCostsDashboardSummaryResponse | null>(null);
    const [window, setWindow] = useState<string>("hour"); // "hour" | "day" | "week_7d" | "month_30d" | "year_365d"
    const [breakdown, setBreakdown] = useState<FinopsCostsDashboardWindowResponse | null>(null);
    const [series, setSeries] = useState<FinopsCostsDashboardSeriesPoint[] | null>(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);

    const fetchAll = useCallback(async () => {
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
    }, [window]);

    const fetchSeries = useCallback(async () => {
        try {
            const res = await fetch(
                apiUrl(`finops/dashboard/costs/series?window=${encodeURIComponent(window)}&limit_points=220`)
            );
            if (!res.ok) {
                setSeries(null);
                return;
            }
            const data = (await res.json()) as FinopsCostsDashboardSeriesPoint[];
            setSeries(Array.isArray(data) ? data : null);
        } catch (err) {
            console.error("FinOps series fetch error:", err);
            setSeries(null);
        }
    }, [window]);

    useEffect(() => {
        fetchAll(); // initial
        fetchSeries();
        const interval = setInterval(() => {
            fetchAll();
            fetchSeries();
        }, 10000); // every 10s
        return () => clearInterval(interval);
    }, [window, fetchAll, fetchSeries]);

    return {
        summary,
        window,
        setWindow,
        breakdown,
        series,
        loading,
        error,
        refresh: fetchAll,
    };
}

