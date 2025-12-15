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
// FinOps (costs)
// -----------------------------

export type FinopsForecastMinuteRow = {
    bucket_minute: string;
    provider_id: string | null;
    burn_rate_usd_per_hour: number;
    forecast_usd_per_minute: number;
    forecast_usd_per_day: number;
    forecast_usd_per_month_30d: number;
};

export type FinopsActualMinuteRow = {
    bucket_minute: string;
    provider_id: string | null;
    instance_id: string | null;
    amount_usd: number;
};

export type FinopsCumulativeMinuteRow = {
    bucket_minute: string;
    provider_id: string | null;
    instance_id: string | null;
    cumulative_amount_usd: number;
};

export type FinopsCostCurrentResponse = {
    latest_bucket_minute: string | null;
    forecast: FinopsForecastMinuteRow[];
    cumulative_total: FinopsCumulativeMinuteRow | null;
};
