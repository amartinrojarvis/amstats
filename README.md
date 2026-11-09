# AMStats

Monitor de barra de menús para **macOS 13+ y Apple Silicon**, escrito en Rust con interfaz AppKit. CPU, RAM, GPU, almacenamiento, subida/bajada de red y batería con tiempo estimado de carga o autonomía.

La barra mantiene el formato compacto de dos líneas y un presupuesto de **191 pt**. Cada indicador tiene un botón nativo independiente; sus anchos solicitados suman 185 pt con los seis visibles. Los valores usan cifras de ancho fijo y porcentajes pequeños. Los indicadores ocultos desaparecen también del muestreo. Un clic abre el detalle; desde Ajustes se puede abrir además un resumen en ventana.

## Abrir e instalar

Descarga la aplicación ARM64 y su checksum en [Releases](https://github.com/amartinrojarvis/amstats/releases/latest). El repositorio y sus descargas son públicos. El ZIP contiene `AMStats.app`. Si compilas localmente, se genera en `dist/AMStats.app`; los binarios no se guardan en Git.

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
- **↑ / ↓:** subida en la línea superior y descarga en la inferior. En la barra, `K`, `M` y `G` equivalen a KB/s, MB/s y GB/s decimales. El panel muestra las unidades completas.
- **Batería:** silueta con relleno proporcional y porcentaje integrado sin `%`; un rayo indica que está cargando. Encima aparece el tiempo estimado (`45m`, `2h15`): autonomía al descargar o tiempo para completar la carga. Cuando no hay un tiempo estimado, muestra `AC` con alimentación externa (carga completa o en pausa), `CARGA` durante la carga y `BAT` al usar batería. El panel conserva la explicación del estado.
- **Detalle contextual:** pulsa CPU, RAM, GPU, SSD, red o batería en la barra para abrir ese recurso. En red puedes elegir el orden por subida o descarga desde el selector del panel. Gráficos LED con 60 segundos de historial; subida desde el centro hacia arriba y descarga hacia abajo, con escala compartida.
- **Procesos:** hasta diez entradas ordenadas de mayor a menor, agrupación por aplicación, desglose de sus procesos, PID y memoria. Puedes elegir el sentido de E/S, pausar y fijar el panel como ventana. «Separar» abre una ventana independiente y «A la barra» recupera el desplegable; al pulsar un indicador se vuelve al desplegable anclado a él. La lista mantiene el orden mientras el cursor está encima.
- **Cargador:** potencia declarada en W, potencia de carga/descarga de batería, salud estimada, ciclos y capacidad. La potencia del adaptador no es el consumo medido en el enchufe.
- **Ajustes:** pestañas General y Actualizaciones. En General puedes activar indicadores, seleccionar 1/2/5 segundos, reducir frecuencia en batería y abrir AMStats al iniciar sesión. La vista previa reproduce la barra real.
- **Acerca de:** versión instalada, Alberto Martín, enlaces a [X](https://x.com/amartinro) e [Instagram](https://www.instagram.com/amartinro/) (@amartinro), y acceso a la pestaña de actualizaciones.
- **Atajos con la app activa:** `⌘1` a `⌘6` desplegable de cada recurso, `⌘,` ajustes, `⌘R` resumen, `⌘U` buscar actualizaciones y `⌘Q` salir.
- Al desactivar todos los indicadores permanece un pequeño acceso **AM** para volver a los ajustes.

Las preferencias se guardan con `NSUserDefaults`, dominio `dev.amartin.amstats`. El inicio automático se consulta y modifica a través de `SMAppService`; si macOS requiere aprobación se muestra su estado y un acceso a Ítems de inicio. No se activa por defecto.

## Actualizaciones desde la app

Pulsa **Buscar actualizaciones…** en el panel de cualquier recurso, en Resumen o en el menú que aparece al hacer clic derecho en un indicador. Con la app activa también puedes usar **⌘U**. En **Ajustes → Actualizaciones** encontrarás la versión instalada, la búsqueda manual y las preferencias automáticas. Sparkle compara la versión instalada con la última release estable, muestra las novedades, verifica la firma y permite instalar y reiniciar la app. La comprobación automática está activada cada 24 horas; puedes desactivarla. La descarga e instalación automáticas son opcionales y están desactivadas inicialmente.

El feed y los ZIP usan HTTPS y firmas Ed25519. La clave privada permanece en el Llavero del mantenedor. No se envía el perfil del sistema a Sparkle. La firma de actualización no sustituye la notarización de Apple: los paquetes actuales usan firma ad hoc. Instala la app en una carpeta de Aplicaciones con permisos de escritura; macOS puede solicitar autorización para sustituirla.

**Si tienes 0.1.0**, instala 0.2.0 manualmente una vez: aquella versión todavía no incluía actualizador. Las siguientes versiones se podrán instalar desde AMStats.

## Por qué consume poco

- Un hilo de muestreo de prioridad Utility y el hilo principal de AppKit. Sin Electron, WebView, Tokio ni servidor. El detalle crea un segundo hilo Utility al abrirse por primera vez; queda dormido al cerrar o pausar el panel.
- Los procesos se consultan solo con el detalle visible, como mínimo cada 2 segundos y respetando el intervalo y ahorro configurados. El detalle de red usa un único lector nativo `nettop`, sin DNS, que termina al cambiar de recurso, cerrar o pausar.
- Matriz LED dibujada con tres trazados, sin animaciones, temporizadores ni vistas por punto. Historial de 60 segundos limitado a 61 muestras en RAM; no se redibuja dos veces la misma muestra.
- CPU y RAM mediante Mach; GPU y alimentación mediante IOKit; red mediante contadores de 64 bits del kernel; espacio mediante `statfs`.
- Un solo estado pendiente para la interfaz: las actualizaciones no se acumulan. La barra se redibuja únicamente cuando cambia el contenido mostrado.
- Los paneles se construyen al abrirlos y solo actualizan sus vistas cuando están visibles.
- Disco y batería cada 30 segundos; cambios de alimentación también por notificaciones. El modo de ahorro duplica el intervalo al funcionar con batería.
- Sin muestreo periódico con todos los indicadores apagados. Pausa durante reposo del sistema o de las pantallas; reinicia las bases de CPU/red al despertar para evitar picos falsos.
- Sin historial persistente ni telemetría. Sparkle consulta el feed de GitHub una vez al día si la búsqueda automática está activada, y descarga las actualizaciones cuando corresponde. Contadores, buffers y conexiones a servicios se reutilizan; los recursos nativos se liberan con RAII.
- El ejecutable de métricas sigue siendo Rust nativo. Desde 0.2.0, la app incluye Sparkle y sus auxiliares de instalación; las cifras de tamaño y memoria de 0.1.0 no representan esta versión.

Si está activado el ahorro en batería, se consulta alimentación aunque su indicador esté oculto: es necesaria para elegir la frecuencia. Las notificaciones de alimentación pueden adelantar una lectura.

## Exactitud y límites

CPU se calcula con diferencias entre contadores globales y se normaliza entre 0 y 100 %. RAM incluye memoria comprimida y excluye caché recuperable, siguiendo la interpretación habitual de memoria usada. Los tamaños del panel se expresan en GB decimales.

El disco corresponde al espacio disponible en el volumen de datos APFS. Las instantáneas y el espacio purgable pueden producir diferencias con Finder. La red suma interfaces físicas `en*` activas: evita contar dos veces túneles VPN, loopback y AWDL; no pretende medir tráfico exclusivo entre interfaces virtuales.

La autonomía utiliza `IOPSGetTimeRemainingEstimate` de macOS (segundos convertidos a minutos); el tiempo de carga procede de `Time to Full Charge` de IOKit. Se descartan estados desconocidos, valores no finitos y el centinela `65535`, y no se reutiliza un tiempo de descarga durante la carga o viceversa. Se mantiene la frecuencia de lectura. Son estimaciones del sistema: su exactitud depende del consumo y de la curva de carga; este cambio no garantiza un menor error en minutos frente a una descarga real.

La GPU consulta `PerformanceStatistics` de `IOAccelerator`. Estas propiedades no son un contrato público estable de Apple; si no existen se muestra `—`, con reintento espaciado, en lugar de inventar un 0 %. Se ha comprobado la lectura en el Mac de desarrollo, no en cada generación de Apple Silicon.

Los tiempos de batería son estimaciones de macOS. Durante una transición, carga optimizada o ausencia de datos se muestra el estado correspondiente. No se extrapola un tiempo artificial. La primera lectura de CPU/red y la primera tras despertar necesitan establecer una base.

El detalle usa contadores de procesos accesibles al usuario; macOS puede restringir procesos del sistema. CPU se normaliza a la capacidad completa del Mac. RAM muestra huella física; las cifras por proceso no son una partición exacta de la memoria global. GPU usa tiempo por cliente en ms/s, puede solaparse y depende de propiedades del controlador. La energía por proceso es una atribución del kernel, no un vatímetro físico. Cuando faltan datos se indica expresamente.

La red por proceso mide TCP/UDP y puede incluir tráfico local o de VPN que no coincide con el total de interfaces físicas. Necesita varias lecturas al abrir el panel (aproximadamente tres intervalos).

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

La versión 0.3.0 añade comprobaciones de agrupación, contadores, historial acotado y selección con indicadores ocultos. Las pruebas visuales y mediciones de sus paneles se documentan en [verification-0.3.0.md](docs/verification-0.3.0.md).

La versión 0.3.2 corrige las acciones del menú contextual de los indicadores, incluida la apertura de Ajustes. Causa y validación: [verification-0.3.2.md](docs/verification-0.3.2.md).

## Desarrollo y versiones

Consulta [CONTRIBUTING.md](CONTRIBUTING.md) para preparar el entorno y [CHANGELOG.md](CHANGELOG.md) para el historial. GitHub Actions comprueba formato, Clippy, pruebas y empaquetado ARM64 en cada cambio de `main` y cada pull request.

## Código

| Archivo | Responsabilidad |
| --- | --- |
| `src/platform.rs` | Frontera FFI, RAII y lecturas nativas |
| `src/metrics.rs` | Cálculos, planificación, pausa, worker y diagnóstico |
| `src/config.rs` | Preferencias y política de muestreo |
| `src/updates.rs` | Integración nativa con Sparkle |
| `scripts/release.sh` | Preparación y firma de versiones estables |
| `src/processes.rs` | Lectores de procesos bajo demanda y agrupación |
| `src/ui/detail.rs` | Detalle contextual y gráficos LED |
| `src/ui.rs` | AppKit, barra compacta, paneles y ServiceManagement |
| `scripts/bundle.sh` | Empaquetado y firma |
| `scripts/benchmark.py` | Medición externa reproducible |

Referencia de producto: [Stats de exelban](https://github.com/exelban/stats). Implementación independiente del conjunto de métricas solicitado. Se consultaron sus [lecturas de RAM](https://github.com/exelban/stats/blob/master/Modules/RAM/readers.swift), [GPU](https://github.com/exelban/stats/blob/master/Modules/GPU/reader.swift) y [batería](https://github.com/exelban/stats/blob/master/Modules/Battery/readers.swift), además del SDK local de Apple y la [documentación de objc2](https://docs.rs/objc2/latest/objc2/).

Creado por **Alberto Martín** · [X: @amartinro](https://x.com/amartinro) · [Instagram: @amartinro](https://www.instagram.com/amartinro/).

Licencia MIT. Sparkle incluye su licencia en `AMStats.app/Contents/Resources/Sparkle-LICENSE.txt`.
