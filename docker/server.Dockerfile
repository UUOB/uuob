# UUOB without a window, for a server or a Raspberry Pi (see README, "Without a window").
#   docker build -f docker/server.Dockerfile -t uuob .
#   docker run -d --name uuob --restart unless-stopped --network host -v uuob-data:/data uuob
#   docker exec uuob uuob join <code>
#
# The program is built on the machine that runs the build, also when the
# image is for another kind of processor (--platform linux/arm64 on a PC).
FROM --platform=$BUILDPLATFORM rust:1-bookworm AS build
ARG TARGETARCH
RUN case "$TARGETARCH" in \
      arm64) echo aarch64-unknown-linux-gnu ;; \
      amd64) echo x86_64-unknown-linux-gnu ;; \
      *) echo "unsupported: $TARGETARCH" >&2; exit 1 ;; \
    esac > /triple \
 && rustup target add "$(cat /triple)" \
 && apt-get update && apt-get install -y --no-install-recommends gcc-aarch64-linux-gnu gcc-x86-64-linux-gnu libc6-dev-arm64-cross libc6-dev-amd64-cross \
 && rm -rf /var/lib/apt/lists/*
ENV CARGO_PROFILE_RELEASE_STRIP=symbols \
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
    CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc \
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc \
    CC_x86_64_unknown_linux_gnu=x86_64-linux-gnu-gcc
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/src/target,id=uuob-server-$TARGETARCH \
    cargo build --release --locked -p uuob-cli --target "$(cat /triple)" \
 && cp "target/$(cat /triple)/release/uuob" /uuob

# Just the program (for `docker build --target program -o <folder>`).
FROM scratch AS program
COPY --from=build /uuob /uuob

FROM debian:bookworm-slim
COPY --from=build /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY --from=build /uuob /usr/local/bin/uuob
ENV UUOB_DATA_DIR=/data
VOLUME /data
CMD ["uuob", "run"]
