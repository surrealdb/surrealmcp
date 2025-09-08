###
# STAGE: builder
# This stage is used to build the SurrealMCP linux binary
###

FROM docker.io/rockylinux/rockylinux:10 AS builder

RUN dnf install -y gcc-toolset-13 git cmake llvm-toolset patch zlib-devel python3.11

ARG RUST_VERSION=1.89.0
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs > /tmp/rustup.sh
RUN bash /tmp/rustup.sh -y --default-toolchain ${RUST_VERSION}
ENV PATH="/root/.cargo/bin:${PATH}"

ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=/opt/rh/gcc-toolset-13/root/usr/bin/aarch64-redhat-linux-gcc

ENTRYPOINT [ "sh" ]

###
# STAGE: build
# This stage is used to build the SurrealMCP linux binary
###

FROM builder AS build

WORKDIR /surrealmcp

COPY --link . .

###
# STAGE: build-debug
# This stage is used to build the SurrealMCP linux binary
###

FROM build AS build-debug

RUN . /opt/rh/gcc-toolset-13/enable && cargo build && cp /surrealmcp/target/debug/surrealmcp /surrealmcp/surrealmcp

RUN rm -rf /surrealmcp/target /surrealmcp/src /surrealmcp/Cargo.*

RUN chmod +x /surrealmcp/surrealmcp

###
# STAGE: build-release
# This stage is used to build the SurrealMCP linux binary
###

FROM build AS build-release

RUN . /opt/rh/gcc-toolset-13/enable && cargo build --release && cp /surrealmcp/target/release/surrealmcp /surrealmcp/surrealmcp

RUN rm -rf /surrealmcp/target /surrealmcp/src /surrealmcp/Cargo.*

RUN chmod +x /surrealmcp/surrealmcp

###
# STAGE: tzdata
# This stage is used to install the timezone files
###

FROM cgr.dev/chainguard/wolfi-base AS tzdata

RUN apk add --no-cache tzdata

#
# Development image (built locally)
#

FROM cgr.dev/chainguard/glibc-dynamic:latest-dev AS dev

COPY --from=build-debug /surrealmcp/surrealmcp /surrealmcp

COPY --from=tzdata /usr/share/zoneinfo /usr/share/zoneinfo

COPY --from=tzdata /usr/share/zoneinfo/UTC /etc/localtime

USER root

RUN mkdir /data /logs \
	&& chown -R nonroot:nonroot /data \
	&& chmod -R 777 /data \
	&& chown -R nonroot:nonroot /logs \
	&& chmod -R 777 /logs \
	&& echo "OK"

VOLUME /data /logs

ENTRYPOINT ["/surrealmcp"]

#
# Production image (built locally)
#
FROM cgr.dev/chainguard/glibc-dynamic:latest AS prod

COPY --from=build-release /surrealmcp/surrealmcp /surrealmcp

COPY --from=tzdata /usr/share/zoneinfo /usr/share/zoneinfo

COPY --from=tzdata /usr/share/zoneinfo/UTC /etc/localtime

COPY --from=dev /data /data

COPY --from=dev /logs /logs

VOLUME /data /logs

ENTRYPOINT ["/surrealmcp"]
