# syntax=docker/dockerfile:1

ARG RUST_VERSION=1.85.0

################################################################################
# Create a stage for building the application.

FROM rust:${RUST_VERSION}-alpine AS build
WORKDIR /app

# Install host build dependencies.
RUN apk add --no-cache clang lld musl-dev git

# Build the application.
# Leverage a cache mount to /usr/local/cargo/registry/
# for downloaded dependencies, a cache mount to /usr/local/cargo/git/db
# for git repository dependencies, and a cache mount to /app/target/ for
# compiled dependencies which will speed up subsequent builds.

# Leverage a bind mount to the workspace source directories to avoid having to
# copy the source code into the container. Once built, copy the executables to
# an output directory before the cache mounted /app/target is unmounted.
RUN --mount=type=bind,source=ztop-core,target=ztop-core \
    --mount=type=bind,source=ztop,target=ztop \
    --mount=type=bind,source=ztop-exporter,target=ztop-exporter \
    --mount=type=bind,source=vendor,target=vendor \
    --mount=type=bind,source=.cargo,target=.cargo \
    --mount=type=bind,source=Cargo.toml,target=Cargo.toml \
    --mount=type=bind,source=Cargo.lock,target=Cargo.lock \
    --mount=type=cache,target=/app/target/ \
    --mount=type=cache,target=/usr/local/cargo/git/db \
    --mount=type=cache,target=/usr/local/cargo/registry/ \
    cargo build --locked --release && \
    cp ./target/release/ztop /bin/ztop && \
    cp ./target/release/ztop-exporter /bin/ztop-exporter

################################################################################

# Create a new stage for running the application that contains the minimal
# runtime dependencies for the application.

FROM alpine:3.18 AS final

# Create a non-privileged user that the app will run under.
ARG UID=10001
RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    appuser
USER appuser

COPY --from=build /bin/ztop /bin/
COPY --from=build /bin/ztop-exporter /bin/

# Default to ztop-exporter; override with /bin/ztop for the TUI
ENTRYPOINT ["/bin/ztop-exporter"]
