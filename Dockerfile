FROM rust:1.82-alpine3.20 AS builder

WORKDIR /cwe_checker

RUN apk add --no-cache musl-dev

COPY . .
RUN if [ $(uname -m) = "aarch64" ]; then \
        export TARGET="aarch64-unknown-linux-musl"; \
    else \
        export TARGET="x86_64-unknown-linux-musl"; \
    fi && \
    cargo build --release --target "$TARGET" --locked

FROM ghcr.io/fkie-cad/ghidra_headless_base:v11.2.1 AS runtime

RUN apk add --no-cache bash

ENV USER=cwe
ENV GROUPNAME=cwe
ENV UID=1000
ENV GID=1000

RUN addgroup --gid "$GID" "$GROUPNAME" \
    && adduser \
        --disabled-password \
        --gecos "" \
        --home "/home/cwe" \
        --ingroup "$GROUPNAME" \
        --no-create-home \
        --uid "$UID" \
        $USER

RUN mkdir -p /home/cwe \
    && mkdir -p /home/cwe/.config/ghidra/${GHIDRA_VERSION_NAME} \
    && chown -R cwe:cwe /home/cwe

USER cwe

# Install all necessary files from the builder stage
COPY --chown=${USER} --from=builder /cwe_checker/target/*/release/cwe_checker /home/cwe/cwe_checker
COPY --chown=${USER} --from=builder /cwe_checker/src/config.json /home/cwe/.config/cwe_checker/config.json
COPY --chown=${USER} --from=builder /cwe_checker/src/lkm_config.json /home/cwe/.config/cwe_checker/lkm_config.json
COPY --chown=${USER} --from=builder /cwe_checker/src/ghidra/p_code_extractor /home/cwe/.local/share/cwe_checker/ghidra/p_code_extractor
RUN echo "{ \"ghidra_path\": \"/opt/ghidra\" }" | tee /home/cwe/.config/cwe_checker/ghidra.json

WORKDIR /

ENV RUST_BACKTRACE=1
ENTRYPOINT ["/home/cwe/cwe_checker"]
