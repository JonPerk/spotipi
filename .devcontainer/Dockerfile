# syntax=docker/dockerfile:1
ARG debian_version=slim-bookworm
ARG rust_version=1.82.0
FROM rust:${rust_version}-${debian_version}

ARG DEBIAN_FRONTEND=noninteractive
ENV CARGO_REGISTRIES_CRATES_IO_PROTOCOL="sparse"
ENV RUST_BACKTRACE=1
# ENV RUSTFLAGS="-D warnings"


RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        git \
        nano\
        openssh-server  \
        # for rust-analyzer vscode plugin
        pkg-config \
        # developer dependencies
        libunwind-dev \
        libpulse-dev \
        portaudio19-dev \
        libasound2-dev \
        libsdl2-dev \
        gstreamer1.0-dev \
        libgstreamer-plugins-base1.0-dev \
        libavahi-compat-libdnssd-dev \
        build-essential \
        libp8-platform-dev \
        cmake \
        libudev-dev \
        libxrandr-dev \
        python3-dev \
        swig && \
    rm -rf /var/lib/apt/lists/*

RUN rustup component add rustfmt && \
    rustup component add clippy && \
    cargo install cargo-hack

RUN git clone https://github.com/Pulse-Eight/libcec.git && \
    mkdir libcec/build && \
    # latest is broken so revert to working commit a2b589a987e2b409e61b04126aadca7c441138bc 88846e801661cd68db146593197a503f676f2183 bf1359face011f554eb82404712d9520127ff5a3
	cd libcec && \
	git reset --hard 88846e801661cd68db146593197a503f676f2183 && \
	cd ../ && \
	# resume normal operations
    cd libcec/build && \
    cmake .. && \
    make -j4 && \
    make install && \
    ldconfig && \
    cd ../../
