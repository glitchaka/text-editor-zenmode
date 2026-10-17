# Zen Editor

**Versión actual: 0.2.21.**

**Zen Editor** es un editor de escritura para Windows construido sobre **Helix 25.07.1**. Helix sigue siendo el motor real de edición dentro de una PTY; la interfaz exterior está escrita en Rust con Slint/FemtoVG y añade escritura sin distracciones, formato enriquecido no destructivo, corrector es-CL, proyectos, Pomodoro, importación y exportación.

Zen Editor existe sobre el trabajo del proyecto **Helix** y de sus contribuidores. Gracias al equipo de Helix por construir y mantener el motor de edición que hace posible este proyecto. Zen Editor no es un producto oficial ni afiliado al proyecto Helix.

## Créditos y licencia

- **Helix Editor** — motor de edición integrado: https://github.com/helix-editor/helix
- **Licencia de Helix:** Mozilla Public License 2.0.
- La licencia completa está en [`LICENSE`](LICENSE) y la información de terceros en [`THIRD_PARTY.md`](THIRD_PARTY.md) y [`CREDITS.md`](CREDITS.md).

## Documento nativo

El formato de trabajo por defecto es **TXT**. El archivo `.txt` contiene solamente el texto que se puede abrir con cualquier editor. Zen Editor guarda metadatos y formato enriquecido en un sidecar hermano:

```text
Capítulo 1.txt
Capítulo 1.txt.zen.json
```

El sidecar no aparece en el launcher. Guarda identidad, proyecto/capítulo, perfil de página y rangos de formato. Si se abre un TXT sin sidecar, Zen Editor lo trata como texto normal y crea el sidecar cuando necesita persistir metadatos o formato.

Los contenedores `.hsst` anteriores continúan siendo compatibles. Un `.hsst` moderno es un ZIP con `manifest.json`, `content.txt`, `formatting.json` e `history.jsonl`; se mantiene como formato legado compatible, no como el formato nuevo por defecto.

## Edición

Helix conserva modos NORMAL/INSERT/SELECT, undo/redo, selección, movimientos y edición real. La presentación usa **soft-wrap no destructivo**: el ancho de ventana nunca inserta saltos físicos en el texto. Los saltos de párrafo escritos o pegados se conservan literalmente.

La selección se comporta como en un editor convencional: al escribir sobre una selección, el texto seleccionado se reemplaza; Retroceso o Suprimir eliminan la selección completa; pegar reemplaza la selección activa.

El ratón se puentea a la PTY para selección y scroll. El área cliente no ocupada por una selección de texto permanece como superficie de arrastre de la ventana. La ventana es `no-frame` y el redondeo se solicita al DWM de Windows mediante `DWMWA_WINDOW_CORNER_PREFERENCE`; no se usa una máscara para simular esquinas.

## F2: corrección, formato y página

`F2` es el único acceso al formato. La isla inferior no contiene botones B/I/U ni paletas de color.

Sobre una selección, F2 ofrece:

- correcciones ortográficas y aceptar palabra en diccionario personal;
- negrita, cursiva, subrayado y tachado;
- color de texto y color de fondo/resaltado;
- alineación izquierda, centrada, derecha y justificada.

También desde F2 se configuran papel, orientación y márgenes. El formato se conserva fuera del texto limpio y el renderer de Zen Editor lo pinta directamente sobre las celdas visibles de Helix.

## Launcher y proyectos

Al iniciar sin archivo se abre la biblioteca `Documentos`. Los documentos pueden tener `project`, `chapter` y `order` sin requerir carpetas separadas. `P` asigna proyecto y número de capítulo al documento seleccionado. Al llegar al principio/final de un capítulo, la navegación puede continuar al capítulo anterior/siguiente del mismo proyecto.

Controles principales del launcher:

| Tecla | Acción |
| --- | --- |
| `↑ / ↓` | mover selección |
| `Enter` | abrir archivo/directorio |
| `N` | crear documento TXT nativo |
| `I` | importar TXT, Markdown o DOCX |
| `P` | asignar proyecto/capítulo |
| `E` | exportar documento o proyecto |
| `Z` | alternar selección de Zenmode |
| `R` | refrescar |
| `Backspace` | subir al directorio padre |
| `Esc` | cerrar |

## Importación

`I` importa TXT, Markdown o DOCX al modelo nativo TXT + sidecar. DOCX conserva, cuando están presentes, negrita, cursiva, subrayado, tachado, color, resaltado y alineación de párrafo.

## Exportación

