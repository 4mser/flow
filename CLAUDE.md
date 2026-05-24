# FLOW — Multiplayer Operating System

## What is this?

FLOW is a cross-platform system that connects multiple PCs on the same network into a shared workspace. Right now it supports **shared clipboard** between machines (macOS, Windows, Linux). Peers discover each other automatically via mDNS — no configuration needed.

## Architecture

```
flow-protocol/   — Shared types: peers, monitors, spatial layout, messages
flow-core/       — Core logic: mDNS discovery, TCP mesh networking, clipboard sync, monitor detection
flow-agent/      — CLI binary that runs on each machine
```

- **Rust workspace** — all crates compile on macOS, Windows, and Linux
- **mDNS** (`_flow._tcp.local.`) for automatic peer discovery on LAN
- **TCP** (port 19847) for reliable message exchange between peers
- Clipboard is monitored every 500ms; changes are broadcast to all connected peers

## How to set up on a new machine

### Prerequisites

1. **Install Rust** (if not installed):
   - macOS/Linux: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
   - Windows: `winget install Rustlang.Rustup` or download from https://rustup.rs
2. **Install Git** (if not installed):
   - Windows: `winget install Git.Git`

### Build

```bash
git clone https://github.com/4mser/flow.git
cd flow
cargo build --release
```

### Firewall (Windows only — REQUIRED)

Run in PowerShell as Administrator:

```powershell
netsh advfirewall firewall add rule name="FLOW-TCP" dir=in action=allow protocol=TCP localport=19847
netsh advfirewall firewall add rule name="FLOW-mDNS" dir=in action=allow protocol=UDP localport=5353
```

### macOS firewall

If macOS prompts "Do you want the application flow-agent to accept incoming network connections?", click **Allow**.

### Run

```bash
# On macOS:
./target/release/flow-agent --name "MacBook" --color 0

# On Windows:
.\target\release\flow-agent.exe --name "Razer" --color 1
```

Both machines must be on the same local network. They will discover each other automatically. Copy text on one machine → paste on the other.

### Available colors

| Flag | Color  |
|------|--------|
| 0    | Red    |
| 1    | Blue   |
| 2    | Green  |
| 3    | Purple |
| 4    | Orange |
| 5    | Pink   |

## For Claude on a new machine

If you are Claude Code running on a Windows/Linux machine and the user wants to set up FLOW:

1. Check if Rust is installed (`rustc --version`). If not, guide the user to install it.
2. Clone this repo and run `cargo build --release`.
3. On Windows: configure the firewall rules above (needs admin PowerShell).
4. Run the agent with a unique `--name` and `--color` that differs from other peers.
5. Verify the agent starts, detects monitors, and shows "Searching for peers on the network..."
6. When the other machine's agent is also running, you should see "Peer joined" and "Connected and syncing" messages.
7. Test by copying text — it should appear on both machines' clipboards.

### Troubleshooting

- **Peers not finding each other**: Ensure both machines are on the same WiFi/LAN. Check that mDNS (UDP 5353) is not blocked.
- **Connection refused**: Check that TCP port 19847 is open on both machines' firewalls.
- **Clipboard not syncing**: On macOS, the app may need Accessibility permissions. On Windows, run as administrator if clipboard access fails.
