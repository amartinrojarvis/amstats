# Verificación 0.3.1 — 2026-11-06

Corrige los problemas comunicados tras 0.3.0: selección incorrecta desde el bloque de la barra, apertura de ventana separada por atajos y resaltado de todo el bloque.

- Cada indicador utiliza su propio NSStatusItem y NSStatusBarButton con tag estable 0–5; el selector ya no calcula el recurso a partir de las coordenadas de currentEvent.
- Prueba de tags con las 63 combinaciones no vacías de indicadores visibles. Prueba de tamaño: anchos nativos solicitados suman 206 pt, dentro de los 212 pt anteriores. Se mantienen las fuentes y las dos líneas; sin gráficos nuevos en la barra.
- El resaltado de las celdas está desactivado. Cada indicador se redibuja solo cuando cambia su texto; disposición y visibilidad se actualizan solo cuando cambia el conjunto de indicadores.
- Inspección visual de CPU y batería mediante atajos: ambos son NSPopover (ventana emergente de accesibilidad), con flecha de anclaje y botón Fijar. El acceso por menú ya no fija automáticamente. La herramienta de inspección no expone los clics en los extras de la barra; la validación visual de esos clics se ha solicitado al usuario.
- Formato, Clippy, 16 pruebas, empaquetado ARM64 y firma anidada correctos.

## Medición

Mismo equipo macOS 27.2 ARM64, seis indicadores, intervalo 5 s, corriente externa. Tras abrir CPU y batería y cambiar a Finder para cerrar el desplegable: 30 s, CPU 0,0188 % de un núcleo, huella física 47,52 MiB (máximo 48,78), 0,133 despertares/s, 0 bytes de E/S. Archivo `benchmark-0.3.1-idle.json`. Hubo una compilación independiente durante parte de la ventana; no es una comparación controlada ni una garantía. La medición precede a un ajuste del anclaje al desfijar un recurso oculto y a extraer la función de ancho, sin cambios de muestreo.
