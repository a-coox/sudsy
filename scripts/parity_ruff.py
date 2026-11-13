"""Runs the ruff installed by parity_format.py (`pip install --target <prefix>`): parity_ruff.py <prefix> <ruff arguments>."""
import subprocess
import sys

sys.path.insert(0, sys.argv[1])
from ruff.__main__ import find_ruff_bin

sys.exit(subprocess.call([find_ruff_bin(), *sys.argv[2:]]))
