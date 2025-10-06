# Contributing to minimap2-rs

Contributions of all kinds are welcome - can be as simple as issues, feature requests, or new features.

The goal is to have two separate libraries, a -sys library, that serves as a smooth surface for interacting with minimap2 via the FFI barrier, and minimap2-rs, which is more opinionated.

## Getting Started
1. **Fork** the repository on GitHub.  
2. **Clone** your fork locally and create a new branch for your changes:
```bash
   git clone https://github.com/your-username/minimap2-rs.git
   cd minimap2-rs
   git checkout -b feature/my-new-feature
```

## Code Style

We follow standard Rust conventions:

* Use `cargo fmt` to format code.
* `cargo clippy` to lint before submitting please.
* Keep functions small and well-documented with `///` doc comments. Add examples when possible.
* Add tests for each function you add!

## Making Changes

* Keep commits focused and descriptive.
* Update or add tests when you change behavior.
* Update documentation or README if relevant.
* Ensure all tests pass with `cargo test`.

## Testing

```bash
cargo test
```

Pull requests must pass CI before merging.

## Submitting Changes

1. Push your branch:

   ```bash
   git push origin feature/my-new-feature
   ```
2. Open a **Pull Request** on GitHub against the `main` branch.
3. Describe the change, motivation, and testing performed.
4. Be responsive to feedback from reviewers.

## Reporting Issues

If you find a bug, please:

* Check open issues first to avoid duplication.
* Include details about your OS, Rust version, and steps to reproduce.
* Use clear, concise language and provide minimal examples if possible.

## Feature Requests

We’re open to ideas!
If you have a new feature suggestion, open an issue labeled `enhancement` and explain:

* What problem it solves.
* How you envision it working.
* Any prior art or references.

## License

By contributing, you agree that your contributions will be licensed under the same
license as the project (see [LICENSE](LICENSE)).

## Code of Conduct

Please note that this project is released with a [Contributor Code of Conduct](CODE_OF_CONDUCT.md).
By participating, you agree to abide by its terms.

