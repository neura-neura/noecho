# Controlador / dispositivo de audio compartido

## Por qué hace falta

Las apps de escritorio remoto y OBS suelen capturar:

- el dispositivo de salida predeterminado, o
- el loopback del endpoint de renderizado activo.

Ese loopback incluye **todas** las sesiones mezcladas en ese endpoint. No existe una API pública de Windows que diga: “captura el predeterminado excepto Discord”.

Por eso NoEcho usa una **salida compartida universal**:

1. Las apps normales se reproducen en el dispositivo compartido.
2. Las apps privadas se reproducen en el dispositivo físico.
3. Un monitor interno reproduce el compartido también en el físico (tú oyes ambas).
4. El programa remoto captura solo el compartido.

## Nombre recomendado

`Audio compartido`

## Desarrollo (provisional)

Durante el desarrollo puedes usar un cable virtual firmado ya instalado, por ejemplo:

- VB-Audio Virtual Cable
- VB-Audio Cable A / B
- Otros endpoints virtuales detectados por nombre

NoEcho los reconoce como candidatos de salida compartida. **No se redistribuyen en este repo**; instálalos por separado y respeta su licencia.

En el apartado avanzado puedes elegir explícitamente el dispositivo compartido y el físico.

### Comprobación

```powershell
cargo run -p tech-probe -- devices
```

Debes ver al menos un dispositivo marcado como `[virtual]`.

## Producción (objetivo)

Para distribución pública se recomienda un driver propio o de terceros con:

- firma de código de modo kernel válida en Windows 11
- compatibilidad con Secure Boot
- sin necesidad de Test Mode ni desactivar verificación de firmas
- instalador/desinstalador limpio
- nombre amigable `Audio compartido`
- al menos un endpoint de render (y opcionalmente capture/loopback)

### Opciones evaluadas

| Opción | Pros | Contras |
|--------|------|---------|
| Driver propio basado en SYSVAD / APOs | Control total, nombre propio | Requiere EV cert + WHQL/attestation, mucho trabajo |
| Cable virtual comercial firmado | Rápido en desarrollo | Licencia, marca de terceros, no es “Audio compartido” |
| Vaciar/silenciar sesión | Simple | **No cumple**: silencia localmente o no evita captura |

## Instalación limpia (meta)

El instalador de NoEcho debería:

1. Detectar si ya existe `Audio compartido`.
2. Si no, instalar el paquete del driver firmado.
3. Verificar el endpoint activo.
4. Al desinstalar NoEcho, ofrecer quitar el driver solo si lo instaló NoEcho.

El desinstalador de la app **siempre** restaura dispositivos predeterminados y limpia preferencias por app antes de salir.

## Notas de seguridad

- No desactives la comprobación de firmas.
- No uses drivers sin revisar licencia y procedencia.
- El monitor local de NoEcho copia PCM del loopback compartido al dispositivo físico; no graba a disco ni envía red.
