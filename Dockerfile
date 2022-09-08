FROM messense/rust-musl-cross:x86_64-musl as builder

WORKDIR /eve

RUN rustup update nightly
RUN rustup target add --toolchain nightly x86_64-unknown-linux-musl

COPY Cargo.lock .
COPY Cargo.toml .
COPY ./src ./src/

RUN cargo +nightly -Z sparse-registry build --release
RUN musl-strip ./target/x86_64-unknown-linux-musl/release/eve

FROM alpine as java

ARG JAVA_URL
RUN cd ~/ && wget --no-check-certificate -O java_jdk.tar.gz -c $JAVA_URL
RUN mkdir /usr/lib/jvm
RUN cd /usr/lib/jvm && tar -xvzf ~/java_jdk.tar.gz

FROM alpine

ENV TZ=Europe/Zurich

ARG JAVA_JDK

# Install java jdk 17
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

COPY --from=builder /eve/target/x86_64-unknown-linux-musl/release/eve ./

CMD ["./eve"]
