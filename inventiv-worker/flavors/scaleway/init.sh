#!/bin/bash
# Scaleway-specific worker initialization
# This script is sourced by run_worker.sh before starting vLLM

set -e

echo "🔧 [Scaleway Flavor] Initializing Scaleway-specific configuration..."

# Storage Strategy (Official Scaleway Recommendations):
# - L4: Block Storage only (mounted at /opt/inventiv-worker)
# - L40S/H100: Scratch Storage (temporary) + Block Storage (persistent)
# - Scratch Storage: /scratch (if available on L40S/H100)
# - Block Storage: /opt/inventiv-worker (persistent data)

# Detect instance type from environment or system
INSTANCE_TYPE="${INSTANCE_TYPE:-}"
if [ -z "$INSTANCE_TYPE" ]; then
    # Try to detect from hostname or other sources
    INSTANCE_TYPE=$(hostname | grep -oE '(L4|L40S|H100|RENDER)' || echo "")
fi

# Setup Scratch Storage for L40S/H100 (if available)
# Scratch Storage is automatically mounted by Scaleway at /scratch on L40S/H100 instances
if [[ "$INSTANCE_TYPE" =~ ^(L40S|H100) ]]; then
    if [ -d "/scratch" ] && mountpoint -q /scratch 2>/dev/null; then
        echo "✅ [Scaleway Flavor] Scratch Storage detected at /scratch"
        echo "   - Use /scratch for temporary data (cache, intermediate results)"
        echo "   - Data on /scratch is lost on instance shutdown"
        echo "   - Backup important data to /opt/inventiv-worker regularly"
        
        # Create directories on Scratch Storage
        mkdir -p /scratch/cache
        mkdir -p /scratch/tmp
        
        # Set environment variables for Scratch Storage usage
        export SCRATCH_DIR="/scratch"
        export HF_HOME="${HF_HOME:-/scratch/cache/huggingface}"
        export TRANSFORMERS_CACHE="${TRANSFORMERS_CACHE:-/scratch/cache/transformers}"
    else
        echo "⚠️  [Scaleway Flavor] Scratch Storage not detected (expected on L40S/H100)"
    fi
fi

# Setup Block Storage mount point (persistent data)
# Block Storage should be mounted at /opt/inventiv-worker
PERSISTENT_DIR="/opt/inventiv-worker"
mkdir -p "$PERSISTENT_DIR"

# Check if Block Storage is mounted
if mountpoint -q "$PERSISTENT_DIR" 2>/dev/null; then
    echo "✅ [Scaleway Flavor] Block Storage detected at $PERSISTENT_DIR"
    echo "   - Use $PERSISTENT_DIR for persistent data (models, results, checkpoints)"
    echo "   - Data on $PERSISTENT_DIR survives instance shutdown"
else
    echo "⚠️  [Scaleway Flavor] Block Storage not mounted at $PERSISTENT_DIR"
    echo "   - Creating directory structure anyway"
fi

# Create directory structure
mkdir -p "$PERSISTENT_DIR/models"
mkdir -p "$PERSISTENT_DIR/results"
mkdir -p "$PERSISTENT_DIR/checkpoints"
mkdir -p "$PERSISTENT_DIR/logs"

# Set HuggingFace cache to use persistent storage (override Scratch Storage if Block Storage is mounted)
if mountpoint -q "$PERSISTENT_DIR" 2>/dev/null; then
    export HF_HOME="$PERSISTENT_DIR/huggingface"
    export TRANSFORMERS_CACHE="$PERSISTENT_DIR/huggingface/transformers"
    echo "✅ [Scaleway Flavor] Using Block Storage for HuggingFace cache: $HF_HOME"
fi

# Log storage configuration
echo "📦 [Scaleway Flavor] Storage Configuration:"
echo "   - Instance Type: ${INSTANCE_TYPE:-Unknown}"
if [ -d "/scratch" ] && mountpoint -q /scratch 2>/dev/null; then
    SCRATCH_SIZE=$(df -h /scratch | tail -1 | awk '{print $2}')
    echo "   - Scratch Storage: /scratch (${SCRATCH_SIZE:-N/A})"
fi
if mountpoint -q "$PERSISTENT_DIR" 2>/dev/null; then
    PERSISTENT_SIZE=$(df -h "$PERSISTENT_DIR" | tail -1 | awk '{print $2}')
    echo "   - Block Storage: $PERSISTENT_DIR (${PERSISTENT_SIZE:-N/A})"
else
    echo "   - Block Storage: Not mounted (will use local filesystem)"
fi

echo "✅ [Scaleway Flavor] Initialization complete"
