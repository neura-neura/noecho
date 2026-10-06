!macro NSIS_HOOK_POSTINSTALL
  ExecWait '"$INSTDIR\noecho.exe" --setup-network' $0
  ${If} $0 != 0
    MessageBox MB_ICONEXCLAMATION "Windows no permitio configurar el control de NoEcho por la red. Vuelve a instalar y acepta el permiso de administrador."
  ${EndIf}
!macroend
