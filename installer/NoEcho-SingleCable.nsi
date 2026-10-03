Unicode true
RequestExecutionLevel user
SetCompressor /SOLID lzma
!include "LogicLib.nsh"
!include "FileFunc.nsh"
Name "NoEcho 0.2.6 — un solo cable"
OutFile "..\dist-installer\NoEcho_0.2.6_un-solo-cable_setup.exe"
ShowInstDetails show
Page instfiles
Section
  InitPluginsDir
  SetOutPath "$PLUGINSDIR"
  File /oname=NoEcho-setup.exe "..\target\release\bundle\nsis\NoEcho_0.2.6_x64-setup.exe"
  File /oname=engine.exe "integration\UnifiedAudio Engine Host.exe"
  File /oname=NoEcho-route.exe "..\target\release\noecho.exe"
  File "Update-UnifiedAudioEngine.ps1"
  IfFileExists "$LOCALAPPDATA\Programs\UnifiedAudio\UnifiedAudio.exe" 0 no_unified
  ExecWait 'powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$PLUGINSDIR\Update-UnifiedAudioEngine.ps1" -Source "$PLUGINSDIR\engine.exe"' $0
  ${If} $0 != 0
    MessageBox MB_ICONSTOP "No se actualizó UnifiedAudio. Cierra UnifiedAudio desde su bandeja y vuelve a ejecutar este instalador. No se han cambiado tus ajustes."
    Abort
  ${EndIf}
  no_unified:
  ExecWait '"$PLUGINSDIR\NoEcho-route.exe" --prepare-parsec' $0
  ${If} $0 != 0
    MessageBox MB_ICONSTOP "No se pudo preparar Parsec. Cierra Parsec desde su bandeja y vuelve a instalar. Si ya está cerrado, revisa %LOCALAPPDATA%\NoEcho\prepare-error.txt. Se conserva tu micrófono."
    Abort
  ${EndIf}
  ${GetParameters} $R0
  ClearErrors
  ${GetOptions} $R0 "/S" $R1
  ${If} ${Errors}
    ExecWait '"$PLUGINSDIR\NoEcho-setup.exe"' $0
  ${Else}
    ExecWait '"$PLUGINSDIR\NoEcho-setup.exe" /S' $0
  ${EndIf}
  ${If} $0 != 0
    MessageBox MB_ICONSTOP "NoEcho no terminó de instalarse."
    Abort
  ${EndIf}
SectionEnd
