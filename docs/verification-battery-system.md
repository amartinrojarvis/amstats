# Salud y tiempo de batería del sistema — 2026-11-09

## Fuentes y discrepancia reproducida

Equipo ARM64 con macOS 27.2. `system_profiler SPPowerDataType` informa capacidad máxima 97 %. La lectura bruta anidada en BatteryData es 3878 mAh, diseño 4382 mAh: la fórmula anterior daba 88,4984 %, mostrado como 88 %. No representan la misma métrica. La nueva consulta del informe del sistema y el panel real muestran 97 %; se conserva la capacidad física de 3878 mAh y 127 ciclos como datos distintos.

La salud se consulta con system_profiler en XML, detalle mini y timeout de 5 s, desde el hilo de detalle. Caché de 15 minutos, reintento de 60 s si no hay lectura. No se vuelve a sustituir un fallo por una salud bruta calculada. La consulta inicial del diagnóstico completo tardó aproximadamente 103 ms; la segunda muestra, sin relanzar el informe, 11 ms. Estos tiempos incluyen los demás colectores del detalle.

El código de Apple para pmset selecciona Time to Empty o Time to Full Charge de IOPSGetPowerSourceDescription. AMStats ahora usa exactamente esos campos y el estado de la misma instantánea de la batería interna. La estimación global IOPSGetTimeRemainingEstimate puede sumar otras fuentes, por lo que se ha eliminado de esta lectura.

Referencias: https://github.com/apple-oss-distributions/PowerManagement/blob/main/pmset/pmset.m y https://github.com/apple-oss-distributions/PowerManagement/blob/main/pmconfigd/BatteryTimeRemaining.m

## Validación y límites

- Formato, Clippy y 18 pruebas correctos, con estados de descarga/carga/pausa/completa y valores inválidos.
- Compilación release y firma ad hoc verificadas.
- Diagnóstico real de batería y panel AppKit: salud 97 %, coincidente con el informe del sistema.
- Estado real: alimentación de red, 100 %, carga completa. pmset: 0:00 remaining; IOPS: ambos campos 0. AMStats muestra AC y carga completa, sin presentar 0 minutos como una estimación útil.
- No se ha desconectado físicamente el cargador ni validado un tiempo no nulo durante carga o descarga. La coincidencia de esos tiempos se basa en usar la misma fuente y se prueba con valores controlados; no implica comprobar una descarga real. Las lecturas pueden variar entre instantes de consulta.

## Aplicación completa, panel de batería abierto

Medición scripts/benchmark.py, PID 16202, bundle dist/AMStats.app. Seis módulos, intervalo 5 s, ahorro en batería activo, alimentación de red, batería llena, panel de batería con procesos agrupados abierto. ARM64, macOS 27.2; modelo concreto no recogido.

15,01 s: CPU 0,0278 % de un núcleo; huella física 51,92 MiB (pico 51,94); RSS 126,17 MiB (pico 126,23); despertares de reposo 0,067/s; E/S de disco 0 bytes. Muestra breve sin comparación controlada, tomada tras la consulta inicial de salud.
