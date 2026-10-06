Unicode true
RequestExecutionLevel user
SetCompressor /SOLID lzma
!include "LogicLib.nsh"
!include "FileFunc.nsh"
!include "nsDialogs.nsh"
Var CloseApps
Var CloseAppsCheckbox
Function .onInit
  StrCpy $CloseApps ${BST_CHECKED}
FunctionEnd
Function OptionsPage
  nsDialogs::Create 1018
  Pop $0
  ${NSD_CreateLabel} 0 0 100% 32u "Elige cómo preparar el audio para NoEcho."
  Pop $0
  ${NSD_CreateCheckbox} 0 40u 100% 24u "Cerrar UnifiedAudio y Parsec durante la instalación"
  Pop $CloseAppsCheckbox
  ${NSD_SetState} $CloseAppsCheckbox $CloseApps
  ${NSD_CreateLabel} 0 74u 100% 56u "Al cerrarlos, se interrumpirá el audio y cualquier conexión de Parsec. Puedes abrirlos de nuevo al terminar.$\r$\n$\r$\nSi desmarcas la casilla, los cambios que estén en uso se aplicarán más adelante."
  Pop $0
  nsDialogs::Show
FunctionEnd
Function OptionsLeave
  ${NSD_GetState} $CloseAppsCheckbox $CloseApps
FunctionEnd
Name "NoEcho 0.2.9 — un solo cable"
OutFile "..\dist-installer\NoEcho_0.2.9_un-solo-cable_setup.exe"
ShowInstDetails show
Page custom OptionsPage OptionsLeave
Page instfiles
Section
  InitPluginsDir
  SetOutPath "$PLUGINSDIR"
  File /oname=NoEcho-setup.exe "..\target\release\bundle\nsis\NoEcho_0.2.9_x64-setup.exe"
  File /oname=engine.exe "integration\UnifiedAudio Engine Host.exe"
  File /oname=NoEcho-route.exe "..\target\release\noecho.exe"
  File "Update-UnifiedAudioEngine.ps1"
  File "Close-AudioApps.ps1"
  ${If} $CloseApps == ${BST_CHECKED}
    DetailPrint "Cerrando UnifiedAudio y Parsec..."
    ExecWait 'powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\Close-AudioApps.ps1"' $0
    ${If} $0 != 0
      MessageBox MB_ICONSTOP "No se pudieron cerrar las aplicaciones de audio. Consulta %LOCALAPPDATA%\NoEcho\close-apps-error.txt."
      Abort
    ${EndIf}
  ${EndIf}
  IfFileExists "$LOCALAPPDATA\Programs\UnifiedAudio\UnifiedAudio.exe" 0 no_unified
  ExecWait 'powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$PLUGINSDIR\Update-UnifiedAudioEngine.ps1" -Source "$PLUGINSDIR\engine.exe"' $0
  ${If} $0 != 0
    MessageBox MB_ICONSTOP "No se pudo preparar la integracion de UnifiedAudio. Consulta %LOCALAPPDATA%\NoEcho\integration-update-error.txt."
    Abort
  ${EndIf}
  no_unified:
  ExecWait '"$PLUGINSDIR\NoEcho-route.exe" --prepare-parsec' $0
  ${If} $0 != 0
    MessageBox MB_ICONSTOP "No se pudo preparar Parsec. Consulta %LOCALAPPDATA%\NoEcho\prepare-error.txt."
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
