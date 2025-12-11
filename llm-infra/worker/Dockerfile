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
ENV ORCHESTRATOR_URL="http://orchestrator:8000"
ENV WORKER_ID="worker-uuid"

# The entrypoint launches both vLLM and the Agent Sidecar
ENTRYPOINT ["./run_worker.sh"]
