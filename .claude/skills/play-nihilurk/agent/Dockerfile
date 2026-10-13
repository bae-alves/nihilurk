# A nihilurk run served over TCP, one percept per key: see serve.sh.
#
# The image holds no game. Build a static one on the host and mount it in:
#
#   cargo build -p nihilurk --target x86_64-unknown-linux-musl
#   docker build -t nihilurk-agent -f .claude/skills/play-nihilurk/agent/Dockerfile .claude/skills/play-nihilurk
#   docker run --rm -d -p 127.0.0.1:7777:7777 \
#       -v "$PWD/target/x86_64-unknown-linux-musl/debug/nihilurk:/usr/local/bin/nihilurk:ro" \
#       nihilurk-agent -s 1
#
# Arguments after the image name are game flags. NIHILURK_SPAWN,
# NIHILURK_LEVEL and NIHILURK_IDLE pass through with -e.

FROM debian:stable-slim
RUN apt-get update && apt-get install -y --no-install-recommends socat lua5.4 \
    && ln -s /usr/bin/lua5.4 /usr/local/bin/lua && rm -rf /var/lib/apt/lists/*
COPY screen.lua /agent/
COPY agent/serve.sh /agent/agent/
ENV TERM=xterm-256color NIHILURK_DATA=/tmp
EXPOSE 7777
ENTRYPOINT ["/agent/agent/serve.sh", "listen"]
