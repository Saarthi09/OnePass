# One container that runs the whole OnePass demo: the stand-in institutions
# (onepass-sources, internal only, port 4001) and OnePass itself
# (onepass-server, public, on $PORT). Used for Railway / Render / Fly.

# ---- build ----
FROM rust:1-bookworm AS build
WORKDIR /app
COPY . .
RUN cargo build --release -p onepass-server -p onepass-sources

# ---- run ----
FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl libdbus-1-3 libudev1 \
 && rm -rf /var/lib/apt/lists/*

# Stellar CLI, used by the server to read the registry and record receipts.
ARG STELLAR_CLI_VERSION=30.0.1
RUN curl -fsSL "https://github.com/stellar/stellar-cli/releases/download/v${STELLAR_CLI_VERSION}/stellar-cli-${STELLAR_CLI_VERSION}-x86_64-unknown-linux-gnu.tar.gz" \
    | tar -xz -C /usr/local/bin stellar

WORKDIR /app
COPY --from=build /app/target/release/onepass-server /app/target/release/onepass-sources /usr/local/bin/
COPY stellar-registry.txt scripts/start.sh ./
RUN chmod +x start.sh

ENV PORT=4000
EXPOSE 4000
CMD ["./start.sh"]
