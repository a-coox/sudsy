#!/usr/bin/env bash
# Fetches the pinned INI corpus into tests/conformance/corpus/ini/<repo> (gitignored): the `.ini`
# files of ansible, php-src's php.ini-development (renamed php.ini), the tox.ini and setup.cfg of
# pytest, setuptools and flake8 (continuation lines) and a `.gitconfig` (renamed gitconfig.cfg;
# indented settings, keys without `=`).
source "$(dirname "$0")/tarball-corpus.sh"
fetch_tarballs ini <<'EOF'
ansible/ansible fd540eda9f96783955b227e84e229ca51f041cae */targets/config/files/*.ini */targets/ansible-config/files/galaxy_server.ini
php/php-src 37e90b982cbc9324c0635fd1170c917c45247ee4 */php.ini-development
pytest-dev/pytest 1b6f4419c6fe9371a8d8c52c12ed0a1577c2504a */tox.ini
pypa/setuptools 049ae8e3083514378aedb352a2017fbc306bd126 */tox.ini
PyCQA/flake8 1a50f8923b5aec2bef0f3e5ee84cf78ffc821fd5 */setup.cfg */tox.ini
mathiasbynens/dotfiles b7c7894e7bb2de5d60bfb9a2f5e46d01a61300ea */.gitconfig
EOF
mv -n "$(dirname "$0")/corpus/ini/php-src/php.ini-development" "$(dirname "$0")/corpus/ini/php-src/php.ini" 2>/dev/null || true
mv -n "$(dirname "$0")/corpus/ini/dotfiles/.gitconfig" "$(dirname "$0")/corpus/ini/dotfiles/gitconfig.cfg" 2>/dev/null || true
