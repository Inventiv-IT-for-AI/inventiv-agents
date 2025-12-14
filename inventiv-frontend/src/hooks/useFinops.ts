import { useEffect, useState } from "react";
import { apiUrl } from "@/lib/api";
import {
    FinopsActualMinuteRow,
    FinopsCostCurrentResponse,
    FinopsCumulativeMinuteRow,
} from "@/lib/types";

export function useFinopsCosts() {
    const [current, setCurrent] = useState<FinopsCostCurrentResponse | null>(null);
    const [actualTotalSeries, setActualTotalSeries] = useState<FinopsActualMinuteRow[]>([]);
    const [cumulativeTotalSeries, setCumulativeTotalSeries] = useState<FinopsCumulativeMinuteRow[]>([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);

    const fetchAll = async () => {
        try {
            setLoading(true);
            const [currentRes, actualRes, cumulativeRes] = await Promise.all([
                fetch(apiUrl("finops/cost/current")),
                fetch(apiUrl("finops/cost/actual/minute?minutes=60")),
                fetch(apiUrl("finops/cost/cumulative/minute?minutes=60")),
            ]);

            if (currentRes.ok) {
                const data: FinopsCostCurrentResponse = await currentRes.json();
                setCurrent(data);
            }
            if (actualRes.ok) {
                const data: FinopsActualMinuteRow[] = await actualRes.json();
                setActualTotalSeries(data);
            }
            if (cumulativeRes.ok) {
                const data: FinopsCumulativeMinuteRow[] = await cumulativeRes.json();
                setCumulativeTotalSeries(data);
            }

            if (!currentRes.ok || !actualRes.ok || !cumulativeRes.ok) {
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
    }, []);

    return {
        current,
        actualTotalSeries,
        cumulativeTotalSeries,
        loading,
        error,
        refresh: fetchAll,
    };
}

