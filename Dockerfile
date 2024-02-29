FROM rust:1.75.0 as builder

WORKDIR /usr/src/livepeer-funds-transfer
COPY . .
RUN cargo install --path .

FROM debian:latest
#RUN apt-get update && apt upgrade -y && apt-get -y install openssl ca-certificates  && apt-get clean  && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/funds_transfer /usr/local/bin/funds_transfer
WORKDIR /root/
CMD ["funds_transfer"]