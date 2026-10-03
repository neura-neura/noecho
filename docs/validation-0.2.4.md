# Validación de la integración con un solo cable

El motor se compila en el repositorio local de UnifiedAudio. El instalador combinado incluye ese motor y NoEcho 0.2.4; requiere UnifiedAudio 0.1.6 en su instalación estándar si está instalado.

Verificaciones realizadas:

- CMake/MSVC: motor y pruebas nativas compilan.
- Suite nativa: sin fallos, incluidas 5000 combinaciones de mezcla/ganancia y las identidades por ejecutable. La contribución seleccionada se retira después de mezclar con voz, usando los mismos frames y ganancia.
- Rust: mapeo de medidores y comunicación mediante un pipe real de prueba con selección inicial, cambio de app y parada. El servidor de esta prueba es simulado; no equivale a validar la llamada remota ni el motor de producción en ejecución.
- La integración de NoEcho controla el motor; no inicia ProcessMixer ni abre un segundo endpoint de salida cuando detecta UnifiedAudio.
- No se reemplazó la instalación en ejecución del usuario ni se modificaron sus perfiles para probar. La instalación completa y la llamada AyuGram/Parsec quedan pendientes de prueba del usuario.

La escucha por botones está deshabilitada en este modo; los medidores vienen del motor y se conserva la escucha configurada previamente. Sin UnifiedAudio se conserva el modo independiente anterior.
