# Motor UnifiedAudio compatible

La integración modifica UnifiedAudio 0.1.6. `unified-audio-noecho.patch` contiene los cambios de archivos existentes; copia `NoEchoMix.h` a `src/UnifiedAudio.EngineHost/src/audio/` después de aplicar el parche.

Compila con CMake/MSVC según el README de UnifiedAudio. Copia el motor Release a esta carpeta antes de ejecutar el generador de instaladores de NoEcho. Este binario no abre una salida adicional: la etapa se ejecuta en el callback de la salida existente.

`UnifiedAudio.NoEcho.v1` es un pipe local independiente del de la interfaz, con permisos para el propietario y el sistema. Acepta `noecho.configure` (enabled/apps) y `noecho.status`. El filtro no se guarda en los perfiles. NoEcho mantiene una señal de control; tras perderla se restaura el sonido original.

El instalador combinado actualiza únicamente instalaciones estándar de UnifiedAudio 0.1.6 cerradas. Guarda el motor anterior y no instala UnifiedAudio donde no existe. Para revertir el motor, cierra ambos programas y restaura `UnifiedAudio Engine Host.exe.before-noecho` sobre el ejecutable original.
