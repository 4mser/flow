# FLOW — Multiplayer Operating System

## What is this?

FLOW connects multiple PCs on the same network into a shared workspace. Copy on one machine, paste on the other. Send files between machines. Works across macOS, Windows, and Linux.

## Architecture

```
flow-protocol/   — Shared types: peers, monitors, messages, file transfer
flow-core/       — mDNS discovery, TCP mesh, clipboard sync, file transfer, monitor detection
flow-agent/      — CLI binary (run on each machine)
flow-ui/         — Tauri desktop app with GUI
```

## Setup on a new machine

### 1. Install Rust

**Windows (PowerShell):**
```powershell
winget install Rustlang.Rustup
# Close and reopen PowerShell after install
rustup update
```

**macOS/Linux:**
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

Verify: `rustc --version` (needs 1.85+)

### 2. Clone and build

```bash
git clone https://github.com/4mser/flow.git
cd flow
cargo build --release
```

The CLI binary is at `target/release/flow-agent` (or `flow-agent.exe` on Windows).
The GUI app is at `target/release/flow-ui` (or `flow-ui.exe`).

To build only the CLI: `cargo build --release -p flow-agent`

### 3. Windows firewall (REQUIRED on Windows)

Run in PowerShell **as Administrator**:
```powershell
netsh advfirewall firewall add rule name="FLOW-TCP-IN" dir=in action=allow protocol=TCP localport=19847
netsh advfirewall firewall add rule name="FLOW-TCP-OUT" dir=out action=allow protocol=TCP localport=19847
netsh advfirewall firewall add rule name="FLOW-mDNS" dir=in action=allow protocol=UDP localport=5353
```

Verify: `netsh advfirewall firewall show rule name="FLOW-TCP-IN"`

### 4. Find your IP

**macOS:** `ipconfig getifaddr en0`
**Windows:** `(Get-NetIPAddress -AddressFamily IPv4 | Where-Object { $_.InterfaceAlias -match "Wi-Fi|Ethernet" }).IPAddress`

### 5. Run

**Both machines must be on the same network.**

mDNS auto-discovery may not work between macOS and Windows. Use `--peer` for direct connection:

```bash
# Machine A (e.g. Mac, IP = 192.168.1.10):
./target/release/flow-agent --name "MacBook" --color 0

# Machine B (e.g. Windows, IP = 192.168.1.20), connect to Machine A:
.\target\release\flow-agent.exe --name "Razer" --color 1 --peer 192.168.1.10
```

Either side can use `--peer`. Only one side needs it — the other accepts the incoming connection.

### 6. Test clipboard

1. Copy text on Machine A (Ctrl+C / Cmd+C)
2. Wait 1 second
3. Paste on Machine B (Ctrl+V)
4. The text should appear

### 7. Send files

```bash
# Send a file to all connected peers:
./target/release/flow-agent --name "MacBook" --color 0 --peer 192.168.1.20 --send /path/to/file.pdf
```

Files are saved to `~/Downloads/FLOW/` on the receiving machine.

## Available colors

| --color | Color  |
|---------|--------|
| 0       | Red    |
| 1       | Blue   |
| 2       | Green  |
| 3       | Purple |
| 4       | Orange |
| 5       | Pink   |

## For Claude on a new machine

If you are Claude Code and the user wants to set up FLOW:

1. Check Rust: `rustc --version`. Needs 1.85+. If missing: install rustup.
2. `git clone https://github.com/4mser/flow.git && cd flow && cargo build --release`
3. On Windows: run the firewall commands above in admin PowerShell.
4. Find the local IP. Find the other machine's IP.
5. Run: `.\target\release\flow-agent.exe --name "<name>" --color 1 --peer <other-machine-ip>`
6. Test: copy text on this machine, check if the other machine's console shows `[clipboard] ←`

### Troubleshooting

- **"Connection refused"**: Check firewall rules on BOTH machines. On macOS, allow the app when prompted.
- **Clipboard not syncing**: Ensure both agents show "Connected:" in their logs. On macOS the app may need Accessibility permissions (System Settings → Privacy & Security → Accessibility).
- **Test network**: From Windows: `Test-NetConnection -ComputerName <mac-ip> -Port 19847`
- **Verbose logs**: `RUST_LOG=flow=debug ./target/release/flow-agent ...`
- **Port in use**: Kill previous instance: `lsof -ti:19847 | xargs kill` (macOS) or `netstat -ano | findstr 19847` then `taskkill /PID <pid> /F` (Windows)