`E` exporta un documento o un proyecto completo a:

- **TXT**: texto limpio;
- **DOCX**: Word/OpenXML sin depender de Microsoft Word;
- **PDF**: salida paginada.

DOCX y PDF usan el perfil de papel, orientación y márgenes del documento y reconstruyen formato enriquecido y alineación de párrafo.

## Isla inferior y Pomodoro

La isla inferior se mantiene compacta y reserva su propia franja para no cubrir la línea de estado: **contador de palabras · Pomodoro · Ω símbolos · maximizar/restaurar · cerrar**. F4 o `:PD` inicia el ciclo de 25 minutos de escritura y 5 de descanso. Durante el descanso se bloquea la escritura. El menú Ω inserta signos tipográficos; `Alt+D`, `Ctrl+G` y `AltGr+-` permiten insertar raya `—`.

F11/Zen maximiza la ventana dentro del área de trabajo de Windows en vez de usar fullscreen exclusivo, por lo que la barra de tareas permanece disponible. Al maximizar, restaurar o redimensionar, la PTY se recalcula y Helix se repinta para ajustar el contenido al nuevo ancho.

## Editor: atajos principales

| Tecla | Acción |
| --- | --- |
| `Z` en NORMAL | entrar/salir de Zenmode |
| `Shift+← / →` | ampliar selección por carácter |
| `Shift+↑ / ↓` | ampliar selección por líneas |
| `Ctrl+← / →` | mover por palabras |
| `Ctrl+Shift+← / →` | ampliar selección por palabras |
| `Ctrl+Backspace` | borrar palabra anterior |
| `Ctrl+Supr` | borrar palabra siguiente |
| `Ctrl+Z` | deshacer |
| `Ctrl+Y` / `Ctrl+Shift+Z` | rehacer |
| `Ctrl+A` | seleccionar todo |
| `Ctrl+C` / `Ctrl+X` | copiar / cortar selección |
| `Ctrl+V` | pegar reemplazando la selección si existe |
| `Ctrl+T` | abrir documento en una nueva pestaña |
| `Ctrl+Tab` / `Ctrl+Shift+Tab` | cambiar de pestaña |
| `Ctrl+W` | cerrar pestaña actual |
| `Ctrl+Shift+T` | reabrir última pestaña cerrada |
| `Ctrl+S` | guardar |
| `F2` | acciones de corrección/formato/página |
| `F4` | Pomodoro |
| `F11` | maximizar/restaurar Zenmode |

## Abrir directamente

Arrastrar un archivo sobre `zen-editor.exe` hace que Windows entregue su ruta como argumento y Zen Editor lo abre directamente, sin pasar por el launcher. También se puede usar:

```powershell
zen-editor "C:\ruta\archivo.txt"
zen-editor --zen "C:\ruta\archivo.txt"
```

## Compilación

Objetivo principal: **Windows x86_64 MSVC**.

```powershell
cargo build --release
```

El binario resultante es `zen-editor.exe`. GitHub Actions ejecuta `rustfmt`, Clippy con warnings como errores, tests, build release y publica el artefacto `zen-editor-windows-x86_64`.

`build.rs` obtiene cuando hace falta Helix 25.07.1 para Windows, JetBrainsMono Nerd Font y el diccionario es-CL. Se pueden proporcionar recursos locales con `HELIX_SST_FONT_FILE` y `HELIX_SST_ARCHIVE`.

## Estructura

```text
src/app.rs       ventana, renderer, launcher, Pomodoro y puente de entrada
src/editor.rs    Helix embebido, PTY, keymap, pegado y sincronización
src/document.rs  TXT/sidecar, compatibilidad HSST, importación y metadatos
src/format.rs    estilos y formato enriquecido
src/page.rs      papel, orientación y márgenes
src/spell.rs     LSP ortográfico y acciones F2
src/export.rs    TXT, DOCX y PDF
src/library.rs   biblioteca Documentos y exports
```

La información de terceros se encuentra en `THIRD_PARTY.md` y `licenses/`.


### Workspace WBW integrado

Desde el navegador, `W` abre el workspace del proyecto seleccionado. Incluye proyecto, manuscrito, escenas, personajes, relaciones, teorías, cronología, atlas, mapas, magia, referencias, documentación/notas, revisión, maquetación, multimedia, configuración y estadísticas. `N` crea registros, `Enter` abre/edita, `R` renombra, `D` elimina, `Ctrl+↑/↓` reordena, `A` adjunta multimedia, `B` exporta `.wbw` e `I` importa `.wbw`.
