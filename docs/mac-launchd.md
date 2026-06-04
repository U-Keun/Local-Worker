# Advanced macOS launchd Notes

This document is for advanced or legacy workflows.

For the current Tauri desktop app, use the in-app setting:

```text
Start Local Worker when this Mac logs in
```

That is the recommended v1 autostart path. It starts Local Worker for the
logged-in macOS user and keeps the menu bar app available for background issue
polling.

The notes below describe the earlier Bash runner approach. Keep them only if you
want to experiment with the legacy scripts directly.

## Legacy Bash Runner

For personal use on a spare MacBook, start manually first:

```bash
./scripts/agent-once.sh
```

After several successful manual runs, you can let macOS run the loop with
`launchd`.

## Example plist

Create `~/Library/LaunchAgents/com.local-dev-agent.example.plist` and adjust the
paths:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.local-dev-agent.example</string>

  <key>ProgramArguments</key>
  <array>
    <string>/bin/bash</string>
    <string>-lc</string>
    <string>cd /absolute/path/to/your/project && ./scripts/agent-loop.sh</string>
  </array>

  <key>RunAtLoad</key>
  <true/>

  <key>KeepAlive</key>
  <false/>

  <key>StandardOutPath</key>
  <string>/absolute/path/to/your/project/logs/launchd.out.log</string>

  <key>StandardErrorPath</key>
  <string>/absolute/path/to/your/project/logs/launchd.err.log</string>
</dict>
</plist>
```

Load it:

```bash
launchctl load ~/Library/LaunchAgents/com.local-dev-agent.example.plist
```

Unload it:

```bash
launchctl unload ~/Library/LaunchAgents/com.local-dev-agent.example.plist
```

## Recommended legacy workflow

1. Use the Tauri app autostart toggle unless you specifically need the Bash
   loop.
2. Run the Bash runner manually until the behavior is predictable.
3. Set `MAX_RUNS=1` and test launchd once.
4. Increase to a loop only after reviewing logs and permissions.

Like the desktop app, a user LaunchAgent runs only while that macOS user is
logged in. A helper that runs while logged out is outside the v1 scope.
