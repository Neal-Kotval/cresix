FROM node:22-alpine AS web-build
WORKDIR /src/web
COPY web/package.json web/package-lock.json* ./
RUN npm ci
COPY web/ ./
RUN npm run build

FROM rust:1.95-bookworm AS rust-build
WORKDIR /src
COPY Cargo.toml Cargo.lock* ./
COPY crates/ ./crates/
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl git openssh-client \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 c6 \
    && install -d -o c6 -g c6 -m 0700 /var/lib/c6 /var/lib/c6-runner \
    && install -d -o c6 -g c6 -m 0700 /run/c6
COPY --from=rust-build /src/target/release/c6-server /usr/local/bin/c6-server
COPY --from=rust-build /src/target/release/c6-runner /usr/local/bin/c6-runner
COPY --from=web-build /src/web/dist /opt/c6/web
USER c6
EXPOSE 8787
ENTRYPOINT ["/usr/local/bin/c6-server"]
