!macro NSIS_HOOK_POSTUNINSTALL
  ; classic right-click menu entries (fallback when the compact menu is not available)
  DeleteRegKey HKCU "Software\Classes\*\shell\Bolt"
  DeleteRegKey HKCU "Software\Classes\Directory\shell\Bolt"
  ; compact Windows 11 menu: remove the sparse package Bolt registered
  nsExec::Exec 'powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "Get-AppxPackage -Name Bolt.ContextMenu | Remove-AppxPackage"'
!macroend
