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

# Run as a non-root user by default: defense-in-depth (a job's code doesn't
# get container-root) and it keeps files the runner writes into the
# bind-mounted workspace owned by a normal uid rather than root. ubuntu:24.04
# already ships a `ubuntu` user at uid 1000. Bring-your-own images that run as
# root still work — HomeRun sets RUNNER_ALLOW_RUNASROOT for them.
RUN mkdir -p /workspace && chown ubuntu:ubuntu /workspace
USER ubuntu

WORKDIR /workspace
