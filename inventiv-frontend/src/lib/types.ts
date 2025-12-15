/**
 * Common TypeScript types used across the application
 */

export type Instance = {
    id: string;
    provider_id: string;
    provider_name: string;
    zone: string;
    region: string;
    instance_type: string;
    status: string;
    ip_address: string | null;
    created_at: string;
    terminated_at?: string | null;
    provider_instance_id?: string | null;
    last_health_check?: string | null;
    last_reconciliation?: string | null;
    health_check_failures?: number | null;
    deletion_reason?: string | null;
    error_code?: string | null;
    error_message?: string | null;
    gpu_vram?: number;
    gpu_count?: number;
    cost_per_hour?: number;
    total_cost?: number;
};

export type Provider = {
    id: string;
    name: string;
    code: string;
    description?: string | null;
    is_active?: boolean;
};

export type Region = {
    id: string;
    provider_id?: string;
    provider_name?: string;
    provider_code?: string | null;
    name: string;
    code: string;
    is_active: boolean;
};

export type Zone = {
    id: string;
    region_id?: string;
    region_name?: string;
    region_code?: string | null;
    provider_id?: string;
    provider_name?: string;
    provider_code?: string | null;
    name: string;
    code: string;
    is_active: boolean;
};

export type InstanceType = {
    id: string;
    provider_id?: string;
    name: string;
    code: string | null;
    cost_per_hour: number | null;
    is_active: boolean;
    gpu_count?: number;
    vram_per_gpu_gb?: number;
    cpu_count?: number;
    ram_gb?: number;
    bandwidth_bps?: number;
};

export type ActionLog = {
    id: string;
    action_type: string;
    component: string;
    status: string;
    provider_name?: string | null;
    instance_type?: string | null;
    error_message: string | null;
    instance_id: string | null;
    duration_ms: number | null;
    created_at: string;
    completed_at?: string | null;
    metadata: Record<string, unknown> | null;
    instance_status_before?: string | null;
    instance_status_after?: string | null;
};

export type ActionType = {
    code: string;
    label: string;
    icon: string;
    color_class: string;
    category?: string | null;
    is_active: boolean;
};

export type InstanceStats = {
    total: number;
    active: number;
    provisioning: number;
    failed: number;
};

// -----------------------------
// FinOps (costs) - EUR
// -----------------------------

export type FinopsForecastMinuteRow = {
    bucket_minute: string;
    provider_id: string | null;

    burn_rate_eur_per_hour: number;

    forecast_eur_per_minute: number;
    forecast_eur_per_hour: number;
    forecast_eur_per_day: number;
    forecast_eur_per_month_30d: number;
    forecast_eur_per_year_365d: number;
};

export type FinopsActualMinuteRow = {
    bucket_minute: string;
    provider_id: string | null;
    instance_id: string | null;
    amount_eur: number;
};

export type FinopsCumulativeMinuteRow = {
    bucket_minute: string;
    provider_id: string | null;
    instance_id: string | null;
    cumulative_amount_eur: number;
};

export type FinopsCostCurrentResponse = {
    latest_bucket_minute: string | null;
    forecast: FinopsForecastMinuteRow[];
    cumulative_total: FinopsCumulativeMinuteRow | null;
};

export type FinopsProviderCostRow = {
    provider_id: string;
    provider_code: string | null;
    provider_name: string;
    amount_eur: number;
};

export type FinopsRegionCostRow = {
    provider_id: string;
    provider_code: string | null;
    region_id: string;
    region_code: string | null;
    region_name: string;
    amount_eur: number;
};

export type FinopsInstanceTypeCostRow = {
    provider_id: string;
    provider_code: string | null;
    instance_type_id: string;
    instance_type_code: string | null;
    instance_type_name: string;
    amount_eur: number;
};

export type FinopsInstanceCostRow = {
    instance_id: string;
    provider_id: string;
    provider_code: string | null;
    provider_name: string;
    region_name: string | null;
    zone_name: string | null;
    instance_type_name: string | null;
    amount_eur: number;
};

export type FinopsCostsDashboardResponse = {
    bucket_minute: string | null;
    total_minute_eur: number;
    by_provider_minute: FinopsProviderCostRow[];
    by_region_minute: FinopsRegionCostRow[];
    by_instance_type_minute: FinopsInstanceTypeCostRow[];
    by_instance_minute: FinopsInstanceCostRow[];
};
