# Verificación 0.3.2 — 2026-11-07

## Causa y corrección

El callback `toggle:` ejecutaba `popUpContextMenu:withEvent:forView:` dentro de `with_app`, manteniendo un préstamo mutable de `APP` durante el bucle de eventos anidado del menú. Las acciones seleccionadas se ejecutan antes de que esa llamada termine; su `with_app` fallaba en `try_borrow_mut` y las descartaba silenciosamente. Afectaba a Ajustes y a las demás acciones que acceden al estado de la aplicación.

La apertura del menú pasa al callback `toggle:`, fuera de cualquier préstamo de `APP`, tanto para clic derecho como para Control-clic. El clic izquierdo conserva la ruta de detalle. Se elimina la recuperación posterior de refresco que compensaba los callbacks bloqueados: ahora estos pueden ejecutarse durante el menú.

## Comprobaciones

- Formato, Clippy con advertencias como errores y 16 pruebas correctos.
- Bundle ARM64 compilado y firma anidada verificada con `codesign --verify --deep --strict`.
- Paquete 0.3.2 abierto: ventana de Ajustes visible, controles y vista previa comprobados visualmente.
- La herramienta de interfaz no expone los indicadores de la barra; no se ha validado visualmente el recorrido completo de clic derecho/Control-clic y selección de Ajustes.
- ZIP y appcast firmados con la clave Ed25519 existente y firmas verificadas. Versión y ejecutable dentro del ZIP coinciden con el bundle compilado.
- Firma de aplicación ad hoc, sin notarización. Sin cambios en el protocolo ni en la configuración de Sparkle. La actualización completa desde 0.3.1 no se ha probado en esta verificación.

No hay cambios en muestreo, frecuencia ni renderizado periódico; no se ha repetido la medición de consumo.
