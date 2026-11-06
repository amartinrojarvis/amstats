# Desarrollo

Se necesita un Mac Apple Silicon con macOS 13+, Rust estable y las Command Line Tools de Xcode. El objetivo es mantener una interfaz nativa y reducir trabajo periódico, asignaciones y procesos auxiliares.

```sh
git clone https://github.com/amartinrojarvis/amstats.git
cd amstats
./scripts/check.sh
./scripts/bundle.sh
```

El binario `--diagnose` permite revisar las lecturas sin abrir la interfaz:

```sh
cargo run --release -- --diagnose --samples 30
```

Antes de enviar un cambio, ejecuta `scripts/check.sh` y comprueba visualmente los indicadores que hayas modificado. Si afecta al muestreo o renderizado, registra una medición de la app completa con `scripts/benchmark.py`, indicando intervalo, módulos, alimentación, ventanas abiertas, equipo y duración. No compares el muestreador sin interfaz con la app completa ni confundas RSS con huella física.

Los cambios de FFI deben respetar la propiedad y liberación de objetos CF/IOKit/Mach y mantener AppKit en el hilo principal. Las métricas no disponibles deben seguir diferenciándose de un cero real. Mantén la documentación y `CHANGELOG.md` alineados con el comportamiento implementado.

La comprobación de GitHub Actions ejecuta formato, Clippy, pruebas y empaquetado en un runner macOS ARM64 estándar. Referencia de las etiquetas de runner: [GitHub Docs](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).

Los archivos de `target/` y `dist/` no se versionan. Los ejecutables se distribuyen como archivos ZIP en Releases. Las releases deben indicar si la firma es ad hoc o Developer ID y si existe notarización.

## Publicar una versión estable

1. Incrementa `version` en `Cargo.toml`, actualiza `Cargo.lock` (`cargo check`) y añade las novedades a `CHANGELOG.md`. El empaquetador copia esa versión a los dos campos de versión del bundle.
2. Ejecuta `./scripts/release.sh`. Descarga Sparkle 2.10.1 con SHA-256 fijado, comprueba el código, empaqueta y firma el ZIP y el appcast. Necesita la clave del Llavero de macOS con cuenta `dev.amartin.amstats`; una clave nueva no puede sustituirla sin planificar la migración. No publiques ni añadas la clave privada a Git.
3. Prueba la app empaquetada y una actualización desde la versión anterior. Confirma que la firma ad hoc o Developer ID y el estado de notarización se describen correctamente. Para Developer ID, configura `SIGNING_IDENTITY` y notariza/grapa el bundle antes de ejecutar `python3 scripts/release.py`.
4. Guarda y sube los cambios. Crea una release **borrador** con etiqueta `vMAJOR.MINOR.PATCH` en ese commit y sube los tres archivos de `dist/release-MAJOR.MINOR.PATCH/`: ZIP, `appcast.xml` y `SHA256SUMS.txt`. Usa `release-notes.md` como descripción.
5. Publica el borrador como release estable y latest solo cuando todos los archivos estén presentes. El feed fijo es `https://github.com/amartinrojarvis/amstats/releases/latest/download/appcast.xml`. Una release estable sin ese archivo interrumpiría las búsquedas. Las prereleases no se sirven por este canal.

Sparkle conserva sus preferencias en NSUserDefaults. Los ejecutables lanzados con `cargo run` no llevan el framework; prueba actualizaciones con `dist/AMStats.app`. Consulta la [guía de publicación de Sparkle](https://sparkle-project.org/documentation/publishing/).
