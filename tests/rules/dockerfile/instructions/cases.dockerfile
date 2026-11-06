FROM alpine:latest
^^^^^^^^^^^^^^^^^^ no-latest-tag
FROM --platform=linux/amd64 alpine:3
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ no-from-platform
FROM --platform=$TARGETPLATFORM alpine:3
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ no-redundant-target-platform
FROM --platform=$BUILDPLATFORM alpine:3 AS scratch
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ no-reserved-stage-name
FROM alpine:3@sha256:abc
WORKDIR app
^^^^^^^^^^^ workdir-absolute
WORKDIR /app
WORKDIR $HOME
EXPOSE 8080 70000
^^^^^^^^^^^^^^^^^ expose-port-range
EXPOSE 80/tcp 443
CMD node server.js
^^^^^^^^^^^^^^^^^^ json-cmd-entrypoint
CMD ["node", "server.js"]
^^^^^^^^^^^^^^^^^^^^^^^^^ single-cmd
ENTRYPOINT /run.sh
^^^^^^^^^^^^^^^^^^ json-cmd-entrypoint
ONBUILD CMD echo hi
ONBUILD ONBUILD RUN x
^^^^^^^^^^^^^^^^^^^^^ no-nested-onbuild
MAINTAINER a@b.c
^^^^^^^^^^^^^^^^ no-maintainer
COPY --from=0 / /
^^^^^^^^^^^^^^^^^ no-copy-whole-stage
COPY a b /dest
^^^^^^^^^^^^^^ copy-many-needs-directory
COPY a b /dest/
COPY a /dest
ADD app.js /app/
^^^^^^^^^^^^^^^^ copy-instead-of-add
ADD app.tar.gz /app/
ADD https://example.com/f /app/
USER app
^^^^^^^^ numeric-user-id
USER 1000:1000
USER $UID
ENV DB_PASSWORD=x
^^^^^^^^^^^^^^^^^ no-secret-in-env
ENV A=1 NPM_TOKEN=y
^^^^^^^^^^^^^^^^^^^ no-secret-in-env
ARG GITHUB_TOKEN
^^^^^^^^^^^^^^^^ no-secret-in-env
ARG VERSION=1
ENV AWS_ACCESS_KEY_ID=x
^^^^^^^^^^^^^^^^^^^^^^^ no-secret-in-env
