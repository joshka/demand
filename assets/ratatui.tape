Output assets/ratatui.gif
Require target/debug/examples/ratatui
# assets/vhs/common.tape, but taller.
Set Shell "bash"
Set Theme "Catppuccin Mocha"
Set FontFamily "monospace"
Set FontSize 18
Set Width 1000
Set Height 720
Set Padding 24
Set TypingSpeed 45ms
Set Framerate 20
Set CursorBlink false
Env PS1 "$ "
Env TERM "xterm-256color"
Env CLICOLOR_FORCE "1"

Hide
Type "export PS1='$ '; clear" Enter
Sleep 500ms
Show

Type "target/debug/examples/ratatui" Enter
Wait+Screen /Who's it for/
Sleep 1s
Enter
Sleep 1s
Type "Ferris" Enter
Sleep 600ms
Down
Sleep 600ms
Enter
Sleep 600ms
Type "/pe"
Sleep 800ms
Space
Sleep 600ms
Escape
Sleep 300ms
Down
Space
Sleep 1s
Enter
Sleep 800ms
Enter
Sleep 2s
