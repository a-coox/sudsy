FROM alpine
^^^^^^^^^^^ untagged-base-image
FROM alpine:3 AS build
FROM build
FROM scratch
FROM alpine:3 AS build
^^^^^^^^^^^^^^^^^^^^^^ duplicate-stage-name
FROM $BASE
FROM localhost:5000/img
FROM alpine@sha256:abc
USER root
USER app
USER 0:0
^^^^^^^^ last-user-not-root
COPY --from=build /a /b
COPY --from=nope /a /b
^^^^^^^^^^^^^^^^^^^^^^ copy-from-unknown-stage
COPY --from=0 /a /b
COPY --from=9 /a /b
^^^^^^^^^^^^^^^^^^^ copy-from-unknown-stage
COPY --from=alpine:3 /a /b
CMD ["a"]
CMD ["b"]
^^^^^^^^^ single-cmd
ENTRYPOINT ["a"]
ENTRYPOINT ["b"]
^^^^^^^^^^^^^^^^ single-entrypoint
HEALTHCHECK CMD a
#           ^^^^^ json-cmd-entrypoint
HEALTHCHECK CMD b
^^^^^^^^^^^^^^^^^ single-healthcheck
#           ^^^^^ json-cmd-entrypoint
FROM alpine:3 AS last
COPY --from=last /a /b
^^^^^^^^^^^^^^^^^^^^^^ copy-from-own-stage
USER app
CMD ["a"]
USER root
^^^^^^^^^ last-user-not-root
