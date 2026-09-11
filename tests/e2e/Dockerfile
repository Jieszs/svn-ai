FROM rust:1.85-bookworm

RUN apt-get update \
    && apt-get install -y --no-install-recommends subversion jq \
    && rm -rf /var/lib/apt/lists/*

