<p align="center">
  <img src="src-tauri/icons/128x128@2x.png" alt="Clipboard AI logo" width="120" height="120" />
</p>

<h1 align="center">Clipboard AI</h1>

<p align="center">Highlight text anywhere in Windows, press a hotkey, and an AI rewrites it in place.</p>

<!-- TODO(gif): the hero loop - select a sentence in Notepad, press the hotkey, tray pulses, corrected text lands. -->

Clipboard AI sits in the tray. You set up **commands** - each is a hotkey, a prompt, and a
backend - and pressing a command's hotkey over any selected text sends it to the AI and pastes
the result back over your selection. That's the whole app. It ships with one command, "Fix
grammar" on `Ctrl+Alt+J`.

Backends can be:

- **OpenAI-compatible** - any `/chat/completions` endpoint (a local Ollama or LM Studio, or OpenAI).
- **Anthropic API** - the Messages API with your own key.
- **Claude CLI** - your Claude subscription through the `claude` command. No API key.

## Build

There's no packaged download yet, so build from source. You need [Node](https://nodejs.org/), a
[Rust toolchain](https://rustup.rs/), and on Windows the MSVC build tools ("Desktop development
with C++").

```sh
npm install
npm run tauri build
```

The installer ends up in `src-tauri/target/release/bundle/`.

## Use it

First launch puts an icon in the tray - left-click it to open settings. Add a backend, add a
command (a name, a hotkey, a prompt), and that's it. Edits apply straight away, no restart. While
a command runs the tray icon pulses, and stops when the result lands.

<!-- TODO(screenshot): the settings window, Commands tab, with a couple of commands. -->

## Worth knowing

- **A hotkey sometimes does nothing, on purpose.** If you switched windows, a modifier key was
  still held, or the model's answer came back malformed, Clipboard AI aborts instead of pasting a
  guess over your text. The History tab tells you why.
- **Your keys and history are stored in plain text** under `%APPDATA%\com.clipboardai.app\`.
  History keeps the last 100 runs - input and output - so you can copy back a result you lost;
  clear it any time from the History tab. The log file never contains your text.

## Develop

```sh
npm run tauri dev
```

Tests: `cargo test` (from `src-tauri/`) and `npm run test`. The core loop - capture, send, paste,
and the safety guards around it - lives in `src-tauri/src/cycle.rs`; the settings UI is SvelteKit
under `src/`.

On Windows, `cargo` needs MSVC's `link.exe` on `PATH`. If a build can't find the linker, load
`vcvars64.bat` from your Visual Studio install in the same shell first.

Windows only for now.

## License

[MIT](LICENSE) - © 2026 Alexander Schnabl
