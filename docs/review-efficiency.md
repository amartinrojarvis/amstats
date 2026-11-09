# Revisión general de eficiencia — 2026-11-09

Revisión del código Rust de arranque, configuración, métricas, frontera FFI, procesos, interfaz y actualizador antes de otra publicación. Referencia local inicial: commit 30d94ec más la corrección de salud/tiempo; revisión y optimizaciones incluidas en la versión 0.4.2.

## Decisiones por componente

| Componente | Revisión y resultado |
| --- | --- |
| main/config | Arranque sin servidor ni temporizadores adicionales. Se mantienen preferencias, frecuencias y ahorro de batería. |
| metrics | Un hilo Utility con espera bloqueante; módulos apagados sin lecturas, pausa en reposo, coalescencia de callbacks, buffers de red reutilizados, lecturas lentas cada 30 s. Se conserva esta arquitectura y sus algoritmos. |
| platform | Se revisan recursos Mach/IOKit/CF con RAII y contadores nativos. El tiempo de la batería usa directamente los minutos de powerd. La salud oficial requiere el informe del sistema; se ejecuta bajo demanda, fuera del hilo de UI, con timeout del informe y caché. |
| processes | Caché de salud retenida a través de resets; baselines, nettop e identidades de proceso sí se reinician para evitar tasas viejas. GPU/red enumeran solo sus PID conocidos. Número de núcleos obtenido una sola vez y solo si hace falta CPU. Reservada capacidad para filas a partir de la muestra anterior. |
| red por proceso | Líneas prestadas de un bloque del búfer, un solo drain por lote, parser sin Vec temporal, rotación de tablas con swap/clear y nombres actualizados solo si cambian. Un nettop con agregación por proceso, sin DNS y únicamente con el detalle de red activo. |
| ranking | Selección del prefijo de diez grupos antes de ordenarlo, en lugar de ordenar todo. No se descartan procesos antes de sumar apps; desglose y órdenes de E/S conservados. Prueba contra ordenación completa de 300 procesos para los tres órdenes, agrupados y sin agrupar. |
| ui | Barra formatea solo sus valores; no construye detalles descartados. Ayudas y accesibilidad por indicador se actualizan si cambian, incluso cuando la cifra redondeada no cambia. Se elimina la ayuda agregada del botón oculto. Resumen evita reasignar textos iguales. |
| ui/detail | Font y color solo al seleccionar recurso. Iconos solo al cambiar de app en la fila. La caché limitada retiene iconos visibles al podarla, sin vaciarla completa. Historial sigue limitado a 61 muestras y gráficos siguen en tres trazados sin animación periódica. |
| updates | Sparkle conserva comparación de versiones, verificación de firmas y planificación propia; no se añaden comprobaciones, red ni timers de AMStats. |

Se mantiene la frecuencia y el significado de las métricas. No se sustituye el tiempo del sistema por un algoritmo de suavizado o proyección. Se conservan TCP y UDP y los procesos completos para agruparlos correctamente.

## Validación

- `scripts/check.sh`: formato, Clippy y 20 pruebas correctos.
- `scripts/bundle.sh`: release ARM64 y firma ad hoc verificadas.
- Diagnóstico de los seis colectores: todos alcanzan ready; GPU con 41 PID / 42 clientes; red con 27 PID, tasas positivas y parser operativo. Pruebas realizadas en paralelo, por lo que no son benchmarks de app completa ni una comparación de rendimiento entre módulos.
- Interfaz real: vista previa de barra, lista de memoria con iconos y agrupación, navegación a red y batería. Salud final 97 %, coincidente con macOS.
- Tras salir de red, comprobación de procesos: AMStats sigue abierto y no queda nettop.
- La salud sigue usando caché de 15 minutos y reintento de 60 s si falta dato; la prueba de reset comprueba que cerrar el panel no la elimina, mientras sí elimina los baselines.
- Sigue pendiente la validación física de un tiempo no nulo durante carga/descarga; solo se ha podido contrastar el estado de carga completa conectado.

## Medición real y límites

Datos en `benchmark-optimization.json`. Mac ARM64, 8 núcleos lógicos y 8 GiB, macOS 27.2, seis módulos, intervalo 5 s, ahorro activo, alimentación externa y batería llena. Muestras de 30 s de app completa. En Ajustes, la compilación optimizada registró CPU 0,0225 % de un núcleo y huella física 31,75 MiB. Tras abrir paneles, la huella en red fue 64,36 MiB; el estado de las vistas y las cachés importa.

No se demuestra un ahorro porcentual global: las cifras de CPU son muy pequeñas, el estado de calentamiento difiere y el tráfico no estaba controlado. En el detalle de red el coste dominante es nettop, medido por separado: 1,6778 % antes y 3,3991 % después con tráfico distinto, sin cambios en su comando. No se atribuye esa variación a una mejora o regresión del código de AMStats. Al salir del detalle se elimina ese proceso. La medición del proceso principal no lo incluye.

Las mejoras comprobables son menos llamadas y asignaciones, caché persistente para metadatos lentos y menor trabajo de ranking, sin reducir información ni la frecuencia seleccionada. No se afirma que toda la aplicación sea óptima ni una reducción de autonomía medida en horas.
