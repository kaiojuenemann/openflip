# Contributing to openflip

Thanks for your interest! openflip is a small project, so the process is light.

## Before you start

- For anything bigger than a typo or small fix, open an issue first so we can
  agree on the approach.
- Check the [roadmap](docs/ROADMAP.md) and open issues to avoid duplicate work.

## Development

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

CI runs exactly these checks on macOS, Windows and Linux. Please make sure they
pass locally before opening a pull request.

## Guidelines

- Keep `openflip-core` free of windowing, GPU and OS-specific code.
- Platform-specific code goes behind `#[cfg(target_os = "...")]` or into
  `platform/<os>`.
- New user-facing strings need an English and a German translation.
- Write tests for logic you add to `openflip-core`.
- One topic per pull request; describe what you changed and how you tested it,
  including on which operating system.

## Commit messages

Use the imperative mood ("Add date line", not "Added date line") and keep the
first line under ~72 characters.

## License

By contributing you agree that your contributions are licensed under the
[MIT license](LICENSE).
