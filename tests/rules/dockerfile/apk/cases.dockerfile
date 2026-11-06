FROM alpine:3
RUN apk add curl
^^^^^^^^^^^^^^^^ apk-no-cache
^^^^^^^^^^^^^^^^ apk-pin-versions
RUN apk add --no-cache curl=8.5.0-r0
RUN apk add --no-cache --virtual .deps gcc=13.2.1-r0 musl-dev=1.2.4-r2
RUN apk add --no-cache ./pkg.apk
RUN --mount=type=cache,target=/var/cache/apk apk add curl=8
