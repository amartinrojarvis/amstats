# Historial de cambios

## 0.1.0 — 2026-11-06

Primera versión de AMStats para macOS 13+ en Apple Silicon.

- CPU, RAM, GPU, almacenamiento, subida/bajada y batería con estimaciones del sistema.
- Barra unificada de 212 pt, alineada en dos líneas, con unidades abreviadas y detalle al pulsar.
- Resumen nativo, ajustes con vista previa, indicadores individuales e inicio automático mediante ServiceManagement.
- Intervalos de 1, 2 y 5 segundos; ahorro en batería, pausa en reposo y colectores desactivables.
- Lecturas Mach/IOKit, un hilo de muestreo, paneles creados bajo demanda y redibujado solo cuando cambia el texto.
- Diez pruebas, scripts de empaquetado y medición, y verificación automática en GitHub Actions.
- Mediciones locales documentadas frente a Stats, con condiciones y límites explícitos.

La aplicación descargable usa firma local ad hoc y no está notarizada. La compatibilidad de las propiedades de GPU depende de macOS. Los ciclos físicos completos de descarga/reposo y todas las generaciones de Apple Silicon todavía no se han validado.
