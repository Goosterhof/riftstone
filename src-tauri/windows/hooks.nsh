; The Riftstone's installer hooks (tauri.conf.json:
; bundle > windows > nsis > installerHooks). Tauri's NSIS template includes
; this file after its own utilities and runs each hook it defines.
;
; NSIS_HOOK_PREUNINSTALL runs before the uninstaller removes a file, while
; riftstone.exe is still in place. It asks the stone to give the nxm:// links
; back (`riftstone.exe --give-back-nxm`, src-tauri/src/nxm.rs
; give_back_on_uninstall), as turning its switch off would: when Windows
; still sends the links to this riftstone.exe, the user's key goes back as
; the stone found it, so the program it took them from answers again, or
; Windows' own handler. A key another program took back is left alone. Where
; the stone cannot put the key back (its memory of it cannot be read), it
; removes the key whole: the exe that key starts is about to go. The
; uninstaller goes on whatever the answer.
;
; An update run by an updater (/UPDATE) puts the same riftstone.exe back in
; the same place, so it keeps the links. An upgrade that uninstalls the old
; version first (the installer's default choice) gives them back like any
; uninstall: the player turns the switch on again after it.
!macro NSIS_HOOK_PREUNINSTALL
  ${If} $UpdateMode <> 1
    ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --give-back-nxm'
    ClearErrors
  ${EndIf}
!macroend
