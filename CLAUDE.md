# Notes for Claude Code sessions in this repo

See `README.md` for what the app is, how to run it, and its architecture. This file is
only for build/environment gotchas specific to working in this repo that aren't
worth putting in the README because they're this-machine-specific, not portable advice.

## `cargo` needs the MSVC linker loaded in the same shell invocation

Windows builds need MSVC's `link.exe` on `PATH`. If a plain `cargo build`/`cargo test`
fails to find it, load `vcvars64.bat` for your Visual Studio install in the *same*
invocation. Verified working on this machine, from PowerShell:

```
cmd /c '"D:\Applications\Work\Visual Studio\18\VC\Auxiliary\Build\vcvars64.bat" >nul && cd /d E:\Code\clipboard-ai\src-tauri && cargo test'
```

Run this via the PowerShell tool, not the Bash/MSYS tool — MSYS re-quotes the path and
silently mangles the space in "Visual Studio", causing the vcvars call to no-op.

Keep any pipe *outside* the `cmd /c '...'` string (e.g. `| Select-Object -Last 60` after
the closing quote). A `| tail` inside it makes cmd re-spawn the chain through a pipeline,
which mangles the quoted path the same way ("vswhere.exe is not recognized").

## Do not run `npm run tauri dev` from an agent

It launches a long-running GUI app (tray icon, hidden window) and hangs the calling
session. If you need to see the app running, ask the user to run it, or use whatever
`run`-skill pattern the project has for launching and screenshotting it.
