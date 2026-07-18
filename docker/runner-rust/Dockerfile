# HomeRun Rust runner image — a worked example of a purpose-built runner image.
#
# Extends the base image (OS + CI tooling + non-root `ubuntu` user) with a Rust
# toolchain so jobs labeled `rust` can run cargo without a per-job install. Use
# this same FROM-the-base pattern to build your own toolchain images.
FROM ghcr.io/agallea/homerun-runner:ubuntu-24.04

# The base image ends with `USER ubuntu`, so this RUN executes as the non-root
# ubuntu user and rustup installs into /home/ubuntu/.cargo.
#
# `--profile minimal` omits rustfmt/clippy, but real Rust CI needs them (plus
# llvm-tools-preview for coverage via cargo-llvm-cov), so add them explicitly.
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --default-toolchain stable --profile minimal \
        -c rustfmt -c clippy -c llvm-tools-preview
ENV PATH="/home/ubuntu/.cargo/bin:${PATH}"
