#!/usr/bin/env bash
# Fetches the pinned Dockerfile corpus into tests/conformance/corpus/dockerfile/<repo> (gitignored):
# the .NET samples and images, official images (golang, python, nginx, postgres) and the
# Dockerfiles of moby and buildx (heredocs, mounts, many stages).
source "$(dirname "$0")/tarball-corpus.sh"
fetch_tarballs dockerfile <<'EOF'
dotnet/dotnet-docker aebbe2e6298be734f78ac4142dc147b4721f0cc0 */samples/*Dockerfile* */src/runtime/10.0/*/amd64/Dockerfile */src/sdk/10.0/*/amd64/Dockerfile */src/aspnet/10.0/*/amd64/Dockerfile
docker-library/golang 236db66f2a220ba203e8cea391499d33d13124e4 */Dockerfile
docker-library/python f4c67ece208dead594e43628d22e3c6027dd97ab */Dockerfile
docker-library/nginx ab68aadec45fb303c14017d568eb4212be0a243c */Dockerfile
docker-library/postgres dfc76bb299b057785026eb2eea39556b6fe52983 */Dockerfile
moby/moby c62c973be7b1827388205875fb35e90f7fb22bff */Dockerfile */Dockerfile.*
docker/buildx b8a908fd6b0f8d66b52e7ea39c1acc96435eceff */Dockerfile
EOF
