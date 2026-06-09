# howdyctl demo recording.
# Regenerate with:  cargo build --release && vhs demo.tape
Output docs/demo.gif

Set Shell bash
Set FontSize 18
Set Width 1000
Set Height 800
Set Padding 24
Set TypingSpeed 60ms
# Everforest Dark (medium) terminal theme — the UI itself is transparent
Set Theme { "background": "#2d353b", "foreground": "#d3c6aa", "cursor": "#d3c6aa", "selection": "#3d484d", "black": "#343f44", "red": "#e67e80", "green": "#a7c080", "yellow": "#dbbc7f", "blue": "#7fbbb3", "magenta": "#d699b6", "cyan": "#83c092", "white": "#d3c6aa", "brightBlack": "#4f585e", "brightRed": "#e67e80", "brightGreen": "#a7c080", "brightYellow": "#dbbc7f", "brightBlue": "#7fbbb3", "brightMagenta": "#d699b6", "brightCyan": "#83c092", "brightWhite": "#d3c6aa" }

Hide
Type "./target/release/howdyctl --demo"
Enter
Sleep 1.2s
Show

# Menu focused — arrow down through the sections, content updates live
Sleep 1.2s
Down
Sleep 1.4s
Down
Sleep 1.6s
Down
Sleep 1.8s
Up
Up
Up
Sleep 1s

# enter the content pane (Cameras), move the selection
Right
Sleep 800ms
Down
Sleep 900ms
Down
Sleep 900ms

# back to the menu, over to Models, enroll with an inline label
Tab
Sleep 700ms
Down
Sleep 900ms
Right
Sleep 700ms
Type "a"
Sleep 600ms
Type "Work laptop"
Sleep 1.3s
Escape
Sleep 700ms

Type "q"
Sleep 600ms
