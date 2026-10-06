# Micrófono y escritorio remoto

El micrófono de UnifiedAudio y la captura de Parsec son rutas distintas. Voice only, PC audio only y Both pertenecen al micrófono virtual.

La versión 0.2.10 mantiene CABLE Input en UnifiedAudio y utiliza Steam Streaming Speakers, ya instalado, como salida interna de la mezcla de NoEcho para Parsec. Las apps siguen saliendo por sus dispositivos habituales. No se usan Cable A/B ni se cambian los dispositivos predeterminados.

El paquete combinado incluye una casilla marcada por defecto para cerrar Parsec y UnifiedAudio durante la instalación. Desmárcala si quieres dejarlos abiertos. Se conservan las integraciones ya preparadas y se aplazan automáticamente los cambios que todavía estén en uso. El instalador prepara la fuente de Parsec. Abre los tres programas y marca AyuGram en NoEcho. Usa el modo de UnifiedAudio que necesites para tu micrófono.

Comprueba desde la otra PC que AyuGram no se oye por Parsec y que un programa permitido sí se oye. Desactivar conserva el sonido sin filtro. NoEcho debe seguir ejecutándose para entregar la mezcla remota.

Consulta el [README](../README.md) para requisitos, escucha, control remoto y cómo volver a la captura original de Parsec.