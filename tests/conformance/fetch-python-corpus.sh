#!/usr/bin/env bash
# Fetches the pinned Python corpus into tests/conformance/corpus/python/<repo> (gitignored): the
# standard library of CPython 3.14.10 (its tests hold the syntax errors), Black 26.10.1's own test
# cases and source, ruff 0.17.4's formatter and parser fixtures (the formatter fixtures come with
# their `*.options.json`), and Black-formatted projects: Django 5.2.8, requests 2.34.2, Flask
# 3.1.3, Click 8.5.1, httpx 0.28.1 and pytest 9.2.0.
source "$(dirname "$0")/tarball-corpus.sh"
fetch_tarballs python <<'EOT'
python/cpython b703b2c5e704341d61e367b9b940b77ed11dded0 */Lib/*.py
psf/black 258e7dbf348bc23cb70b8cdf693481c04634ebcc */tests/data/* */src/*.py */action/*.py */docs/*.py
astral-sh/ruff 05cdc36a45a29e078c2af9358969d0f576408bef */crates/ruff_python_formatter/resources/test/fixtures/* */crates/ruff_python_parser/resources/*
django/django 47fe39af56ecd0ad73b9c7562511015e96b91b80 */django/*.py */tests/*.py
psf/requests 6e83187b8feb273ed4c6cdab5efd8d54901dfab3 */src/requests/*.py */tests/*.py
pallets/flask 22d924701a6ae2e4cd01e9a15bbaf3946094af65 */src/flask/*.py */tests/*.py
pallets/click cf19179e8e2fd54c601378cec9e0d21bb12b6fb1 */src/click/*.py */tests/*.py
encode/httpx 26d48e0634e6ee9cdc0533996db289ce4b430177 */httpx/*.py */tests/*.py
pytest-dev/pytest ae1d0ff57b8bd4252da9ade7353523ac0ac73281 */src/_pytest/*.py
EOT
