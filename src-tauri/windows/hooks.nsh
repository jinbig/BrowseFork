; Run registration through the same tested Rust implementation used by Settings.
; Only this user's BrowseFork keys are written. Existing defaults stay untouched.
!macro NSIS_HOOK_POSTINSTALL
  ExecWait '"$INSTDIR\browsefork.exe" --register-browser' $0
  ${If} $0 != 0
    MessageBox MB_ICONSTOP "BrowseFork could not register with Windows Default Apps. Open BrowseFork Settings and try Set as default browser."
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ExecWait '"$INSTDIR\browsefork.exe" --unregister-browser' $0
!macroend
