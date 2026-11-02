# Security

## Reporting a vulnerability

Use GitHub's private reporting: [Report a vulnerability](https://github.com/alexschnabl/clipboard-ai/security/advisories/new). Do not open a public issue for it.

## What the app does with your data

- The selected text, and its HTML when the selection is formatted, goes to the backend you configured and nowhere else. Inline images never leave the machine.
- API keys are stored in plain text in `%APPDATA%\com.clipboardai.app\config.json`. This is a known limitation, not a bug. Use a local backend if you want no keys on disk.
- The run history (last 100 runs with input and output) is stored in the same folder and can be cleared from the History tab.
- The app has no telemetry and no update check.

## Supported versions

Only the latest release gets fixes.
