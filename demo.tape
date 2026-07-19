# Demo tape for recall-ai — recorded by charmbracelet/vhs
# https://github.com/charmbracelet/vhs
# Run: vhs demo.tape  (renders to demo.gif)

Output demo.gif

Set Shell bash
Set FontSize 14
Set Width 1200
Set Height 720
Set Padding 20
Set Theme "Catppuccin Mocha"

# --- Scene 1: import 6826 messages ---
Type "recall import ~/.claude/projects/"
Sleep 500ms
Enter
Sleep 4s

# --- Scene 2: search shows real content ---
Type "recall search ctxguard --days 365"
Sleep 500ms
Enter
Sleep 3s

# --- Scene 3: today digest ---
Type "recall today"
Sleep 500ms
Enter
Sleep 3s

# --- Scene 4: serve localhost web UI ---
Type "recall serve &"
Sleep 500ms
Enter
Sleep 2s
Type "curl -s 'http://127.0.0.1:7777/search?q=ctxguard' | head -50"
Sleep 500ms
Enter
Sleep 4s

# --- Scene 5: stats + closing ---
Type "recall stats"
Sleep 500ms
Enter
Sleep 3s