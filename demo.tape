# howdyctl demo recording.
# Regenerate with:  cargo build --release && vhs demo.tape
Output docs/demo.gif

Set Shell bash
Set FontSize 18
Set Width 1000
Set Height 640
Set Padding 24
Set TypingSpeed 60ms

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
