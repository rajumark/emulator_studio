# Emulator Studio

A tiny cross-platform desktop app for starting and stopping the Android
emulators already installed on your machine. Roughly 3 MB, no Android Studio
required at runtime.

Emulators are launched detached, so they keep running after you quit the app.

## Install

Grab the file for your platform from the
[latest release](../../releases/latest):

| Platform | File |
| --- | --- |
| macOS (Apple Silicon) | `*_aarch64.dmg` |
| macOS (Intel) | `*_x64.dmg` |
| Windows | `*_x64-setup.exe` |
| Linux | `*_amd64.AppImage` or `*_amd64.deb` |

macOS builds are unsigned — right-click the app and choose **Open** the first
time.

## How it finds your SDK

In order: a path saved in the app, `ANDROID_HOME`, `ANDROID_SDK_ROOT`, then the
default location for your OS. If none works, set it under **⋮ → SDK path**.

## Development

```bash
npm install
npm run dev      # hot-reloading dev build
npm run build    # release bundles for the current platform
```

## License

MIT
