# Image that records the README demo (demo/cdc.tape). Built by
# demo/record.sh from `git archive HEAD`, so the volt binary in the
# recording is the committed source. See book/CONTRIBUTING-BOOK.md,
# "The README demo".

# Static (musl) build of volt: runs in the VHS image whatever its libc.
FROM rust:1.95-alpine AS build
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY . .
ENV CARGO_BUILD_JOBS=2
RUN cargo build --release --locked -p volt-driver --bin volt

# VHS renders the tape in a headless terminal (ttyd) and encodes the GIF
# (ffmpeg); both ship in this image. nano is the editor in the recording.
FROM ghcr.io/charmbracelet/vhs:v0.12.1
RUN apt-get update \
    && apt-get install -y --no-install-recommends nano \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/volt /usr/local/bin/volt
