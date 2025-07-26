FROM rust:bookworm as builder

WORKDIR /app
COPY ./Cargo.toml ./Cargo.lock ./
COPY ./src ./src
RUN cargo build --release

FROM gcr.io/distroless/cc
COPY --from=builder /app/target/release/pdu_exporter /pdu_exporter

EXPOSE 9117
USER nonroot
ENTRYPOINT ["/pdu_exporter"]
