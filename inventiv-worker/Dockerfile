# Base image with vLLM installed
FROM vllm/vllm-openai:latest

# Install python dependencies for our agent sidecar
RUN pip install requests pydantic

# Create worker directory
WORKDIR /app
COPY agent.py .
COPY run_worker.sh .

# Make scripts executable
RUN chmod +x run_worker.sh

# Environment variables
ENV CONTROL_PLANE_URL="http://orchestrator:8001"
ENV WORKER_ID="worker-uuid"
ENV WORKER_HEALTH_PORT="8080"
ENV WORKER_VLLM_PORT="8000"

# The entrypoint launches both vLLM and the Agent Sidecar
ENTRYPOINT ["./run_worker.sh"]
