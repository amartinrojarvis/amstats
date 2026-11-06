# Verificación de AMStats 0.2.0 — 6 de noviembre de 2026

- Formato, Clippy con advertencias tratadas como errores, diez pruebas y validación del plist: correctos.
- GitHub Actions sobre `7ce0de1`: correcto, incluido el empaquetado ARM64.
- Bundle final de aproximadamente 3,3 MiB; ejecutable de 420.000 bytes. Incluye Sparkle 2.10.1, cuya distribución se verifica con SHA-256 antes de empaquetar.
- Firma ad hoc de los componentes y del bundle verificada con `codesign --verify --deep --strict`. No hay notarización Apple.
- ZIP y appcast firmados con la clave Ed25519 de AMStats y verificados mediante `sign_update --verify`. La clave privada se conserva en el Llavero; únicamente la pública se incluye en el repositorio.
- Ventana «Acerca de» revisada en macOS: versión 0.2.0, Alberto Martín, X e Instagram @amartinro, GitHub y controles de actualización legibles y sin recortes.
- Búsqueda automática activada y descarga/instalación automática desactivada inicialmente. La búsqueda manual comunica un error recuperable cuando el feed no está disponible y «Estás al día» cuando el feed público anuncia la versión instalada.

## Actualización completa

Se creó una copia del bundle en `target/update-test/AMStats.app`, modificando sus campos de versión a **0.1.9** y volviéndola a firmar. Es una versión anterior simulada del bundle nuevo, no la antigua 0.1.0, que carece de Sparkle.

Desde esa copia, «Buscar actualizaciones» consultó el feed HTTPS público de GitHub y mostró **0.2.0 disponible (tienes 0.1.9)** con sus novedades. Al pulsar «Instalar actualización», Sparkle descargó la release, sustituyó el bundle y reinició AMStats. Después:

- El bundle de prueba tenía `CFBundleVersion = 0.2.0`.
- Su ejecutable tenía exactamente el mismo SHA-256 que el paquete publicado.
- La firma profunda del bundle actualizado era válida.
- Una segunda búsqueda mostró «AMStats 0.2.0 es la versión más nueva disponible».

Se volvió a abrir la app de producción en `dist/AMStats.app`, dejando visible «Acerca de». No se modificaron los indicadores, el intervalo de muestreo ni la preferencia existente de inicio de sesión.

## Límites

No se esperaron 24 horas para observar la comprobación programada ni se ensayó la instalación desatendida. La prueba completa usó la búsqueda e instalación iniciadas desde la interfaz. No se probaron permisos de administrador en `/Applications`, otro Mac, reposo prolongado ni un archivo malicioso. Las cifras de memoria anteriores del repositorio corresponden a 0.1.0 y no se han repetido con Sparkle.

Release pública: [v0.2.0](https://github.com/amartinrojarvis/amstats/releases/tag/v0.2.0). Referencias de integración: [Sparkle](https://sparkle-project.org/documentation/) y [publicación y firmas](https://sparkle-project.org/documentation/publishing/).
