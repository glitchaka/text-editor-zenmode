# Historial de versiones — Zen Editor

Cambios funcionales por versión. Se omiten commits internos de mantenimiento, pruebas temporales y workflows de parcheo.

## Portable scene round-trip — 2026-10-17

- Exportación/importación `.wbw` ahora conserva y restaura manuscrito, escenas TXT, sidecars de formato y multimedia con rutas portables.
- Renombrar/reordenar/editar una escena sincroniza sus metadatos nativos; eliminarla retira TXT y sidecar.
- El selector F3 de referencias semánticas permanece estable mientras está abierto y restaura el terminal antes de insertar el vínculo.

## Scene documents and semantic references — 2026-10-17

- Las escenas WBW son documentos TXT nativos reales, se abren en el editor/pestañas y sincronizan título, capítulo, estado y orden con su ficha.
- Las escenas heredadas sin archivo se migran automáticamente a TXT al abrir el proyecto.
- Manuscrito y escenas quedan separados: el índice de manuscrito no duplica documentos de tipo escena.
- F3 abre el selector de referencias semánticas del proyecto e inserta un marcador estable enlazado por ID a personajes, Atlas, magia, referencias o documentación.

## Completion pass — 2026-10-17

- Añadida Documentación/notas como módulo WBW editable.
- Tablero de escenas muestra tarjetas con capítulo, POV, ubicación y estado; mantiene creación, ficha, reordenación y edición.
- Un clic de ratón posiciona el cursor y vuelve a escritura; arrastrar conserva selección, evitando el estado residual que provocaba inserciones/repeticiones inesperadas.
- El cierre de F2 conserva el parser VT y vuelve a escritura después de aplicar el formato.
- Se mantienen soft-wrap no destructivo, esquinas DWM nativas, arrastre de superficie completa y reflow sin reiniciar el parser.

## WBW complete workspace — 2026-10-17

- Completado el workspace WBW: Proyecto, Manuscrito, Tablero de escenas, Personajes, Relaciones, Hilos/teorías, Cronología, Atlas, Mapas, Magia, Referencias, Revisión, Maquetación, Multimedia, Configuración y Estadísticas.
- Las fichas permiten editar todos sus campos escalares/listas; los registros se crean, renombran, eliminan y reordenan.
- Multimedia local puede adjuntarse desde el workspace.
- Importación/exportación `.wbw` portable con `manifest.json`, `project.json`, manuscrito y medios.
- Proyecto, maquetación y configuración son editables sin contaminar los TXT.

## WBW workspace — 2026-10-17

- Integrado workspace de Worldbuilder Writer por proyecto: manuscrito, escenas, personajes, relaciones, cronología, mundo/atlas, magia, referencias, notas y estadísticas.
- `W` abre el workspace del proyecto desde el navegador; el manuscrito abre documentos reales y las demás secciones permiten crear registros persistentes.
- La información WBW se guarda por proyecto en `.zen-project-<proyecto>.json` sin contaminar el TXT.
- Las correcciones no mandatorias del editor se movieron a `TODO.md`.
- Reparado el fallo de compilación introducido por el intento de restaurar hard-wrap físico.

## v0.2.25 — 2026-10-17

- Restaurado el ajuste físico al ancho de página para que cada línea visible tenga numeración propia.
- Corregido el regreso al navegador al cerrar el último documento: ya no queda la superficie vacía.
- El área completa vuelve a ser arrastrable al salir del editor aunque haya quedado un estado de selección transitorio.

## v0.2.24

- Corregido el estado visual del editor después de Enter/Esc: ya no se reconstruye el terminal en cada pulsación.
- El repintado de F2 conserva el parser VT para evitar restos, líneas fantasma y marcadores `~` incoherentes.
- El reajuste al maximizar/restaurar conserva el estado del terminal mientras Helix realiza el reflow.

## v0.2.23 — 2026-10-17
- Corregido Ctrl+C/Ctrl+X mediante un puente explícito al portapapeles del sistema.
- Reflow de Helix en varias pasadas tras maximizar/restaurar y cambios de tamaño de Windows.
- Limpieza diferida de superficies transitorias para evitar restos visuales de menús/completado.
- El navegador de manuscrito y la ficha literaria se posponen al TODO posterior al gran cambio.

## v0.2.22 — 2026-10-17
- Repintado F2 en dos fases para eliminar residuos visuales de menús y submenús.
- Reflow fiable al maximizar/restaurar usando el tamaño real del área cliente de Windows.
- Pestañas reales sobre los buffers de Helix: abrir, cambiar, cerrar y reabrir documentos.
- Los cambios automáticos de capítulo quedan incorporados al conjunto de pestañas abiertas.

## v0.2.21 — 2026-10-17
- La escritura reemplaza correctamente una selección activa, como en un editor convencional.
- Retroceso y Suprimir eliminan completamente el texto seleccionado.
- Pegar reemplaza la selección activa.
- El contenido se recalcula y repinta después de maximizar, restaurar o redimensionar la ventana.

## v0.2.20 — 2026-10-17
- Eliminar restos visuales de los menús F2 mediante un repintado completo de Helix.
- Añadir creación de carpetas desde el launcher.
- Añadir agrupación de documentos por proyecto mediante un manifiesto de proyecto.
- Mantener el dock visible y compacto sin cubrir la línea de estado del editor.
- Mostrar contador de palabras en el dock.
- Cambiar F11/Zen a ventana maximizada sin ocultar la barra de tareas de Windows.

## v0.2.19 — 2026-10-17
- Cierre determinista del menú F2 al confirmar una acción.
- Retorno automático a modo escritura después de aplicar formato o configuración de página.
- Eliminada la dependencia del timestamp del archivo para detectar el cierre de una acción F2.

