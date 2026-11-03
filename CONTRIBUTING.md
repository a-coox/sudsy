# Contributing

Contributions are welcome under the project's [MIT license](LICENSE).

## Sign-off (DCO)

Every commit in a pull request must be signed off, certifying the
[Developer Certificate of Origin](https://developercertificate.org/): that you wrote the change
or otherwise have the right to submit it under the project's license.

```
git commit -s -m "..."
```

This adds `Signed-off-by: Your Name <you@example.com>`, matching your commit author email. CI
checks it on every pull request (`scripts/check-dco.sh`); to sign off commits already on your
branch, run `git rebase --signoff origin/main` and force-push.

## Development

See [docs/development/dev-loop.md](docs/development/dev-loop.md): run `scripts/gate.sh` before
committing and `scripts/bench.sh` after a change to anything on a hot path.
