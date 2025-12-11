#!/bin/bash

# 0. Load Flavor Specifics
if [ -n "$FLAVOR" ] && [ -f "flavors/$FLAVOR/init.sh" ]; then
    source "flavors/$FLAVOR/init.sh"
else
    echo "No flavor specified or init script not found. Using default."
fi

# 1. Start vLLM Server in background
# We pass through arguments to vLLM (model, etc)
python3 -m vllm.entrypoints.openai.api_server "$@" &
VLLM_PID=$!

echo "vLLM started with PID $VLLM_PID"

# 2. Wait for vLLM to be ready (optional check here or inside agent.py)

# 3. Start Agent Sidecar
python3 agent.py &
AGENT_PID=$!

echo "Agent Started with PID $AGENT_PID"

# 4. Wait for any process to exit
wait -n $VLLM_PID $AGENT_PID

# Exit with status of process that exited first
exit $?
