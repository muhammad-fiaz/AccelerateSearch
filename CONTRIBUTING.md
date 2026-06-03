# Contributing to AccelerateSearch

First, thank you for taking the time to contribute. This document covers the
local development workflow, code style, and pull-request process for
AccelerateSearch.

## Ground Rules

- Be respectful, patient, and welcoming. Read the
  [Code of Conduct](https://github.com/muhammad-fiaz/acceleratesearch/blob/main/CODE_OF_CONDUCT.md)
  if one is published in the repository.
- Open an issue before opening a large pull request so the design can be
  agreed upon first.
- Keep the change focused — one feature, one bug, one cleanup per PR.

## Local Setup

You need:

- **Rust** — always the latest stable. Never pin a specific version.
- **Git** — for source control.
- *(Optional)* `cargo-edit` for `cargo upgrade`.

```bash
git clone https://github.com/muhammad-fiaz/AccelerateSearch.git
cd AccelerateSearch
cargo check --workspace
cargo test --workspace
cargo bench --bench evaluate
```

## Project Layout

```
acceleratesearch/
├── src/                # binary entry point
├── crates/             # 30 modular sub-crates
├── benches/            # unified benchmark suite (evaluate.rs)
├── config/             # default TOML configuration
├── docs/               # design + user-facing documentation
├── .github/            # CI, release, issue templates
└── README.md
```

Every crate uses a short, simple name (`api`, `server`, `config`, etc.) and
references other crates as path dependencies declared in
`[workspace.dependencies]`.

## Code Style

- **Rust 2024** edition. `cargo fmt --all` must be a no-op before you push.
- **Zero `unwrap`/`expect` outside `#[cfg(test)]`** — use `?`, `map_err`,
  `ok_or_else`.
- **Zero `unsafe`** — enforced via `[lints.rust] unsafe_code = "deny"`.
- **Zero clippy warnings** under `cargo clippy --workspace --all-targets`.
- **All tests inline** in `#[cfg(test)]` modules at the bottom of the file
  they exercise. No separate `tests/` folder.
- **Public items must be documented** with `///` doc comments that explain
  *what* the item does, *when* to use it, and (where useful) a `# Examples`
  block.
- **Comments** — docstring comments only. Do not leave self-narrating
  comments such as "we do X here because Y". If the code is obvious, leave
  it uncommented; if it is subtle, document it in a docstring.

## Pull Request Workflow

1. **Create a feature branch** off `main`:
   ```bash
   git checkout -b feat/short-description
   ```
2. **Make focused commits** with messages that follow the
   [Conventional Commits](https://www.conventionalcommits.org/) style.
3. **Run the full validation suite locally**:
   ```bash
   cargo fmt --all
   cargo clippy --workspace --all-targets
   cargo test --workspace
   cargo build --release --workspace
   ```
4. **Push the branch** and open a PR against `main`. Fill in the
   pull-request template.
5. **Wait for CI to pass** on all three platforms (Linux, macOS, Windows).
6. **Address review feedback** by pushing additional commits to the same
   branch.

## Adding a Crate

If you need a new sub-crate, follow the pattern of the existing ones:

```bash
mkdir -p crates/mycrate/src
```

- `[package] name = "mycrate"`, `version.workspace = true`, `publish = false`.
- Add the crate to `[workspace] members` in the root `Cargo.toml`.
- Add a path entry to `[workspace.dependencies]`: `mycrate = { path = "crates/mycrate" }`.
- Add it to the diagram in `docs/architecture.md`.

## Reporting Bugs

Use the **Bug Report** issue template. Include:

- A minimal reproduction (commands, request, expected, actual).
- Your operating system, Rust version (`rustc --version`), and AccelerateSearch
  version (`accelerate version`).
- Relevant log lines (with `--log-level debug` if helpful).

## License

By contributing, you agree that your contributions will be licensed under the
[Apache License, Version 2.0](./LICENSE).
