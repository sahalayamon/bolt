!macro NSIS_HOOK_POSTUNINSTALL
  DeleteRegKey HKCU "SoftwareClasses*shellBolt"
  DeleteRegKey HKCU "SoftwareClassesDirectoryshellBolt"
  nsExec::Exec 'powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "Get-AppxPackage -Name Bolt.ContextMenu | Remove-AppxPackage"'
!macroend
