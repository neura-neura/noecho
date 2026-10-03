# Validación 0.2.6

## Cambio de ruta

Se mantiene el cable de micrófono de UnifiedAudio. La mezcla para Parsec se entrega por una salida interna independiente, Steam Streaming Speakers. El instalador configura host_audio_id con el identificador enumerado en Windows y host_audio_cancel=0, únicamente con Parsec cerrado. Se respaldan sus preferencias. No se cambian salidas de las apps ni los dispositivos predeterminados.

La configuración host_audio_id fue comprobada con el selector real de Parsec 150-105a: su valor corresponde al IMMDevice ID de la salida de Steam. Tras la comprobación se restauró Default. La instalación activa no se convirtió a la nueva ruta durante el desarrollo.

## Pruebas realizadas

- Entrega y captura de un tono por Steam Streaming Speakers: frames=96960, peak_energy=0.008332258.
- Prueba con procesos separados sonando por Ugreen y auriculares Realtek: el permitido llega a la salida interna y el privado conserva su reproducción local.
- Medición en la salida receptora: peak_energy=0.008340734 antes de excluir ambos tonos; peak_energy=0 después. El primer cambio de selección se aplicó sin desmarcar y volver a marcar.
- Desmarcar vuelve a incluir; reiniciar el proceso marcado mantiene su exclusión; el dispositivo predeterminado local se conservó; no hubo errores de captura en esa prueba.
- Prueba de configuración de Parsec: solo cambian fuente de audio y cancelación nativa; se conservan pantalla, app de eco anterior, metadatos y otras preferencias.
- Compilación de frontend y pruebas de audio.
- Repetición de la prueba de hardware con lectura explícita del cable de UnifiedAudio como micrófono: misma exclusión a cero y sin errores de captura.
- Regresión de modos: un micrófono virtual PC-only con pista de voz cero conserva su señal de salida; Voice-only no apaga los medidores de los programas capturados por separado.

Falta validar la recepción de una llamada real desde otra PC con el instalador nuevo. El método filtra contribuciones de programas; no promete retirar eco acústico recogido por un micrófono físico. La salida interna debe estar instalada y disponible. Desactivar conserva el transporte de audio; salir completamente de NoEcho lo detiene.
