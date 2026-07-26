# Limitaciones técnicas reales

## Lo que Windows permite (y no permite)

### Posible en modo usuario

- Enumerar endpoints (`IMMDeviceEnumerator`)
- Enumerar sesiones (`IAudioSessionManager2`)
- Obtener PID, ruta, icono y estado de reproducción
- Loopback de un dispositivo (`AUDCLNT_STREAMFLAGS_LOOPBACK`)
- Process loopback capture (Windows 10 2004+) para **validar** inclusión/exclusión de árboles de proceso
- Cambiar dispositivo predeterminado y preferencias por aplicación mediante PolicyConfig / AudioPolicyConfig (interfaces no documentadas pero ampliamente usadas)

### No suficiente por sí solo

- Silenciar o bajar volumen de una sesión: la app deja de oírse localmente o sigue en el mix según el capturador.
- Detectar StarDesk/Parsec/etc. y parchearlos uno a uno: frágil y no universal.
- Process loopback capture **no reemplaza** el dispositivo predeterminado que usan los programas remotos. Sirve para pruebas o capturadores propios, no para redirigir AnyDesk/Parsec sin que ellos usen esa API.

## Implicación

La exclusión universal real requiere una **ruta de render separada** (dispositivo virtual compartido) que los capturadores vean como “el audio del sistema”.

## Riesgos conocidos

1. **PolicyConfig no documentado**: puede cambiar entre builds de Windows. NoEcho intenta varios IID/offsets y degrada con mensaje claro.
2. **Apps que fijan su propio endpoint**: algunas recuerdan un dispositivo concreto y hay que reaplicar rutas al detectar nuevas sesiones.
3. **Electron/Chromium multi-proceso**: se agrupa por ejecutable y árbol; un PID suelto no basta.
4. **Cambio de audífonos Bluetooth/USB**: puede invalidar el monitor local; hay que actualizar el dispositivo físico.
5. **Sin dispositivo virtual**: la protección no puede activarse de forma universal; la UI lo indica y el motor restaura.
6. **Captura física directa**: si el usuario configura OBS/Parsec para capturar los auriculares físicos en lugar del compartido, oirá también lo privado. Es una limitación de configuración del capturador, no del enrutado.

## Recuperación

Antes de mutar el sistema se guarda:

- dispositivos predeterminados multimedia/comunicaciones
- apps excluidas
- ids de dispositivos compartido/físico

Se restaura al desactivar, salir, error, o al detectar sesión incompleta al inicio.

## Pruebas recomendadas con capturadores

| App | Método habitual | Resultado esperado con NoEcho |
|-----|-----------------|-------------------------------|
| StarDesk | Endpoint predeterminado / loopback | Solo ruta compartida |
| Parsec | Salida de Windows / hook de audio | Solo ruta compartida si usa default |
| RustDesk | Dispositivo de audio del sistema | Solo ruta compartida |
| AnyDesk | Audio del sistema | Solo ruta compartida |
| OBS Studio | WASAPI loopback del dispositivo elegido | Elegir `Audio compartido` / virtual |

Documenta en cada entorno real qué dispositivo captura cada app (`docs/remote-capture-notes.md`).
