# A nihilurk run served over TCP, one percept per key: see serve.sh.
#
# The image holds no game. Build a static one on the host and mount it in:
#
#   cargo build -p nihilurk --target x86_64-unknown-linux-musl
#   docker build -t nihilurk-agent -f .claude/skills/play-nihilurk/agent/Dockerfile .claude/skills/play-nihilurk
#   docker run --rm -d -p 127.0.0.1:7777:7777 \
#       --read-only --tmpfs /tmp --cap-drop ALL --security-opt no-new-privileges --pids-limit 128 \
#       -v "$PWD/target/x86_64-unknown-linux-musl/debug/nihilurk:/usr/local/bin/nihilurk:ro" \
#       nihilurk-agent -s 1
#
# Arguments after the image name are game flags. NIHILURK_SPAWN,
# NIHILURK_LEVEL and NIHILURK_IDLE pass through with -e. The server runs as an
# unprivileged user and listens on every interface inside the container, and
# the protocol has no login, so publish the port on 127.0.0.1 unless the
# player is on another machine you trust.

FROM alpine:3.24
RUN apk add --no-cache bash util-linux-misc flock socat lua5.4 \
    && ln -s /usr/bin/lua5.4 /usr/local/bin/lua \
    && adduser -S -D -H -s /sbin/nologin agent
COPY screen.lua /agent/
COPY agent/serve.sh /agent/agent/
ENV TERM=xterm-256color NIHILURK_BIND=0.0.0.0
USER agent
EXPOSE 7777
ENTRYPOINT ["/agent/agent/serve.sh", "listen"]
