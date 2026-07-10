# First-party base image for HomeRun's Docker runner mode.
#
# This image intentionally does NOT bundle the GitHub Actions runner binary.
# HomeRun bind-mounts the runner (downloaded once, cached on the host — the
# same cache native runners use) into the container at start time, so this
# image only needs to provide an OS plus the runner's runtime dependencies
# and a general CI toolchain. Bring-your-own images only need to satisfy the
# same two things — see docs/DOCKER_RUNNERS.md.
FROM ubuntu:24.04

RUN apt-get update && apt-get install -y --no-install-recommends \
    bash \
    ca-certificates \
    curl \
    git \
    build-essential \
    libicu74 \
    libssl3 \
    libkrb5-3 \
    zlib1g \
    nodejs \
    npm \
    python3 \
    python3-pip \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /workspace
