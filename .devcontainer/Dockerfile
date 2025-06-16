# .devcontainer/Dockerfile
FROM mcr.microsoft.com/devcontainers/rust:latest

# Install extra dependencies required by Goose
RUN apt update && apt install -y \
    libxcb1 libxcb1-dev libdbus-1-3 nvi

# Install Goose preconfigured to use OpenRouter's o4-mini model
ENV GOOSE_PROVIDER=openrouter \
    GOOSE_MODEL=o4-mini
RUN curl -fsSL https://github.com/block/goose/releases/download/stable/download_cli.sh \
    | CONFIGURE=false bash

# Copy static Goose configuration to the default location for any user
COPY goose-config.yaml /tmp/goose-config.yaml
RUN mkdir -p /root/.config/goose && cp /tmp/goose-config.yaml /root/.config/goose/config.yaml && \
    if id vscode 2>/dev/null; then \
        mkdir -p /home/vscode/.config/goose && cp /tmp/goose-config.yaml /home/vscode/.config/goose/config.yaml; \
    fi && rm /tmp/goose-config.yaml
