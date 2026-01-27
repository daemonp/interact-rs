# interact-rs

Interact with nearest object keybind for World of Warcraft 1.12.1.5875 - Rust port

A Rust port of the [Interact](https://github.com/example/Interact) DLL that provides an "interact with nearest object" keybind functionality for the WoW 1.12.1 Vanilla client.

## Features

- Interact with the nearest valid object within 5 yards using a single keybind
- Smart priority system for object selection
- Auto-loot support
- Backwards compatible with the C version

## Priority Order

When multiple objects are in range, the nearest object is selected using this priority:

1. **Lootable corpses** - Dead units with loot available
2. **Game objects** - Chests, herbs, mining nodes, etc.
3. **Skinnable corpses** - Dead units that can be skinned (but not looted)
4. **Alive NPCs** - Living units for interaction

## Installation

1. Download `interact.dll` from the [Releases](https://github.com/example/interact-rs/releases) page
2. Place the DLL in your WoW game folder
3. Add `interact.dll` to `dlls.txt` (for VanillaFixes loader)
4. Launch the game

## Keybinding Setup

After loading, go to **Key Bindings** in the game menu. Two new bindings will be available:

- **Interact** - Standard interaction
- **Interact (auto-loot)** - Interaction with auto-loot enabled

## Lua API

### InteractNearest(autoloot)

Finds and interacts with the nearest valid object within 5 yards.

**Parameters:**
- `autoloot` (number) - `0` for normal interact, non-zero for auto-loot

**Returns:**
- `1` if interaction occurred, `0` otherwise

**Example:**
```lua
-- Normal interact
InteractNearest(0)

-- Interact with auto-loot
InteractNearest(1)
```

## Building from Source

### Prerequisites

- Rust nightly toolchain
- MinGW-w64 (for cross-compilation from Linux)

### Install Dependencies

```bash
# Install Rust target
rustup target add i686-pc-windows-gnu

# On Ubuntu/Debian
sudo apt install mingw-w64

# On Arch Linux
sudo pacman -S mingw-w64-gcc
```

### Build

```bash
# Development build
cargo build

# Release build
cargo build --release
```

The DLL will be at `target/i686-pc-windows-gnu/release/interact.dll`

## Blacklisted Objects

Some game objects are blacklisted to prevent issues:
- 179830, 179831, 179785, 179786

## Debug Logging

Debug logs are written to `Logs\interact_debug.log` in the WoW directory.

## License

BSD-2-Clause
