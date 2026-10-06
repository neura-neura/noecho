# NoEcho 0.2.10

## Uso diario

1. Marca los programas que no quieres que oiga quien se conecta a tu PC.
2. Pulsa **Activar**. Tú sigues escuchándolos.
3. Al terminar, pulsa **Desactivar**. Parsec continúa recibiendo el audio, ahora sin filtro.

Puedes cambiar la selección mientras NoEcho está activado. No hace falta desmarcar y volver a marcar para aplicar un cambio. Cerrar la ventana conserva NoEcho en la bandeja si está habilitada esa opción.

En **Ajustes → Iniciar con Windows**, puedes hacer que NoEcho se abra en la bandeja al iniciar sesión. Desmarcarlo elimina ese inicio automático. Si se interrumpe la lectura de UnifiedAudio, NoEcho intenta reconectar y reaplicar las apps seleccionadas; los errores de niveles antiguos dejan de mostrarse al cerrar la comprobación de sonido.

## Instalación para Parsec

Ejecuta `NoEcho_0.2.10_un-solo-cable_setup.exe`. La casilla «Cerrar UnifiedAudio y Parsec durante la instalación» está marcada por defecto. Cierra ambos programas para preparar el audio; puedes abrirlos de nuevo al terminar. Si la desmarcas, puedes dejarlos abiertos. Si el motor de UnifiedAudio y la ruta de Parsec ya están preparados, el instalador los conserva sin modificarlos. NoEcho debe permanecer abierto, aunque sea en la bandeja, para entregar la mezcla a Parsec. «Iniciar con Windows» viene activado por defecto y se aplica también al actualizar desde versiones anteriores. Puedes desactivarlo en Ajustes; tu elección se conservará en las siguientes ejecuciones y actualizaciones.

Si se necesita reemplazar un motor en uso o preparar una ruta nueva de Parsec, la instalación continúa y guarda ese cambio pendiente. Un asistente lo aplica automáticamente cuando la aplicación termina de usarlo; también retoma los pendientes al iniciar sesión. Con la casilla desmarcada, no fuerza el cierre ni reinicio de UnifiedAudio o Parsec. La app muestra cuándo hay una preparación pendiente; una ruta de Parsec todavía pendiente no se presenta como filtrada.

Esta integración necesita **Steam Streaming Speakers**, una salida virtual ya instalada en el equipo de desarrollo. El instalador detecta su identificador real y configura Parsec automáticamente para capturarla con su cancelación propia desactivada. No instala Steam ni otro cable. Si esa salida no existe, explica el requisito y no configura Parsec hacia una salida inexistente. Los cambios de Parsec se realizan solo con Parsec cerrado; se guarda `config.json.before-noecho` junto a su configuración.

