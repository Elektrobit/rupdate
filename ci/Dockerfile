# SPDX-License-Identifier: MIT
ARG REPO
FROM ${REPO}${REPO:+/}ubuntu:jammy

ARG USER=ci
ARG UID=1000
ARG GID=1000

ENV DEBIAN_FRONTEND=noninteractive \
    RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH \
    RUST_VERSION=1.61

RUN apt-get update \
 && apt-get install -y sudo locales vim screen libncurses-dev file wget \
    build-essential git pkg-config python3-minimal cmake rpm \
 && rm -rf /var/lib/apt/lists/*

# Download and install rustup
RUN set -eux; \
    url="https://static.rust-lang.org/rustup/archive/1.25.1/$(uname -m)-unknown-linux-gnu/rustup-init"; \
    wget "$url"; \
    chmod +x rustup-init; \
    ./rustup-init -y --no-modify-path --profile minimal --default-toolchain ${RUST_VERSION}; \
    rm rustup-init; \
    chmod -R a+w $RUSTUP_HOME $CARGO_HOME; \
    rustup --version; \
    cargo --version; \
    rustc --version;

RUN locale-gen en_US.UTF-8 \
    && groupadd -g $GID -o ci \
    && useradd -m -u $UID -g $GID -o -s /bin/bash $USER \
    && echo "$USER ALL=(ALL) NOPASSWD:ALL" >> /etc/sudoers

USER $USER
ENV LC_ALL=en_US.UTF-8 \
    DOCKERBUILD=1 \
    RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH \
    RUST_VERSION=${RUST_VERSION}

# Install the clippy extension for linting.
RUN set -eux; \
    rustup toolchain install ${RUST_VERSION} \
        --component clippy

CMD /bin/bash
