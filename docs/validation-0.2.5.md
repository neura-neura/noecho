# Validación 0.2.5

La consulta de niveles de UnifiedAudio pasa de un proceso PowerShell cada dos segundos a un único lector durante la sesión, con conexiones breves cada 100 ms. Cada medidor conserva el máximo desde la consulta anterior, independientemente de los niveles que lee la interfaz de UnifiedAudio. La interfaz de NoEcho suaviza la caída y muestra fuentes desactivadas, micrófono silenciado y errores de conexión. Detecta también un contador de renderizado detenido durante más de dos segundos.

Verificado:

- Motor C++ y frontend TypeScript compilados.
- Pruebas nativas: un pulso seguido de 200 bloques silenciosos permanece disponible para NoEcho, se consume una sola vez y no modifica el medidor de UnifiedAudio.
- Transporte por pipe real de Windows: activación, cambio de selección, desactivación y dos lecturas continuas; respuestas JSON con saltos de línea se entregan como una sola lectura completa.
- Conversión de estados: voz, programas, salida, micrófono silenciado y fuente de programas desactivada.
- Pruebas de mezcla existentes: conservación de voz y audio permitido al retirar la contribución seleccionada bajo cambios de ganancias.

En la instalación activa anterior los cuatro niveles devueltos por el motor eran cero. La configuración guardada de UnifiedAudio tenía el modo solo voz. Esto explica la ausencia del sistema, pero no determina por sí solo la causa de la falta de voz: se añadieron estados de silencio, señal de entrada y renderizado para distinguirla en la nueva versión. No se alteraron dispositivos, efectos ni el modo de mezcla de la instalación activa. La recepción desde otra PC aún requiere prueba con el instalador nuevo.
