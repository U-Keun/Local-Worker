# Running the Agent on macOS with launchd

For personal use on a spare MacBook, start manually first:

```bash
./scripts/agent-once.sh
```

After several successful manual runs, you can let macOS run the loop with `launchd`.

## Example plist

Create `~/Library/LaunchAgents/com.local-dev-agent.example.plist` and adjust the paths:

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

Recommended workflow:

1. Run manually until the behavior is predictable.
2. Set `MAX_RUNS=1` and test launchd once.
3. Increase to a loop only after reviewing logs and permissions.
