!macro NSIS_HOOK_POSTUNINSTALL
  DeleteRegKey HKCU "Software\Classes\*\shell\Bolt"
  DeleteRegKey HKCU "Software\Classes\Directory\shell\Bolt"
!macroend
