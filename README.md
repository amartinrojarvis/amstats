# AMStats

Monitor de barra de menús para **macOS 13+ y Apple Silicon**, escrito en Rust con interfaz AppKit. CPU, RAM, GPU, almacenamiento, subida/bajada de red y batería con tiempo estimado de carga o autonomía.

La barra reúne los seis indicadores en **un único bloque de 212 pt**, con separación de 2 pt y dos líneas alineadas. Los valores usan cifras de ancho fijo y porcentajes pequeños. Los indicadores ocultos desaparecen también del muestreo. Un clic abre el detalle; desde Ajustes se puede abrir además un resumen en ventana.

## Abrir e instalar

Descarga la aplicación ARM64 y su checksum en [Releases](https://github.com/amartinrojarvis/amstats/releases/tag/v0.1.0). El repositorio y sus descargas son privados. El ZIP contiene `AMStats.app`. Si compilas localmente, se genera en `dist/AMStats.app`; los binarios no se guardan en Git.

Para instalarla, cópiala a `~/Applications` o `/Applications` **antes** de activar el inicio automático. No necesita permisos de administrador para medir.

Para compilar necesitas Rust estable y las Command Line Tools de Xcode:

```sh
git clone https://github.com/amartinrojarvis/amstats.git
cd amstats
./scripts/bundle.sh
# Opcional: instalar en ~/Applications (con AMStats cerrada)
./scripts/bundle.sh --install
```

La compilación genera una app ARM64 con firma local ad hoc. La distribución a otros equipos requiere tu identidad Developer ID y notarización de Apple; la firma local no equivale a notarización. Puedes pasar `SIGNING_IDENTITY` al script. El generador Swift de iconos se ejecuta únicamente durante el empaquetado si falta el icono; **el ejecutable y su lógica son Rust**, sin runtime Swift propio.

## Uso

- **CPU / RAM / GPU / SSD:** porcentaje de utilización. SSD expresa espacio ocupado, no actividad de entrada/salida.
- **↓ / ↑:** descarga y subida. En la barra, `K`, `M` y `G` equivalen a KB/s, MB/s y GB/s decimales. El panel muestra las unidades completas.
- **Batería:** porcentaje y tiempo disponible; `↗` indica tiempo hasta completar la carga y `↘`, autonomía. `AC` indica alimentación externa. El panel distingue carga completa, carga en pausa y estimación todavía no disponible.
- **Ajustes:** activar indicadores, seleccionar 1/2/5 segundos, reducir frecuencia en batería e iniciar sesión automáticamente. La vista previa reproduce la barra real.
- **Atajos con la app activa:** `⌘,` ajustes, `⌘R` resumen y `⌘Q` salir.
- Al desactivar todos los indicadores permanece un pequeño acceso **AM** para volver a los ajustes.

Las preferencias se guardan con `NSUserDefaults`, dominio `dev.amartin.amstats`. El inicio automático se consulta y modifica a través de `SMAppService`; si macOS requiere aprobación se muestra su estado y un acceso a Ítems de inicio. No se activa por defecto.

## Por qué consume poco

- Un hilo de muestreo de prioridad Utility y el hilo principal de AppKit. Sin Electron, WebView, Tokio, servidor, consultas de procesos ni ejecución periódica de comandos.
- CPU y RAM mediante Mach; GPU y alimentación mediante IOKit; red mediante contadores de 64 bits del kernel; espacio mediante `statfs`.
- Un solo estado pendiente para la interfaz: las actualizaciones no se acumulan. La barra se redibuja únicamente cuando cambia el texto mostrado.
- Los paneles se construyen al abrirlos y solo actualizan sus vistas cuando están visibles.
- Disco y batería cada 30 segundos; cambios de alimentación también por notificaciones. El modo de ahorro duplica el intervalo al funcionar con batería.
- Sin muestreo periódico con todos los indicadores apagados. Pausa durante reposo del sistema o de las pantallas; reinicia las bases de CPU/red al despertar para evitar picos falsos.
- Sin historial persistente, telemetría, tráfico propio ni actualizador residente. Contadores, buffers y conexiones a servicios se reutilizan; los recursos nativos se liberan con RAII.
- Binario de aproximadamente **404 KB** y aplicación de aproximadamente **496 KiB**, incluido el icono; pueden variar según compilador.

Si está activado el ahorro en batería, se consulta alimentación aunque su indicador esté oculto: es necesaria para elegir la frecuencia. Las notificaciones de alimentación pueden adelantar una lectura.

## Exactitud y límites

CPU se calcula con diferencias entre contadores globales y se normaliza entre 0 y 100 %. RAM incluye memoria comprimida y excluye caché recuperable, siguiendo la interpretación habitual de memoria usada. Los tamaños del panel se expresan en GB decimales.

El disco corresponde al espacio disponible en el volumen de datos APFS. Las instantáneas y el espacio purgable pueden producir diferencias con Finder. La red suma interfaces físicas `en*` activas: evita contar dos veces túneles VPN, loopback y AWDL; no pretende medir tráfico exclusivo entre interfaces virtuales.

La GPU consulta `PerformanceStatistics` de `IOAccelerator`. Estas propiedades no son un contrato público estable de Apple; si no existen se muestra `—`, con reintento espaciado, en lugar de inventar un 0 %. Se ha comprobado la lectura en el Mac de desarrollo, no en cada generación de Apple Silicon.

Los tiempos de batería son estimaciones de macOS. Durante una transición, carga optimizada o ausencia de datos se muestra el estado correspondiente. No se extrapola un tiempo artificial. La primera lectura de CPU/red y la primera tras despertar necesitan establecer una base.

## Verificación y rendimiento

```sh
./scripts/check.sh
# Muestreador sin interfaz: no confundir sus cifras con las de la app completa
cargo run --release -- --diagnose --samples 30
# App completa en ejecución; sustituye PID por su identificador
python3 scripts/benchmark.py PID --seconds 60
```

`benchmark.py` usa `proc_pid_rusage` y registra CPU respecto a un núcleo, huella física, RSS, despertares y E/S. La huella física y RSS **no son la misma medida**: RSS incluye páginas de bibliotecas compartidas. Para comparar con Stats hay que usar la misma máquina, métricas activas, intervalo, alimentación y carga; mantener los paneles cerrados y medir ambas apps por separado.

La medición preliminar de 30 s de la app anterior al rediseño compacto, con los seis indicadores a 2 s y paneles sin abrir, se conserva en [`docs/benchmark-idle.json`](docs/benchmark-idle.json): 0,0476 % de un núcleo, huella física de 16,91 MiB, RSS de 73,58 MiB y 0 bytes de E/S. Es una observación local breve, **no una garantía ni una comparación con Stats**. Las mediciones posteriores identifican sus condiciones en [`docs/verification.md`](docs/verification.md).

En una medición posterior de 30 s con los seis módulos activos, AMStats usó **34,64 MiB de huella física** y Stats 3.0.25 **172,22 MiB** (aproximadamente un 80 % menos). Es una comparación puntual con las configuraciones instaladas; no se igualaron todos los intervalos ni las condiciones de arranque. Los datos y límites están en [`docs/verification.md`](docs/verification.md).

Las pruebas cubren diferencias y desbordamiento de CPU, reinicio y aparición de interfaces de red, unidades, estados de batería, política de ahorro, desactivación de colectores y anchura real de texto para evitar recortes incluso con 100 %. La comprobación visual incluye ajustes, resumen, activación/desactivación e inicio automático (registro y anulación).

## Desarrollo y versiones

Consulta [CONTRIBUTING.md](CONTRIBUTING.md) para preparar el entorno y [CHANGELOG.md](CHANGELOG.md) para el historial. GitHub Actions comprueba formato, Clippy, pruebas y empaquetado ARM64 en cada cambio de `main` y cada pull request.

## Código

| Archivo | Responsabilidad |
| --- | --- |
| `src/platform.rs` | Frontera FFI, RAII y lecturas nativas |
| `src/metrics.rs` | Cálculos, planificación, pausa, worker y diagnóstico |
| `src/config.rs` | Preferencias y política de muestreo |
| `src/ui.rs` | AppKit, barra compacta, paneles y ServiceManagement |
| `scripts/bundle.sh` | Empaquetado y firma |
| `scripts/benchmark.py` | Medición externa reproducible |

Referencia de producto: [Stats de exelban](https://github.com/exelban/stats). Implementación independiente del conjunto de métricas solicitado. Se consultaron sus [lecturas de RAM](https://github.com/exelban/stats/blob/master/Modules/RAM/readers.swift), [GPU](https://github.com/exelban/stats/blob/master/Modules/GPU/reader.swift) y [batería](https://github.com/exelban/stats/blob/master/Modules/Battery/readers.swift), además del SDK local de Apple y la [documentación de objc2](https://docs.rs/objc2/latest/objc2/).

Licencia MIT.
