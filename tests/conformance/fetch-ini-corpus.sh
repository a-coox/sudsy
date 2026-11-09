#!/usr/bin/env bash
# Fetches the pinned INI corpus into tests/conformance/corpus/ini/<repo> (gitignored): the
# `.cfg` and `.ini` files of ansible and php-src's php.ini-development (renamed php.ini).
source "$(dirname "$0")/tarball-corpus.sh"
fetch_tarballs ini <<'EOF'
ansible/ansible fd540eda9f96783955b227e84e229ca51f041cae */targets/config/*.cfg */targets/config/files/*.ini */targets/ansible-config/files/base_*valid.cfg */targets/ansible-config/files/galaxy_server.ini */galaxy/data/apb/tests/ansible.cfg */targets/connection_ssh/*.cfg
php/php-src 37e90b982cbc9324c0635fd1170c917c45247ee4 */php.ini-development
EOF
mv -n "$(dirname "$0")/corpus/ini/php-src/php.ini-development" "$(dirname "$0")/corpus/ini/php-src/php.ini" 2>/dev/null || true
