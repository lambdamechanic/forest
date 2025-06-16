# .devcontainer/Dockerfile
FROM mcr.microsoft.com/devcontainers/rust:latest

USER root
# Install extra dependencies required by Goose
RUN apt update && apt install -y \
    libxcb1 libxcb1-dev libdbus-1-3 nvi

USER vscode
# Install Goose preconfigured to use OpenRouter's o4-mini model
ENV GOOSE_PROVIDER=openrouter \
    GOOSE_MODEL=o4-mini
RUN curl -fsSL https://github.com/block/goose/releases/download/stable/download_cli.sh \
    | CONFIGURE=false bash

USER root
# Copy static Goose configuration for the vscode user
COPY goose-config.yaml /tmp/goose-config.yaml
RUN mkdir -p /home/vscode/.config/goose && \
    cp /tmp/goose-config.yaml /home/vscode/.config/goose/config.yaml && \
    chown -R vscode:vscode /home/vscode/.config/goose && \
    rm /tmp/goose-config.yaml

USER vscode
