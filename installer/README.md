# Instalador de NoEcho 0.2.10

Ejecuta `npm run installer` para generar el instalador normal y `dist-installer/NoEcho_0.2.10_un-solo-cable_setup.exe`. El script `NoEcho-Control.ps1` se copia junto al instalador.

Para Parsec, usa el paquete combinado. La casilla para cerrar UnifiedAudio y Parsec está marcada por defecto. Puedes desmarcarla para dejarlos abiertos. Si ya estan preparados, se conservan; un cambio necesario que esté en uso queda pendiente y se aplica automáticamente después. Se conserva CABLE Input como micrófono y se configura Parsec para recibir la mezcla de NoEcho por Steam Streaming Speakers, ya instalado. El paquete no instala Steam ni necesita Cable A/B. Si falta la salida interna, la preparación informa el requisito. Tener Parsec abierto no bloquea la instalación.

Sin UnifiedAudio, el instalador normal mantiene el modo independiente. Instalar el controlador virtual puede requerir reiniciar Windows.

El paquete combinado ejecuta `noecho.exe --prepare-parsec` antes de finalizar. Esa función solo cambia host_audio_id y host_audio_cancel, con copia de seguridad. Las apps y Windows mantienen sus salidas. NoEcho debe permanecer ejecutándose para entregar la mezcla, incluso cuando las exclusiones están desactivadas. Sigue [el README](../README.md) para pruebas y API.

El instalador configura el firewall para recibir comandos TCP 47832 desde la misma subred. Windows pide permiso de administrador una vez durante la instalación. NoEcho 0.2.10 no requiere clave ni activación manual del control por red. Consulta la sección API del README para enviar start/stop por IP.
