# Verificación 0.3.0 — 2026-11-06

Entorno: Mac Apple Silicon, macOS 27.2. Seis indicadores activos, intervalo de 5 s, alimentación externa, ahorro en batería activado. Aplicación nativa release, firma ad hoc y Sparkle 2.10.1.

## Comprobaciones

- Formato, Clippy con advertencias como errores y 16 pruebas correctas.
- Bundle ARM64 y firma anidada verificados por el empaquetador; ZIP y appcast Ed25519 verificados con la clave pública existente.
- Inspección visual del detalle fijado y del desplegable: tarjetas, matriz LED y red con subida morada desde el centro hacia arriba y descarga cian hacia abajo.
- Agrupación por aplicación y desglose hasta la ficha de PID, memoria y ruta.
- Lecturas reales de procesos de red en orden descendente; lectura nativa con PTY para evitar acumulación de muestras en buffers de salida.
- Cargador de 60 W declarados; batería llena a +0,0 W, 126 ciclos y salud estimada del 88 %. Son lecturas del equipo de prueba, no resultados garantizados para otros modelos.
- Detección de indicador por posición cubierta por prueba, incluyendo las 63 combinaciones no vacías de indicadores visibles. Las coordenadas se convierten a la vista de dibujo para respetar la línea superior/inferior independientemente del botón nativo. El gráfico comparte escala entre subida y descarga.
- El lector nettop desaparece al cerrar el desplegable o cambiar de recurso.
- La ficha modal libera el estado de AppKit antes de entrar al bucle modal, para no bloquear las notificaciones del muestreador.

## Rendimiento local

Prueba de 30 s con panel de red fijado visible, después del calentamiento del lector:

| Proceso | CPU de un núcleo | Huella física | E/S durante la medición |
| --- | ---: | ---: | ---: |
| AMStats | 0,0306 % | 46,91 MiB | 0 bytes |
| nettop | 0,0089 % | 2,69 MiB | 0 bytes |

Datos: `benchmark-0.3.0-network.json` y `benchmark-0.3.0-nettop.json`. Esta medición precede a la corrección de la ficha modal (sin cambios en el muestreo ni dibujo). No hubo interacción durante la ventana medida. Suma observada de CPU: 0,0395 % de un núcleo. No es una garantía ni una comparación controlada con otras apps. RSS y huella física son medidas distintas.

Prueba adicional de 30 s con paneles cerrados, tras abrir detalles: 0,0698 % de un núcleo, 39,20 MiB de huella física, 0,7 despertares/s, 36.864 bytes leídos y 0 escritos. No quedaba ningún proceso nettop. Datos en `benchmark-0.3.0-idle.json`. La diferencia frente a la prueba de red refleja ventanas breves y distinto estado de cachés/carga; no permite inferir que abrir el panel reduzca consumo. Ambas mediciones preceden al ajuste final de conversión de coordenadas, que solo afecta a los clics.

## Límites

GPU y energía por proceso dependen de lo que publique macOS. Los procesos protegidos pueden no exponer datos. Potencia declarada del cargador, potencia de batería y energía atribuida a procesos son medidas distintas; no se mide físicamente el enchufe. No se ha validado todo el hardware Apple Silicon ni un ciclo físico completo de descarga/reposo.
