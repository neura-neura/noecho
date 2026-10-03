# Validación de NoEcho 0.2.0

Verificación local del 2 de octubre de 2026.

- `npm run build`: compilación TypeScript y Vite correcta.
- `cargo test -p audio-core -p noecho`: tres pruebas correctas, sin fallos. Cubren árbol de procesos, omisión de ancestros que contienen un proceso privado, eliminación de capturas duplicadas y formato de comandos.
- `cargo run -p audio-core --example mix-smoke`: reproducción de dos tonos desde procesos separados. El tono permitido se incluyó y el privado quedó fuera de la mezcla. Marcar y desmarcar tuvo efecto en la primera actualización confirmada; al reiniciar el proceso privado se volvió a detectar y excluir. La salida predeterminada quedó igual. La app privada reprodujo en Realtek USB Audio y la permitida en la salida predeterminada Ugreen. Sin advertencias del mezclador.
- API: consultas, exclusiones persistidas, ajustes de voz y escucha, y parada verificados. Petición sin token: 401; origen de navegador: 403; comando desconocido: 400. La instancia final de release respondió a la API.
- Inicio por API: el canal estándar estaba ocupado por otras aplicaciones y el motor rechazó activarlo, como corresponde. Esto comprueba la detección de conflicto; no equivale a una prueba de recepción remota.
- Revisión visual: ventana inicial a 1000 × 740, lista con desplazamiento propio, medidores en panel independiente, texto sin errores de codificación. Ejecutado el binario final de release.
- Instalador NSIS generado y copiado a `dist-installer/NoEcho_0.2.0_x64-setup.exe`. Se verificó que los hashes del original y la copia coinciden. No se publicó.

Pendiente en la PC destino: instalación completa, entrada del cable dedicada en el programa remoto, recepción del audio permitido y ausencia del privado, micrófono y desconexión/reconexión de dispositivos. La prueba local comprueba la mezcla de NoEcho; no confirma qué fuente captura un programa remoto externo. Consulta los requisitos y límites en el README.
