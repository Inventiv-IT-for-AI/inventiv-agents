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
    gpu_vram?: number;
    cost_per_hour?: number;
    total_cost?: number;
};

export type Provider = {
    id: string;
    name: string;
};

export type Region = {
    id: string;
    name: string;
    code: string;
    is_active: boolean;
};

export type Zone = {
    id: string;
    name: string;
    code: string;
    is_active: boolean;
};

export type InstanceType = {
    id: string;
    name: string;
    code: string | null;
    cost_per_hour: number | null;
    is_active: boolean;
    gpu_count?: number;
    vram_per_gpu_gb?: number;
};

export type ActionLog = {
    id: string;
    action_type: string;
    component: string;
    status: string;
    error_message: string | null;
    instance_id: string | null;
    duration_ms: number | null;
    created_at: string;
    completed_at?: string | null;
    metadata: Record<string, unknown> | null;
};

export type InstanceStats = {
    total: number;
    active: number;
    provisioning: number;
    failed: number;
};
