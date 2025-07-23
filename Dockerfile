FROM docker.io/rust:bookworm as rust-builder
COPY ./Cargo* /app/
COPY ./src /app/src
WORKDIR /app
RUN cargo install --path . --root . --profile release \
  && strip /app/bin/pdu_exporter

FROM docker.io/debian:bookworm-slim
ENTRYPOINT ["/bin/pdu_exporter"]

COPY --from=rust-builder /app/bin/pdu_exporter /bin/pdu_exporter
