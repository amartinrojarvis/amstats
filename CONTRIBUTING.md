# Desarrollo

Se necesita un Mac Apple Silicon con macOS 13+, Rust estable y las Command Line Tools de Xcode. El objetivo es mantener una interfaz nativa y reducir trabajo periódico, asignaciones y procesos auxiliares.

```sh
git clone https://github.com/amartinrojarvis/amstats.git
cd amstats
./scripts/check.sh
./scripts/bundle.sh
```

El repositorio es privado; necesitas acceso a la cuenta o repositorio para clonarlo. El binario `--diagnose` permite revisar las lecturas sin abrir la interfaz:

```sh
cargo run --release -- --diagnose --samples 30
```

Antes de enviar un cambio, ejecuta `scripts/check.sh` y comprueba visualmente los indicadores que hayas modificado. Si afecta al muestreo o renderizado, registra una medición de la app completa con `scripts/benchmark.py`, indicando intervalo, módulos, alimentación, ventanas abiertas, equipo y duración. No compares el muestreador sin interfaz con la app completa ni confundas RSS con huella física.

Los cambios de FFI deben respetar la propiedad y liberación de objetos CF/IOKit/Mach y mantener AppKit en el hilo principal. Las métricas no disponibles deben seguir diferenciándose de un cero real. Mantén la documentación y `CHANGELOG.md` alineados con el comportamiento implementado.

La comprobación de GitHub Actions ejecuta formato, Clippy, pruebas y empaquetado en un runner macOS ARM64 estándar. Referencia de las etiquetas de runner: [GitHub Docs](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).

Los archivos de `target/` y `dist/` no se versionan. Los ejecutables se distribuyen como archivos ZIP en Releases. Las releases deben indicar si la firma es ad hoc o Developer ID y si existe notarización.
