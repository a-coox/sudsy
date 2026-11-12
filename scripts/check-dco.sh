#!/usr/bin/env bash
# Developer Certificate of Origin check: every non-merge commit in BASE..HEAD carries a
# `Signed-off-by:` trailer with its author's email (`git commit -s`). Usage: check-dco.sh BASE [HEAD]
set -euo pipefail
base=${1:?usage: check-dco.sh BASE [HEAD]}
head=${2:-HEAD}
missing=0
for sha in $(git rev-list --no-merges "$base..$head"); do
  email=$(git log -1 --format=%ae "$sha")
  signed=0
  while IFS= read -r line; do
    [[ ${line,,} == signed-off-by:*"<${email,,}>"* ]] && signed=1
  done < <(git log -1 --format=%B "$sha")
  if [ "$signed" = 0 ]; then
    echo "missing sign-off by <$email>: $(git log -1 --format='%h %s' "$sha")"
    missing=1
  fi
done
if [ "$missing" = 1 ]; then
  echo "Sign each commit with 'git commit -s' (see CONTRIBUTING.md); fix existing ones with 'git rebase --signoff $base'."
  exit 1
fi
echo "DCO OK"
