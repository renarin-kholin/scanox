FROM rust
LABEL authors="renarin"
WORKDIR /usr/src/scanox
COPY ./target/release/ .

CMD ["./scanox-backend"]