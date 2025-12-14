import { useState } from "react";
import { apiUrl } from "@/lib/api";
import { Provider, Region, Zone, InstanceType } from "@/lib/types";

export function useCatalog() {
    const [providers, setProviders] = useState<Provider[]>([]);
    const [regions, setRegions] = useState<Region[]>([]);
    const [zones, setZones] = useState<Zone[]>([]);
    const [instanceTypes, setInstanceTypes] = useState<InstanceType[]>([]);
    const [loading, setLoading] = useState(false);

    const fetchCatalog = async () => {
        setLoading(true);
        try {
            const [regionsRes, zonesRes, typesRes] = await Promise.all([
                fetch(apiUrl("regions")),
                fetch(apiUrl("zones")),
                fetch(apiUrl("instance_types")),
            ]);

            if (regionsRes.ok) {
                const data: Region[] = await regionsRes.json();
                setRegions(data.filter((r) => r.is_active));
            }
            if (zonesRes.ok) {
                const data: Zone[] = await zonesRes.json();
                setZones(data.filter((z) => z.is_active));
            }
            if (typesRes.ok) {
                const data: InstanceType[] = await typesRes.json();
                setInstanceTypes(data.filter((t) => t.is_active));
            }

            // Hardcoded provider for now (Scaleway)
            setProviders([
                { id: "00000000-0000-0000-0000-000000000001", name: "Scaleway" },
            ]);
        } catch (err) {
            console.error("Failed to fetch catalog", err);
        } finally {
            setLoading(false);
        }
    };

    return {
        providers,
        regions,
        zones,
        instanceTypes,
        loading,
        fetchCatalog,
    };
}
