#!/usr/bin/env bash
# Fetches the pinned Makefile corpus into tests/conformance/corpus/makefile/<repo> (gitignored): every
# `Makefile` of redis, git, prometheus and docker/cli, kubernetes' `build/root/Makefile`, lua's
# `makefile`.
source "$(dirname "$0")/tarball-corpus.sh"
fetch_tarballs makefile <<'EOF'
redis/redis dda59d1f8e852a5add0e9952fe992ba2f14b1f14 */Makefile
git/git 3c60ef1e38ee80337025a1b908327f7879386329 */Makefile
lua/lua 8cb93bada561977eb370819e39d7d644e5fd0a3f */makefile
prometheus/prometheus 0b1983060510028164ab063444a0b64c0ca4b69d */Makefile
kubernetes/kubernetes ce68b65461ae66a3ab29c6dda1288afcf958591b */build/root/Makefile
docker/cli 7c0f1b261a24b36ad39b6031db6ffe41e60bc7e7 */Makefile
EOF
