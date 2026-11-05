# ZipChat Packaging & Platform Deployment Guide

ZipChat is a self-contained, native Rust desktop chat client designed to run efficiently on modest hardware, including Linux ARM64 (Raspberry Pi OS 64-bit), Linux x86_64, macOS, and Windows.

---

## 1. System Requirements & Build Dependencies

ZipChat uses `eframe` / `egui` with hardware/software OpenGL rendering (`glow`) and pure-Rust TLS (`rustls`), avoiding hard dependencies on heavy browser engines or platform-specific webviews.

### Debian / Ubuntu / Raspberry Pi OS (ARM64 & x86_64)

Install the required development libraries for windowing and graphics:
```bash
sudo apt-get update && sudo apt-get install -y \
    build-essential \
    pkg-config \
    libfontconfig1-dev \
    libx11-dev \
    libx11-xcb-dev \
    libxkbcommon-dev \
    libxkbcommon-x11-dev \
    libwayland-dev
```

### macOS
- Xcode Command Line Tools: `xcode-select --install`
- No additional C libraries required.

### Windows
- Visual Studio C++ Build Tools or MinGW.
- No additional runtime dependencies required.

---

## 2. Building from Source

### Native Build (Host Machine)
```bash
# Debug build
cargo build

# Optimized release build
cargo build --release
```
The compiled standalone executable will be located at:
- `target/release/zipchat` (Linux / macOS)
- `target/release/zipchat.exe` (Windows)

---

## 3. Raspberry Pi OS (Linux ARM64 / aarch64)

ZipChat is built with first-class Raspberry Pi 4 / 5 (ARM64) support:

### Option A: Native Build on Raspberry Pi
Clone the repository on the Raspberry Pi and run:
```bash
cargo build --release
```

### Option B: Cross-Compilation from Linux x86_64
Using `cross`:
```bash
cargo install cross --git https://github.com/cross-rs/cross
cross build --target aarch64-unknown-linux-gnu --release
```
Verify the generated binary architecture:
```bash
file target/aarch64-unknown-linux-gnu/release/zipchat
# Output should indicate: ELF 64-bit LSB pie executable, ARM aarch64
```

---

## 4. Standalone Distribution Packaging

### Debian Package (.deb) for Raspberry Pi OS & Debian
Using `cargo-deb`:
```bash
cargo install cargo-deb
cargo deb --target aarch64-unknown-linux-gnu
```
This generates a `.deb` package installable via `dpkg -i <filename>.deb`.

### AppImage for Linux x86_64 & ARM64
1. Download `linuxdeploy` (and `linuxdeploy-plugin-appimage`).
2. Point to `target/release/zipchat` with an icon and desktop entry.
3. Bundle and run anywhere without installing system packages.

---

## 5. Persistent Storage & Offline Operation

ZipChat automatically persists settings and conversations to standard platform configuration directories:
- **Linux / Raspberry Pi OS**: `$XDG_CONFIG_HOME/zipchat/` or `~/.config/zipchat/`
- **macOS**: `~/Library/Application Support/zipchat/`
- **Windows**: `%APPDATA%\zipchat\`
- **Fallback**: `./.zipchat` in current working directory

Data is stored in two human-readable JSON files using safe atomic writes:
- `settings.json`: Gateway URL, active model, streaming toggle, and auto-scroll preferences.
- `conversations.json`: Complete conversation histories and message statuses.

ZipChat operates completely offline for reviewing, managing, and exporting previously saved conversations. Network access is only initiated when connecting to the configured Universal AI Gateway.
