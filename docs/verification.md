# Verificación local — 6 de noviembre de 2026

Equipo: Apple Silicon, 8 GiB de RAM, macOS 27.2. Compilación ARM64 de producción, Rust 1.99.1. Alimentación externa. Los resultados son observaciones de este equipo, no límites garantizados para otros dispositivos.

## Resultado entregado

- `dist/AMStats.app`, firma ad hoc verificada con `codesign --verify --strict`.
- Ejecutable ARM64 de 403.568 bytes. Solo enlaza frameworks y bibliotecas del sistema; no incorpora runtime web ni Swift.
- Barra compacta de **212 pt** con los seis indicadores activos; versión inicial de seis bloques: 386 pt. Reducción del **45,1 %**. El ancho se reduce al apagar indicadores.
- Separación de 2 pt, márgenes exteriores de 4 pt, columnas estables y porcentaje con tipografía pequeña. Red abreviada y tiempo de batería conservado.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, diez pruebas y validación del plist: correctos.

## Comprobaciones funcionales

Lecturas reales verificadas de CPU, memoria, GPU, almacenamiento APFS, red y batería. La GPU devuelve valores reales en este equipo. La batería se probó físicamente conectada y cargada al 100 %; carga, descarga, tiempo desconocido y carga en pausa se verificaron también con pruebas de estados, sin simular que se hubiera desconectado el cargador.

Interfaz AppKit revisada visualmente en modo oscuro: ajustes, resumen y vista previa del indicador compacto. Se activaron y desactivaron los seis indicadores y se comprobó la persistencia de las preferencias al reiniciar. Registro y anulación de inicio automático mediante `SMAppService`: ambos correctos. Se dejó **desactivado** al terminar. No se cerró la sesión del usuario para comprobar un arranque real tras login.

Las pruebas de anchura usan las fuentes reales de AppKit y valores extremos (100 %, tasas altas de red y tiempo de batería largo), para detectar recortes. El reposo físico del equipo y la descarga real no se forzaron durante el desarrollo. La pausa y reinicio de bases están implementados mediante notificaciones de macOS; conviene validar un ciclo real de reposo en uso prolongado.

## Mediciones

`benchmark.py` toma diferencias entre contadores de `proc_pid_rusage`, fuera del proceso medido. CPU se expresa respecto a **un núcleo**, no al total de núcleos. La huella física es memoria atribuida al proceso; RSS incluye páginas compartidas y no se debe presentar como la misma medida.

| Condiciones, 30 segundos | CPU / núcleo | Huella física | RSS | Despertares de reposo/s | E/S de disco |
| --- | ---: | ---: | ---: | ---: | ---: |
| Antes del rediseño compacto; seis indicadores, 2 s, paneles sin abrir | 0,0476 % | 16,91 MiB | 73,58 MiB | 0,333 | 0 bytes |
| Versión compacta; seis indicadores, 2 s, Ajustes y vista previa abiertos | 0,0268 % | 32,83 MiB | 100,17 MiB | 0,100 | 0 bytes |

Las condiciones son diferentes: **esta tabla no demuestra una mejora de CPU entre versiones**. La app puede ser regulada por App Nap y los valores cambian con actividad, ventanas, carga y decisiones del sistema. En esa primera verificación no se hizo una comparación directa con Stats. La medición posterior se documenta a continuación. No se ha hecho una medición energética de larga duración.

Datos completos: [`benchmark-idle.json`](benchmark-idle.json) y [`benchmark-compact-settings.json`](benchmark-compact-settings.json).

Para repetir una comparación válida: mismas métricas e intervalo, misma alimentación y carga, paneles cerrados, calentamiento previo y al menos 60–300 s por aplicación, ejecutándolas por separado. El modo `--diagnose` mide solo el muestreador y no sustituye esta medición de la app completa.


## Comparación posterior con Stats 3.0.25

A petición del usuario se abrió la instalación existente de `/Applications/Stats.app`, que estaba cerrada, y se midieron ambas aplicaciones durante 30 segundos. Se conservaron las preferencias de Stats: CPU, RAM, GPU, disco, red y batería activos; sensores, Bluetooth y reloj desactivados. AMStats mantuvo los mismos seis indicadores a 2 segundos, con sus ajustes ya abiertos. El panel de Stats se abrió después de terminar la medición para cerrar la aplicación.

| Aplicación | Huella física al terminar | RSS al terminar |
| --- | ---: | ---: |
| AMStats compacta | 34.64 MiB | 96.03 MiB |
| Stats 3.0.25 | 172.22 MiB | 280.47 MiB |

En esta observación AMStats utilizó **79.9 % menos huella física**, una diferencia de 137.58 MiB. Son mediciones actuales, no registros históricos del uso anterior de Stats. No se igualaron los intervalos internos de Stats ni se normalizó el tiempo desde el arranque; ambas aplicaciones estaban ejecutándose simultáneamente. Por tanto, sirven como comparación puntual de memoria con las configuraciones instaladas, no como benchmark controlado de CPU o energía ni como garantía permanente.

Se cerró Stats al finalizar y se dejó AMStats en ejecución. Los datos están en [`benchmark-stats-installed.json`](benchmark-stats-installed.json) y [`benchmark-amstats-comparison.json`](benchmark-amstats-comparison.json).
