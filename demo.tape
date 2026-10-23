# VHS tape definition for generating demo GIF
# Run: vhs demo.tape

Output demo.gif

Set Shell "bash"
Set FontSize 15
Set Width 1200
Set Height 700
Set Padding 15
Set Theme "Catppuccin Mocha"

# Launch Sub-1K Radio
Type "sub1k"
Sleep 500ms
Enter

# Wait for TUI to load tracks and active playback
Sleep 3s

# Navigate the underdog queue
Down 2
Sleep 1s
Down 2
Sleep 1s

# Toggle Smart Boost Mode
Type "b"
Sleep 2s

# Rescue current track (Save to Liked Songs)
Type "s"
Sleep 2s

# Skip to next underdog track
Type "n"
Sleep 3s

# Exit cleanly
Type "q"
Sleep 1s
