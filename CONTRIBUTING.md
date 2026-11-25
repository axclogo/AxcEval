# Contributing

Thank you for helping improve AxcEval.

1. Open an issue for protocol or execution-semantic changes before implementation.
2. Add or update protocol fixtures for observable behavior changes.
3. Keep the core deterministic and free of I/O.
4. Run formatting, Clippy and all workspace tests before submitting a pull request.
5. Update `CHANGELOG.md` for user-visible changes.
6. Apply every rule in [`docs/ENGINEERING.md`](docs/ENGINEERING.md), including the one-semantic-path and repository
   hygiene requirements.

Public APIs and issue codes follow semantic versioning. New failure kinds need fixtures and documentation. Do not weaken
tests or artifact size checks to make a change pass.
