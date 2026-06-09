# howdyctl demo recording.
# Regenerate with:  cargo build --release && vhs demo.tape
Output docs/demo.gif

Set Shell bash
Set FontSize 18
Set Width 1000
Set Height 640
Set Padding 24
Set TypingSpeed 60ms
# Everforest Dark (medium) terminal theme — the UI itself is transparent
Set Theme { "background": "#2d353b", "foreground": "#d3c6aa", "cursor": "#d3c6aa", "selection": "#3d484d", "black": "#343f44", "red": "#e67e80", "green": "#a7c080", "yellow": "#dbbc7f", "blue": "#7fbbb3", "magenta": "#d699b6", "cyan": "#83c092", "white": "#d3c6aa", "brightBlack": "#4f585e", "brightRed": "#e67e80", "brightGreen": "#a7c080", "brightYellow": "#dbbc7f", "brightBlue": "#7fbbb3", "brightMagenta": "#d699b6", "brightCyan": "#83c092", "brightWhite": "#d3c6aa" }

Hide
Type "./target/release/howdyctl --demo"
Enter
Sleep 1.2s
Show

# Cameras tab — IR vs RGB, the active one marked
Sleep 1s
Down
Sleep 1.2s

# Models tab — enroll with an inline label
Tab
Sleep 1.2s
Type "a"
Sleep 600ms
Type "Work laptop"
Sleep 1.3s
Escape
Sleep 700ms

# Test tab — the live match-distance gauge
Tab
Sleep 1.6s
Type "+++"
Sleep 1s
Type "---"
Sleep 1.2s

# Doctor tab — health checks
Tab
Sleep 2.2s

# back around to Cameras
Tab
Sleep 1s
Type "q"
Sleep 600ms
