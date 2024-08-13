FROM --platform=$TARGETPLATFORM messense/rust-musl-cross:x86_64-musl AS base-amd64
ARG TARGET="x86_64-unknown-linux-musl"
FROM --platform=$TARGETPLATFORM alpine AS alpine-amd64
ARG TARGET="x86_64-unknown-linux-musl"

FROM --platform=$TARGETPLATFORM messense/rust-musl-cross:aarch64-musl AS base-arm64
ARG TARGET="aarch64-unknown-linux-musl"
FROM --platform=$TARGETPLATFORM alpine AS alpine-arm64
ARG TARGET="aarch64-unknown-linux-musl"

# ======================== SETUP RUST ========================
FROM base-$TARGETARCH AS base
WORKDIR /eve
RUN rustup update nightly && \
    rustup target add --toolchain nightly $TARGET && \
    rustup default nightly

# ======================== BUILD DEPS ========================
FROM base AS build-deps
COPY Cargo.lock .
COPY Cargo.toml .
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -r ./src

# ======================== BUILD PROJECT ========================
FROM build-deps AS build
COPY src src
RUN touch src/main.rs
RUN cargo build --release && \
    musl-strip ./target/$TARGET/release/eve

# ======================== GET JAVA ========================
FROM alpine-$TARGETARCH as java

ARG TARGETARCH
ARG JAVA_URL_amd64
ARG JAVA_URL_arm64

RUN if [ "$TARGETARCH" = "amd64" ]; then \
        JAVA_URL=$JAVA_URL_amd64; \
    elif [ "$TARGETARCH" = "arm64" ]; then \
        JAVA_URL=$JAVA_URL_arm64; \
    fi && \
    cd ~/ && wget --no-check-certificate -O java_jdk.tar.gz -c $JAVA_URL

RUN mkdir /usr/lib/jvm && \
    tar -xvzf ~/java_jdk.tar.gz -C /usr/lib/jvm

# ======================== FINAL ========================
FROM alpine-$TARGETARCH

ENV TZ=Europe/Zurich

ARG JAVA_JDK

RUN apk add --no-cache dpkg
COPY --from=java /usr/lib/jvm/ /usr/lib/jvm/
RUN echo "PATH=\"/usr/lib/jvm/$JAVA_JDK/bin\"" > /etc/environment
RUN echo "JAVA_HOME=\"/usr/lib/jvm/$JAVA_JDK\"" >> /etc/environment
RUN update-alternatives --install "/usr/bin/java" "java" "/usr/lib/jvm/$JAVA_JDK/bin/java" 0
RUN update-alternatives --install "/usr/bin/javac" "javac" "/usr/lib/jvm/$JAVA_JDK/bin/javac" 0
RUN update-alternatives --set java /usr/lib/jvm/$JAVA_JDK/bin/java
RUN update-alternatives --set javac /usr/lib/jvm/$JAVA_JDK/bin/javac

USER 405
WORKDIR /eve
COPY --from=build /eve/target/$TARGET/release/eve ./

CMD ["./eve"]
