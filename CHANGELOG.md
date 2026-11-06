# Historial de cambios

## 0.3.1 — 2026-11-06

- Corrige la selección desde la barra: cada indicador tiene un botón nativo con identidad propia, sin deducir el recurso de las coordenadas del evento.
- CPU abre CPU, batería abre batería y el desplegable se ancla al indicador pulsado.
- Elimina la apertura automática de ventanas al usar el menú o los atajos. La ventana separada solo se crea al pulsar «Fijar»; volver a la barra recupera el desplegable.
- Elimina el resaltado de fondo del bloque completo y conserva las dos líneas compactas: 206 pt de anchos solicitados, dentro de los 212 pt anteriores.
- Solo redibuja el indicador cuyo texto cambia, sin temporizadores ni lecturas adicionales.

## 0.3.0 — 2026-11-06

- Panel contextual para cada indicador, con tarjetas, pestañas, colores por recurso y opción de fijar como ventana.
- Gráficos de barras LED/LCD con historial local de 60 segundos. Red dividida desde el centro: subida arriba, descarga abajo, con escala común.
- Subida en la línea superior de la barra y descarga en la inferior; cada línea selecciona su orden de procesos.
- Procesos de mayor a menor consumo para CPU, RAM, GPU, disco, red y energía, agrupables por app y con desglose, PID y memoria.
- Pausa de lecturas, orden estable al pasar el cursor y atajos ⌘1–⌘6.
- Potencia declarada del cargador, carga/descarga de batería, salud estimada, ciclos y capacidad; identificación de procesos que impiden reposo.
- Consultas de procesos solo mientras el detalle esté visible; lector de red único que termina al cerrar, pausar o cambiar de recurso. Gráficos sin animaciones ni temporizadores adicionales.
- Dieciséis pruebas y validación de formato, Clippy y firma del bundle.

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
