FROM debian
LABEL authors="renarin"
WORKDIR /usr/src/scanox
COPY ./target/release/ .
RUN apt-get update && apt-get install -y openssl ca-certificates

CMD ["./scanox-backend"]
