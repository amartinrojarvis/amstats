# Historial de cambios

## 0.2.0 — 2026-11-06

- Comprobación diaria de nuevas versiones con Sparkle 2.10.1, búsqueda manual e instalación desde la app.
- Instalación automática opcional; preferencias persistentes y verificación Ed25519 del feed y del archivo antes de extraerlo.
- Acerca de con versión, autoría de Alberto Martín y enlaces a X e Instagram (@amartinro), además del repositorio.
- Versiones del ejecutable y del bundle sincronizadas desde Cargo.toml; scripts para preparar releases firmadas.
- Repositorio y descargas públicos. La versión 0.1.0 necesita una primera instalación manual de 0.2.0.

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
