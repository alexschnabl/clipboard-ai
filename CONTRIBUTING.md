# Contributing

Bug reports and small fixes are welcome any time. For a larger change, open an issue first so the approach is agreed before you spend the time.

## Setup

The README's "Build from source" and "Develop" sections cover the toolchain (Rust, Node, the Tauri CLI) and the dev loop. Windows is the only platform for now.

## Before you open a pull request

- `cargo test` in `src-tauri` and `npm run check` pass.
- You tried the change in at least one real target app (Notepad, Outlook, Word, a browser), not only in tests. Say which in the pull request.
- Commits are small, carry a conventional prefix (`feat:`, `fix:`, `docs:`, `chore:`) and a body that says why.
- The README stays in plain prose: hyphens, no em dashes, no emoji, no marketing adjectives.

## What is out of scope for now

Other operating systems, an auto-updater and a signed installer. Issues about them are fine, but they are not planned.
