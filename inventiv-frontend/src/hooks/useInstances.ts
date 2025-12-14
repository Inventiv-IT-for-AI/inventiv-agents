import { useState, useEffect } from "react";
import { apiUrl } from "@/lib/api";
import { Instance } from "@/lib/types";

export function useInstances() {
    const [instances, setInstances] = useState<Instance[]>([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);

    const fetchInstances = async () => {
        try {
            setLoading(true);
            const res = await fetch(apiUrl("instances"));
            if (res.ok) {
                const data = await res.json();
                setInstances(data);
                setError(null);
            } else {
                setError("Failed to fetch instances");
            }
        } catch (err) {
            console.error("Polling Error:", err);
            setError(err instanceof Error ? err.message : "Unknown error");
        } finally {
            setLoading(false);
        }
    };

    useEffect(() => {
        fetchInstances(); // Initial fetch
        const interval = setInterval(fetchInstances, 5000); // Poll every 5s

        // Listen for manual refreshes
        window.addEventListener("refresh-instances", fetchInstances);

        return () => {
            clearInterval(interval);
            window.removeEventListener("refresh-instances", fetchInstances);
        };
    }, []);

    const refreshInstances = () => {
        window.dispatchEvent(new Event("refresh-instances"));
    };

    return {
        instances,
        loading,
        error,
        refreshInstances,
    };
}
