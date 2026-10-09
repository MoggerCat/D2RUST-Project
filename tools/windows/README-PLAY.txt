d2rs playtest build (Windows)
=============================

Build of commit @COMMIT@ (branch @REF@).
A private development build of a Diablo II: Lord of Destruction
reimplementation. It contains no game files: it reads your own
Diablo II 1.14d install.

Start
-----
1. Copy d2-client.exe (and d2_client.pdb) into your Diablo II folder,
   the one holding Game.exe and d2data.mpq.
2. Double-click d2-client.exe. The main menu opens.

Or keep it anywhere and point it at the game:
   d2-client.exe --game-dir "C:\Program Files (x86)\Diablo II"
(or set the D2_GAME_DIR environment variable to that folder).

Shortcuts straight into a game (from a command prompt):
   d2-client.exe --new sorceress MyName     new character
   d2-client.exe --save "path\to\MyName.d2s" load a save
   d2-client.exe --difficulty nightmare ...  normal / nightmare / hell
   d2-client.exe --res 640x480               the 640 x 480 frame

Characters are saved in Documents\d2rs\saves (never inside the game
folder). Key bindings: %APPDATA%\d2rs\controls.toml (the original
game's default keys).

When it crashes
---------------
It writes d2rs-crash.log next to d2-client.exe (backtrace and what it
was doing). Send that file, a screenshot, and the save
(Documents\d2rs\saves\<name>.d2s). Full guide: docs/PLAYTEST.md in the
repository.
