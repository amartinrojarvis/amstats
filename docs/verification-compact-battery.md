# AMStats 0.4.1: barra compacta y batería gráfica — 2026-11-09

- `scripts/check.sh`: formato, Clippy y 17 pruebas correctos, incluidos valores largos y tiempos desconocidos/unidades.
- `scripts/bundle.sh`: compilación y verificación de firma ad hoc correctas.
- Vista previa real de Ajustes revisada en macOS: batería llena con número integrado. Tras la revisión visual se simplificó a un relleno sin contorno, con padding alrededor del número y alineado con la fila de valores. Se comprobó el estado AC al estar cargada y conectada. No se ha validado físicamente una descarga completa ni un ciclo de carga; no se afirma una mejora cuantificada del error temporal.
- Se mantiene el intervalo de alimentación de 30 segundos y las notificaciones existentes.

## Medición de la aplicación completa

Medición de 15 s durante esta implementación, antes del ajuste final de altura, con los seis módulos activos, intervalo de 5 s, ahorro en batería activo, alimentación de red y batería al 100 %, Ajustes abierto con la vista previa; sin panel de procesos. macOS 27.2, ARM64; modelo concreto no disponible en el entorno restringido. Proceso del bundle `dist/AMStats.app`, PID 14597.

CPU: 0,0227 % de un núcleo. Huella física: 32,16 MiB. RSS: 98,94 MiB. Despertares de reposo: 0/s. Lectura y escritura de disco del proceso: 0 bytes. Es una muestra breve, no una comparación controlada ni una medida de autonomía.

## Paquete 0.4.1

ZIP sin errores de integridad, versión 0.4.1 en ambos campos del bundle, longitud y URL de appcast comprobadas, SHA-256 verificados. Firmas Ed25519 del ZIP y del feed verificadas con Sparkle. Bundle ad hoc, sin notarización.
