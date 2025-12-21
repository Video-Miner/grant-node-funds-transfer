FROM rust:1.90.0 as builder

WORKDIR /usr/src/livepeer-funds-transfer
COPY . .
RUN cargo install --path .

FROM debian:latest
COPY --from=builder /usr/local/cargo/bin/funds_transfer /usr/local/bin/funds_transfer
WORKDIR /root/
CMD ["funds_transfer"]