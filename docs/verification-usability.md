# Revisión de usabilidad — 2026-11-07

Mejoras de navegación incluidas en la versión 0.4.0. La validación visual se realizó sobre la compilación previa al incremento de versión.

## Criterios

Las actualizaciones deben poder buscarse desde la vista que ya está usando la persona, sin pasar por información del autor. Las preferencias de actualización pertenecen a Ajustes. Cada ventana auxiliar debe tener una función clara, y la navegación entre ellas debe evitar que se acumulen. Los textos deben describir la acción que ejecuta cada botón.

## Resultado

- Ajustes tiene pestañas nativas General y Actualizaciones. Se conservan los indicadores, vista previa, frecuencia, ahorro e inicio automático. Actualizaciones reúne versión, búsqueda y preferencias de Sparkle; la instalación automática sigue dependiendo de la búsqueda automática.
- Buscar actualizaciones está disponible en cada detalle y en Resumen, además del menú y ⌘U. Acerca de enlaza directamente a la pestaña correspondiente.
- El menú separa navegación/configuración, los seis detalles y la información/salida. Se conservan los atajos.
- Ajustes, Resumen y Acerca de ocultan la ventana auxiliar anterior. Resumen activa la aplicación. La búsqueda cierra el detalle para dejar visible la respuesta de Sparkle.
- Separar y A la barra describen el cambio entre desplegable y ventana, con ayuda contextual. La navegación tiene una fila propia bajo el estado completo; el panel crece 32 pt.
- El error de actualizador no disponible se muestra fuera del préstamo mutable de App, para no bloquear callbacks durante el modal.

## Verificación

- Formato, Clippy y 16 pruebas correctos. Compilación ARM64 y firma anidada verificadas por el empaquetador.
- Inspección visual de General y Actualizaciones: textos legibles y controles sin solapamientos.
- Búsqueda real desde Ajustes, Resumen y detalle: Sparkle muestra «Estás al día» para la versión 0.3.2.
- Navegación por ⌘R, ⌘1 y ⌘, comprobada. Menú principal y enlace Acerca de → Actualizaciones comprobados.
- Separar abre Detalle de AMStats con el botón A la barra; este recupera el desplegable anclado. Panel final inspeccionado en ventana, con estado y acciones en filas distintas y sin recortes.
- La automatización no expone los extras de la barra: el recorrido por clic derecho no se ha vuelto a comprobar visualmente. No se ha probado una instalación de una versión superior.

El paquete local se genera en `dist/AMStats.app`. No se añaden temporizadores ni cambios al muestreo.

## Paquete 0.4.0

Formato, Clippy, 16 pruebas y empaquetado ARM64 correctos tras incrementar la versión. Firma anidada del bundle y firmas Ed25519 del ZIP y appcast verificadas con la clave existente. La versión y el ejecutable dentro del ZIP coinciden con el paquete compilado. Firma de aplicación ad hoc, sin notarización.
