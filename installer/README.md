# Instalador de NoEcho 0.2.6

Ejecuta `npm run installer` para generar el instalador normal y `dist-installer/NoEcho_0.2.6_un-solo-cable_setup.exe`. El script `NoEcho-Control.ps1` se copia junto al instalador.

Para Parsec, usa el paquete combinado. Cierra Parsec, UnifiedAudio y NoEcho desde sus bandejas antes de instalar. Se conserva CABLE Input como micrófono y se configura Parsec para recibir la mezcla de NoEcho por Steam Streaming Speakers, ya instalado. El paquete no instala Steam ni necesita Cable A/B. Si falta la salida interna o Parsec sigue abierto, la preparación informa el fallo.

Sin UnifiedAudio, el instalador normal mantiene el modo independiente. Instalar el controlador virtual puede requerir reiniciar Windows.

El paquete combinado ejecuta `noecho.exe --prepare-parsec` antes de finalizar. Esa función solo cambia host_audio_id y host_audio_cancel, con copia de seguridad. Las apps y Windows mantienen sus salidas. NoEcho debe permanecer ejecutándose para entregar la mezcla, incluso cuando las exclusiones están desactivadas. Sigue [el README](../README.md) para pruebas y API.
