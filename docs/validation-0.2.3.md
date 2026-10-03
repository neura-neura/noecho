# Validación de 0.2.3

Verificación local del 2 de octubre de 2026.

- TypeScript/Vite compilan. La ventana inicial muestra unas cinco filas completas con letra de 14–15 px y casillas de 18 px, sin maximizar.
- Pruebas del algoritmo: referencia estéreo con 50 ms de retraso y cambio de ganancia de 0,75 a 0,4. Reducción de energía de 60,7 dB después de adaptación. Con otra señal simultánea, la energía de salida fue 0,799 de la señal independiente. Son señales sintéticas; no garantizan estos resultados con todos los efectos o programas.
- Arranque con UnifiedAudio abierto: correcto. Se capturó su salida CABLE Input y se eligió CABLE-A Input para entregar el resultado. Sin avisos del mezclador. Se comprobaron medidores y comandos include/exclude/stop.
- Los hashes SHA256 de config.xml y settings.json de UnifiedAudio fueron idénticos antes y después. No se cambiaron sus efectos, dispositivos ni configuración.
- El estado guardado local de UnifiedAudio tenía modo voz; esta prueba no confirma su mezcla real de voz y programas ni la recepción desde otra PC. La prueba sintética sí incluye una mezcla con dos fuentes.
- No se publicó la versión. El instalador NSIS queda en dist-installer.

Pendiente al probar: recibir el canal final de NoEcho en la PC remota, cancelación con el procesamiento y la latencia reales, calidad de voz durante conversación simultánea y audio residual durante adaptación/cambios de ganancia.
