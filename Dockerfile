FROM rust:bookworm AS builder

WORKDIR /app
COPY ./Cargo.toml ./Cargo.lock ./
COPY ./src ./src
RUN cargo build --release
RUN mkdir -p /etc/pdu_exporter/{configs,logs}

FROM gcr.io/distroless/cc
COPY --from=builder /app/target/release/pdu_exporter /pdu_exporter
COPY --from=builder /etc/pdu_exporter /etc/pdu_exporter

EXPOSE 9117
USER nonroot
ENTRYPOINT ["/pdu_exporter"]