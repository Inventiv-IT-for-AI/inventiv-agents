#!/bin/bash
# Scaleway Specific Initialization

echo "[Scaleway Flavor] Initializing..."

# 1. Example: Mount Object Storage bucket (using s3fs or rclone if installed)
# if [ -n "$S3_BUCKET" ]; then
#   echo "Mounting S3 Bucket..."
# fi

# 2. Network Optimizations for Scaleway
# (Optional) Tune sysctl for high bandwidth

# 3. Export specific env vars for vLLM
export VLLM_GPU_MEMORY_UTILIZATION=${scales_gpu_mem_util:-0.95}

echo "[Scaleway Flavor] Ready."
