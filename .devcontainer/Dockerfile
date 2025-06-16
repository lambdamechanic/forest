# .devcontainer/Dockerfile
FROM mcr.microsoft.com/devcontainers/rust:latest
RUN apt update && apt install -y libxcb1 libxcb1-dev libdbus-1-3 nvi
RUN curl -fsSL https://github.com/block/goose/releases/download/stable/download_cli.sh | CONFIGURE=false bash