## v0.2.18 — 2026-10-17
- Restaurado el control visible de pantalla completa y el atajo F11.
- Reordenado F2 para priorizar formato y alineación cuando existe una selección.
- Añadidos LICENSE, créditos y agradecimiento explícito a Helix.

## v0.2.17 — 2026-10-17
- Cambio definitivo del nombre del paquete y binario a Zen Editor.
- Unificación del ejecutable en `zen-editor.exe`.
- Limpieza de configuración heredada del nombre anterior.

## v0.2.16 — 2026-10-17
- Formato enriquecido visible sobre el renderer de Helix.
- Restaurado el arrastre de la ventana sin marcos artificiales.
- Restauradas las esquinas redondeadas nativas de Windows mediante DWM.
- F2 concentrado como único acceso a formato, color, resaltado y alineación.
- Isla inferior compacta con Pomodoro, símbolos y cierre.
- Autocompletado y tokens semánticos integrados con el LSP de prosa.

## v0.2.15 — 2026-10-07
- Mejorado el refresco visual del formato enriquecido.
- Ajustado el comportamiento de la isla y del cronómetro Pomodoro.

## v0.2.14 — 2026-10-07
- Correcciones al formato real de documentos nativos.
- Pomodoro retirado del launcher y concentrado en la interfaz del editor.

## v0.2.13 — 2026-10-06
- Isla de controles trasladada al borde inferior.
- Reducción de controles visibles para mantener el modo de escritura sin distracciones.

## v0.2.12 — 2026-10-05
- Nueva isla inferior compacta.
- Ajustes al formato visual y a la integración de Pomodoro.

## v0.2.11 — 2026-10-05
- Formato contextual y Pomodoro integrados en el flujo normal del editor.
- Mejorado el refresco del formato después de aplicar acciones.

## v0.2.10 — 2026-10-05
- Formato HSST/TXT nativo aplicado sin insertar marcas en el texto.
- Acciones de formato y configuración de página integradas mediante F2.
- Pomodoro implementado directamente en Rust.

## v0.2.9 — 2026-10-05
- La isla permanece visible correctamente al salir de Zenmode.
- Simplificado el flujo de formato nativo.

## v0.2.8 — 2026-10-05
- Corregida la ejecución de comandos de formato en Windows.
- Corregida la visibilidad de la isla al alternar Zenmode.

## v0.2.7 — 2026-10-05
- Corregida la selección de texto y el directorio usado por el formato.
- Corregida la entrada Win32 utilizada por los comandos de formato.

## v0.2.6 — 2026-10-05
- Separación definitiva entre contenido de texto y formato enriquecido.
- Compatibilidad con contenedores HSST compuestos.
- Biblioteca `Documentos` junto al ejecutable.
- Mejoras de interacción del editor y exportación.

## v0.2.5 — 2026-10-04
- Restaurada la numeración de líneas.
- Pegado multilinea corregido sin reescribir párrafos.
- Añadido motor de formato enriquecido y conservación en DOCX.
- Añadido perfil de página para papel, orientación y márgenes.
- Launcher ampliado para archivos generales.

## v0.2.4 — 2026-10-04
- Añadidos deshacer, rehacer y seleccionar todo.
- Añadido ajuste visual de texto sin modificar el contenido físico del documento.

## v0.2.3 — 2026-10-04
- Corregido el pegado desde modo Normal.
- Mejorado el ajuste visual del texto al área de escritura.

## v0.2.2 — 2026-10-04
- Conservación de párrafos al pegar texto desde otras aplicaciones.
- Soft-wrap no destructivo para prosa.

## v0.2.1 — 2026-10-04
- Barra de controles estabilizada fuera de Zenmode.
- Mejoras en la edición y persistencia de documentos HSST.

## v0.2.0 — 2026-10-04
- Introducidos documentos HSST, biblioteca y exportación TXT/DOCX/PDF.
- Añadidos proyectos y navegación continua entre capítulos.
- Añadidos tema de escritura, formato enriquecido y tokens semánticos.
- Exportación de proyectos completos y preservación del formato.

## v0.1.7 — 2026-10-04
- Corregido Ctrl+Supr.
- Activado el LSP ortográfico para prosa.

## v0.1.6 — 2026-10-04
- Corrector ortográfico integrado mediante LSP.
- Zenmode utilizable dentro del editor.
- Barra de controles autoocultable.
- Navegación y borrado por palabras con Ctrl.
- Soporte para archivos de prosa sin extensión.

## v0.1.5 — 2026-10-03
- Selector de Zenmode real desde la pantalla principal.
- Mejoras de foco y acceso al HWND de Windows.
- Navegación y borrado por palabras mediante Ctrl.

## v0.1.4 — 2026-10-03
- Zenmode real a pantalla completa.
- F11 añadido como acceso a pantalla completa.
- Bloqueo de foco mientras Zenmode está activo.

## v0.1.3 — 2026-10-03
- Añadidos splash inicial y barra de estado propia.
- Corregido el marco visual del launcher TUI.

## v0.1.2 — 2026-10-03
- Iconografía semigráfica retrofuturista para el launcher.
- Build automático de Windows en GitHub Actions.

## v0.1.1 — 2026-10-03
- Rediseño del launcher como TUI de consola.
- Versión e iconos mostrados correctamente en el launcher.

## v0.1.0 — 2026-10-03
- Primera versión independiente de Helix-SST Zenmode.
- Foco inicial y arranque del editor corregidos.
- Toolchain y compilación de Windows incorporados al repositorio.
- Corrector habilitado para archivos sin extensión.

## v0.0.1 — 2026-10-03
- Inicio del proyecto de editor Zen basado en Helix.
