# NoEcho 0.2.6

## Uso diario

1. Marca los programas que no quieres que oiga quien se conecta a tu PC.
2. Pulsa **Activar**. Tú sigues escuchándolos.
3. Al terminar, pulsa **Desactivar**. Parsec continúa recibiendo el audio, ahora sin filtro.

Puedes cambiar la selección mientras NoEcho está activado. No hace falta desmarcar y volver a marcar para aplicar un cambio. Cerrar la ventana conserva NoEcho en la bandeja si está habilitada esa opción.

## Instalación para Parsec

Cierra **Parsec**, **UnifiedAudio** y **NoEcho** desde sus bandejas. Ejecuta `NoEcho_0.2.6_un-solo-cable_setup.exe`. Después abre UnifiedAudio, NoEcho y Parsec. NoEcho debe permanecer abierto, aunque sea en la bandeja, para entregar la mezcla a Parsec.

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

La API controla la instancia abierta de NoEcho en la sesión del usuario de la PC destino. NoEcho debe ejecutarse (puede estar en la bandeja). No es un servicio de Windows y no inicia sesión ni reproduce audio en una sesión cerrada.

Escucha en `http://127.0.0.1:47832`. Cada apertura genera un token en `%LOCALAPPDATA%\NoEcho\api-token`; todas las solicitudes requieren `Authorization: Bearer <token>`. El token cambia al reiniciar. La API escucha solo en localhost; usa SSH, PowerShell Remoting o un túnel SSH desde otra PC. No abras este puerto directamente a Internet.

El script `scripts/NoEcho-Control.ps1` se distribuye junto al instalador y como recurso de la aplicación. Guárdalo, por ejemplo, en `C:\NoEcho\NoEcho-Control.ps1` en la PC destino.

```powershell
.\NoEcho-Control.ps1 -Command status
.\NoEcho-Control.ps1 -Command apps
.\NoEcho-Control.ps1 -Command exclude -Apps discord.exe,spotify.exe
.\NoEcho-Control.ps1 -Command start
.\NoEcho-Control.ps1 -Command include -Apps spotify.exe
.\NoEcho-Control.ps1 -Command monitor -Value voice
.\NoEcho-Control.ps1 -Command monitor -Value remote
.\NoEcho-Control.ps1 -Command monitor -Value none
.\NoEcho-Control.ps1 -Command voice -Value true
.\NoEcho-Control.ps1 -Command meters
.\NoEcho-Control.ps1 -Command stop
```

| Comando | Parámetros | Resultado |
| --- | --- | --- |
| `status`, `apps`, `devices`, `config`, `meters` | Ninguno | Datos en JSON. |
| `start` | `-Apps` opcional | Inicia la mezcla; apps reemplaza las exclusiones. Permite reaplicar una mezcla activa. |
| `stop` | Ninguno | Detiene mezcla y escucha sin cambiar salidas locales. |
| `exclude` / `include` | `-Apps nombre.exe,...` | Añade o elimina exclusiones y confirma su aplicación. |
| `set-exclusions` | `-Apps` o lista vacía | Reemplaza la lista, incluidas apps que se abrirán después. |
| `monitor` | `-Value none/voice/system/private/remote` | Selecciona escucha de prueba. |
| `voice` | `-Value true/false` | Incluye o quita la voz de la mezcla. |
| `channel` | `-Value ID` o `automatic` | Elige canal virtual con la mezcla detenida; usa `devices` para obtener IDs. |

El script devuelve JSON y termina con código 1 si falla. No imprime el token. Usa el mismo usuario de Windows que ejecuta NoEcho; otro perfil tiene otra carpeta de AppData.

### Desde otra PC mediante SSH

Con OpenSSH ya habilitado y autorizado en la PC destino:

```powershell
ssh usuario@PC-DESTINO 'powershell -NoProfile -File C:\NoEcho\NoEcho-Control.ps1 -Command exclude -Apps discord.exe'
ssh usuario@PC-DESTINO 'powershell -NoProfile -File C:\NoEcho\NoEcho-Control.ps1 -Command start'
ssh usuario@PC-DESTINO 'powershell -NoProfile -File C:\NoEcho\NoEcho-Control.ps1 -Command meters'
```

Con PowerShell Remoting configurado:

```powershell
Invoke-Command -ComputerName PC-DESTINO -Credential (Get-Credential) -ScriptBlock {
  & 'C:\NoEcho\NoEcho-Control.ps1' -Command exclude -Apps 'discord.exe','spotify.exe'
}
```

### HTTP para tus herramientas

`GET /v1/status`, `/v1/apps`, `/v1/devices`, `/v1/config` y `/v1/meters` consultan datos. `POST /v1/command` acepta JSON:

```powershell
$apiToken = (Get-Content "$env:LOCALAPPDATA\NoEcho\api-token" -Raw).Trim()
$headers = @{ Authorization = "Bearer $apiToken" }
Invoke-RestMethod http://127.0.0.1:47832/v1/command -Method Post -Headers $headers `
  -ContentType application/json -Body '{"command":"exclude","apps":["discord.exe"]}'
Invoke-RestMethod http://127.0.0.1:47832/v1/command -Method Post -Headers $headers `
  -ContentType application/json -Body '{"command":"voice","value":false}'
```

Respuestas: `{"ok":true,"data":...}` o `{"ok":false,"error":"..."}`. Errores de comando: HTTP 400; autenticación: 401; rutas desconocidas: 404; orígenes de navegador: 403. El límite de cuerpo es 64 KiB. Consulta `status.warnings` y `meters.errors`: iniciar no garantiza que todos los procesos sean capturables.

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
