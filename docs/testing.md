# Smoke tests

## Automatizable sin capturador

```powershell
cargo run -p tech-probe -- smoke
cargo run -p tech-probe -- devices
cargo run -p tech-probe -- sessions
cargo run -p tech-probe -- groups
```

## Manual (aceptación)

1. YouTube sonando.
2. Discord en llamada.
3. Activar NoEcho con Discord privado.
4. Local oye ambos.
5. OBS/Parsec/RustDesk en salida compartida no oyen Discord.
6. Restaurar.
7. Task Manager → End task NoEcho con protección activa → reabrir → audio recuperado.

## Recuperación

```powershell
# Simular restauración
cargo run -p tech-probe -- deactivate
```
