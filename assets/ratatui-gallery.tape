Output assets/ratatui-gallery.gif
Require target/debug/examples/ratatui_gallery
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
Env VISUAL "perl -pi -e s/^-/-_edited/"
Env PS1 "$ "
Env TERM "xterm-256color"
Env CLICOLOR_FORCE "1"

Hide
Type "export PS1='$ '; clear" Enter
Sleep 500ms
Show

Type "target/debug/examples/ratatui_gallery" Enter
Wait+Screen /your name/
Sleep 1s
Enter
Sleep 800ms
Type "Ferris" Enter
Sleep 800ms
Tab
Type "hunter2"
Sleep 600ms
Tab
Type "ru"
Sleep 600ms
Down
Sleep 400ms
Tab
Sleep 600ms
Tab
Sleep 600ms
Right
Sleep 600ms
Type "/30"
Sleep 800ms
Enter
Sleep 800ms
Tab
Space
Down 2
Space
Sleep 800ms
Tab
Right
Down 2
Right
Sleep 800ms
Tab
Sleep 800ms
Tab
Sleep 800ms
Tab
Right
Sleep 800ms
Tab
Sleep 600ms
Type "e"
Sleep 1200ms
Tab
Sleep 1500ms
Tab
Type "demo" Enter
Sleep 800ms
Enter
Sleep 800ms
Enter
Sleep 1500ms
Ctrl+C
Sleep 500ms
