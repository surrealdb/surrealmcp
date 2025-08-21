
FROM docker.io/rockylinux:9.3 AS builder-base

# Install base build tools and dependencies
RUN dnf update -y
RUN dnf install -y gcc gcc-c++ make git cmake openssl-devel zlib-devel python3.11 clang clang-devel llvm-devel
RUN dnf clean all

# Install Rust
ARG RUST_VERSION=1.89.0
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs > /tmp/rustup.sh
RUN bash /tmp/rustup.sh -y --default-toolchain ${RUST_VERSION}
ENV PATH="/root/.cargo/bin:${PATH}"

# Add Rust targets for cross-compilation
# RUN rustup target add x86_64-unknown-linux-gnu
# RUN rustup target add aarch64-unknown-linux-gnu

ENTRYPOINT [ "sh" ]


FROM builder-base AS build

WORKDIR /surrealmcp

COPY --link . .


FROM build AS build-debug

# Build for the native architecture of the current platform
RUN cargo build && \
    cp /surrealmcp/target/debug/surrealmcp /surrealmcp/surrealmcp

RUN rm -rf /surrealmcp/target /surrealmcp/src /surrealmcp/Cargo.*


FROM build AS build-release

# Build for the native architecture of the current platform
RUN cargo build --release && \
    cp /surrealmcp/target/release/surrealmcp /surrealmcp/surrealmcp

RUN chmod +x /surrealmcp/surrealmcp

RUN rm -rf /surrealmcp/target /surrealmcp/src /surrealmcp/Cargo.*


FROM cgr.dev/chainguard/glibc-dynamic:latest-dev AS dev

COPY --from=build-debug /surrealmcp/surrealmcp /surrealmcp

ENTRYPOINT ["/surrealmcp"]


FROM cgr.dev/chainguard/glibc-dynamic:latest AS prod

COPY --from=build-release /surrealmcp/surrealmcp /surrealmcp

USER 65532

ENTRYPOINT ["/surrealmcp"]