Las apps conservan sus salidas habituales. Windows conserva tus altavoces y auriculares predeterminados. Solo cambia la fuente que captura Parsec: recibe la mezcla de NoEcho en lugar del audio original de los altavoces. Su filtro nativo New captura todas las salidas y no sirve para esta ruta; debe permanecer apagado. [Parsec describe este comportamiento](https://support.parsec.app/hc/en-us/articles/32381681933204-Eliminating-Sound-Echo-While-Co-Op-ing-With-Friends).

El instalador incluye el motor compatible de UnifiedAudio 0.1.6 para su instalación estándar. Conserva sus efectos, perfiles y dispositivos; guarda el motor anterior con la extensión `.before-noecho`. Otras versiones o ubicaciones necesitan una integración adaptada.

## Micrófono de UnifiedAudio

Hay dos rutas independientes:

- **Micrófono:** procesamiento de UnifiedAudio → filtro de contribuciones de NoEcho → tu mismo `CABLE Input` → micrófono virtual de tus apps de llamada.
- **Escritorio remoto:** captura de programas de NoEcho → mezcla sin las apps marcadas → Steam Streaming Speakers → Parsec.

**Voice only**, **PC audio only** y **Both** siguen controlando únicamente el contenido del micrófono virtual. Puedes usar cualquiera de ellos. NoEcho captura y filtra los programas para Parsec independientemente de esa selección. No agrega el micrófono otra vez a Parsec cuando UnifiedAudio lo entrega a tu llamada, para evitar duplicar tu voz.

Si eliges Both o PC audio only, la etapa de NoEcho en UnifiedAudio también retira las contribuciones marcadas de ese micrófono. Esto se realiza antes de escribir en su cable existente, sin recapturar y volver a escribir en el mismo cable. El filtro de contribuciones no sustituye un cancelador de eco acústico de altavoces recogidos físicamente por el micrófono.

Si UnifiedAudio no está abierto, la captura de programas para Parsec sigue funcionando. NoEcho no requiere instalar UnifiedAudio para filtrar el escritorio remoto.

## Comprobar el sonido

- **Mi micrófono virtual:** salida procesada de UnifiedAudio, que puede contener voz, PC o ambos. Sin UnifiedAudio se mide el micrófono de comunicaciones.
- **Todos los programas:** programas capturados, independientemente del modo de UnifiedAudio.
- **Los programas que elegí:** contribuciones que se omiten de la mezcla remota.
- **El sonido que enviamos:** mezcla entregada a la salida que captura Parsec.

Pulsa **Escuchar** con auriculares para probar una señal. Cerrar esta ventana detiene la escucha. Una barra confirma la señal local; la recepción se comprueba desde la otra PC. Una fuente silenciosa muestra silencio, sin atribuirlo al modo Voice only.

## Requisitos y límites

Windows 10 versión 2004 (build 19041) o posterior y una salida remota independiente. [Microsoft documenta la captura por proceso desde Windows 10 versión 2004](https://learn.microsoft.com/en-us/samples/microsoft/windows-classic-samples/applicationloopbackaudio-sample/). Esta integración automática utiliza la salida de Steam existente; no requiere Cable A/B. El mismo cable de micrófono no puede representar a la vez dos mezclas distintas.

Los sonidos de Windows sin PID propio, audio protegido, exclusivo o inaccesible pueden no estar disponibles. Si apps comparten un árbol de procesos, se prioriza omitir el árbol que podría contener audio privado. Otra app escribiendo directamente en la salida interna podría introducir audio ajeno; NoEcho comprueba que esté libre al iniciar. Parsec puede volver a Default si desaparece el dispositivo, por lo que debes conservar disponible esa salida. No se afirma integración automática con otros programas remotos.

**Desactivar** conserva una mezcla sin exclusiones para que Parsec no pierda el sonido. **Salir completamente de NoEcho** detiene esa mezcla. Para volver a usar Parsec sin NoEcho, selecciona Default en Parsec y restaura su cancelación según tu configuración anterior; el archivo de respaldo conserva los valores originales. No cambies el cable del micrófono de UnifiedAudio.

## API y comandos del sistema

El control por red está disponible al instalar NoEcho 0.2.10. No requiere clave, SSH ni habilitar un botón. El instalador solicita el permiso de administrador de Windows para crear una regla de firewall que permite TCP 47832 para NoEcho desde la misma subred, en redes físicas o virtuales como ZeroTier. Todos los equipos de esa subred pueden controlar NoEcho. No publiques este puerto en Internet.

NoEcho debe permanecer abierto en la PC destino, aunque esté en la bandeja. La API escucha en las direcciones IPv4 de ese equipo y en localhost. Consulta la IP de la PC destino en Windows y utiliza los comandos de esta sección. Si cambia la IP de la red, utiliza la nueva IP.

Desde PowerShell en tu PC, estos ejemplos controlan el equipo `192.168.196.211`:

```powershell
# Desactivar el filtro, conservando el audio completo de Parsec
Invoke-RestMethod 'http://192.168.196.211:47832/v1/command' -Method Post -ContentType 'application/json' -Body '{"command":"stop"}'
# Activar con las apps ya seleccionadas
Invoke-RestMethod 'http://192.168.196.211:47832/v1/command' -Method Post -ContentType 'application/json' -Body '{"command":"start"}'
# Consultar el estado
Invoke-RestMethod 'http://192.168.196.211:47832/v1/status'
# Añadir una app a las exclusiones
Invoke-RestMethod 'http://192.168.196.211:47832/v1/command' -Method Post -ContentType 'application/json' -Body '{"command":"exclude","apps":["telegram.exe"]}'
```

También se incluye `scripts/NoEcho-Control.ps1` en la instalación:

```powershell
.\NoEcho-Control.ps1 -Computer 192.168.196.211 -Command stop
.\NoEcho-Control.ps1 -Computer 192.168.196.211 -Command start
.\NoEcho-Control.ps1 -Computer 192.168.196.211 -Command status
.\NoEcho-Control.ps1 -Computer 192.168.196.211 -Command exclude -Apps telegram.exe
```

Sin `-Computer`, el script controla NoEcho en tu propia PC. `-BaseUrl` permite elegir otra URL.

| Comando | Parámetros JSON | Resultado |
| --- | --- | --- |
| `status`, `apps`, `devices`, `config`, `meters` | Ninguno | Consulta los datos. |
| `start` | `apps` opcional | Activa el filtro; si apps no está vacío, reemplaza las exclusiones. |
| `stop` | Ninguno | Desactiva el filtro y la escucha; Parsec conserva su mezcla sin exclusiones. |
| `exclude` / `include` | `apps`: lista de ejecutables | Añade o elimina exclusiones. |
| `set-exclusions` | `apps`: lista, puede estar vacía | Reemplaza las exclusiones. |
| `monitor` | `value`: `none`, `voice`, `system`, `private`, `remote` | Selecciona la escucha de prueba. |
| `voice` | `value`: booleano | Incluye o quita el micrófono en la mezcla remota cuando no lo entrega UnifiedAudio. |
| `channel` | `value`: ID o null | Elige canal en el modo manual con la mezcla detenida. Parsec administra su canal automáticamente. |

`GET /v1/status`, `/v1/apps`, `/v1/devices`, `/v1/config` y `/v1/meters` consultan datos. `POST /v1/command` recibe los comandos anteriores. Respuestas: `{"ok":true,"data":...}` o `{"ok":false,"error":"..."}`. Un comando inválido devuelve HTTP 400; una ruta desconocida, 404; una solicitud con cabecera Origin de navegador, 403. El cuerpo máximo es 64 KiB. Revisa `status.warnings` y `meters.errors` para comprobar la captura.

Si Windows deniega el permiso de firewall durante la instalación, se muestra un aviso y el acceso desde la red puede quedar bloqueado. Vuelve a ejecutar el instalador y acepta ese permiso. No necesitas modificar dispositivos de audio.

## Desarrollo y generar instalador

```powershell
npm ci
npm run build
cargo test -p audio-core -p noecho
# Prueba optativa con tonos suaves. Conserva los defaults.
cargo run -p audio-core --example mix-smoke
# Prueba de la salida interna de Parsec, incluida la señal del micrófono virtual.
cargo run -p audio-core --example mix-smoke -- --internal
npm run installer
```

El instalador y el script quedan en `dist-installer/`. Generarlo no publica ni sube la versión. Valida finalmente con el capturador y los dispositivos de tu PC destino.
