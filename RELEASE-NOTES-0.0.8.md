# Formula 0.0.8: what's new since 0.0.7

## Multiple windows
- File ▸ Open and Open Recent open the file in a new window when the current window already holds a document. Only an empty, untouched window is reused.
- Unsaved documents are numbered across windows, Untitled_1, Untitled_2…, in the window title and as the suggested name when saving.
- Mac: a Window menu in the menu bar (Minimize, Zoom, Bring All to Front, then the open windows). The Dock icon's menu lists the open windows by name.

## Linux
- Formula is also available as a Flatpak, which installs on almost any Linux distribution.
- The desktop shows Formula's icon on its windows under Wayland too.
- Settings, recent files and recovery copies are kept in `$XDG_CONFIG_HOME/formula` when that is set (needed for the Flatpak), `~/.config/formula` otherwise.
