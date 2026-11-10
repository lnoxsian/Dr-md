# Debian Packaging Guide

This guide describes how to package **dr.md** (Doctor Markdown) as a Debian `.deb` package. This allows for native installation on Debian, Ubuntu, Linux Mint, Pop!_OS, and other Debian-based Linux distributions, including the setup of application icons, desktop menus, and file MIME-type associations.

## Prerequisites

To package the application, you must have the standard Debian package builder utility (`dpkg-deb`) installed on your system. This is pre-installed on Debian/Ubuntu systems by default.

Ensure you also have the build requirements for Rust and local graphic/audio development libraries as listed in the [Build Guide](BUILD.md).

## Packaging

The repository provides an automated workflow in the `Makefile` to package `dr.md`.

### Using Make

Run the following command to build the optimized binary and compile the `.deb` package:

```bash
make package-deb
```

The command will output the completed package in `target/debian/`:

```
target/debian/dr-md_<VERSION>_<ARCHITECTURE>.deb
```
For example: `target/debian/dr-md_0.2.1_amd64.deb`.

---

## Installation

Install the generated `.deb` package using `apt` or `dpkg`:

### Installing via apt (Recommended, handles dependency resolving)

```bash
sudo apt install ./target/debian/dr-md_0.2.1_amd64.deb
```

### Installing via dpkg

```bash
sudo dpkg -i target/debian/dr-md_0.2.1_amd64.deb
sudo apt install -f # Fix any missing dependencies if needed
```

---

## Package Contents

The packaging script (`scripts/package_deb.sh`) builds a package structure containing:

| Path | Description |
|---|---|
| `/usr/bin/dr-md` | The compiled optimized binary. |
| `/usr/share/applications/dr-md.desktop` | Application launcher configuration (for app menus, docks, and search). |
| `/usr/share/icons/hicolor/256x256/apps/dr-md.png` | Native high-resolution application icon. |
| `DEBIAN/control` | Control metadata (version, architecture, dependencies, description). |

### Desktop Entry Features
The installed desktop launcher (`dr-md.desktop`) integrates with your desktop environment, enabling:
- Launching from search, application launchers, or favorites.
- Markdown file association (supports opening `.md` files directly).
- Grouping inside "Utility", "TextEditor", and "Development" categories.
