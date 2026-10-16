# Zen Editor

**Zen Editor** es un editor de escritura para Windows construido sobre **Helix 25.07.1**. Mantiene Helix como motor real de edición dentro de una PTY y añade una interfaz Rust/Slint orientada a escritura larga, formato enriquecido no destructivo, documentos `.hsst`, corrección ortográfica en español de Chile, proyectos, importación y exportación.

Versión actual: la indicada por `Cargo.toml`.

## Qué es

Zen Editor combina dos usos:

- conservar **Helix como editor completo**, capaz de abrir archivos de código y texto normales;
- añadir un modo de escritura con documentos `.hsst` capaces de guardar formato, metadatos, página y estructura de proyecto sin meter esas marcas dentro del texto visible.

La interfaz exterior está escrita en Rust con Slint 1.18.1 y FemtoVG. Helix sigue siendo responsable de la edición, movimientos, selecciones, undo/redo, modos NORMAL/INSERT/SELECT y resaltado de los archivos que abre.

## Carpeta de trabajo

Al ejecutar `zen-editor.exe` sin una ruta explícita, el programa crea y abre una carpeta `Documentos` junto al ejecutable:

```text
Helix-SST/
├── zen-editor.exe
├── Documentos/
├── exports/
├── config/
└── data/
```

La raíz puede cambiarse con:

```powershell
$env:HELIX_SST_LIBRARY = "D:\Mi biblioteca"
```

En ese caso se utilizan:

```text
D:\Mi biblioteca\Documentos\
D:\Mi biblioteca\exports\
```

El launcher enumera archivos y directorios de la carpeta actual. Un archivo normal se entrega directamente a Helix; los `.hsst` reciben el tratamiento adicional de documento nativo de Helix-SST.

`Backspace` permite subir al directorio padre.

## El formato `.hsst`

Desde la versión 2 del formato, un `.hsst` ya no es un archivo de texto con marcas incrustadas. Es un **contenedor ZIP con extensión `.hsst`**, conceptualmente parecido a un `.docx`, pero mucho más simple.

Estructura:

```text
mi-documento.hsst
├── manifest.json
├── content.txt
├── formatting.json
└── history.jsonl
```

### `content.txt`

Contiene únicamente el texto que Helix edita:

```text
El corazón descontrolado, martillando con fuerza...
```

No contiene marcas de formato como:

```text
**negrita**
__subrayado__
{{fg:red}}texto{{/fg}}
```

El texto permanece limpio, legible y recuperable independientemente de la capa de formato.

### `manifest.json`

Guarda identidad, proyecto y configuración del documento. Por ejemplo:

```json
{
  "format": 2,
  "id": "...",
  "title": "Capítulo 1",
  "project": "Puerto Ámbar",
  "type": "chapter",
  "chapter": 1,
  "order": 10,
  "language": "es-CL",
  "status": "draft",
  "page": {
    "paper": "letter",
    "orientation": "portrait",
    "margin_top_mm": 25,
    "margin_right_mm": 25,
    "margin_bottom_mm": 25,
    "margin_left_mm": 25
  }
}
```

El `id` es estable y permite reconocer el documento aunque el archivo cambie de nombre.

### `formatting.json`

Guarda rangos de formato separados de `content.txt`.

Actualmente puede representar:

- negrita;
- cursiva;
- subrayado;
- tachado;
- color de texto;
- color de destacador/fondo;
- alineación de párrafo.

Los rangos usan posiciones sobre el texto limpio. Helix no necesita insertar etiquetas visibles para conservar el formato.

La paleta incluye blanco, rojo, naranja, amarillo, verde, cian, azul, púrpura y gris.

### `history.jsonl`

Se reserva como registro de cambios del contenedor. Actualmente se crea dentro de cada `.hsst` para mantener estable la estructura del formato y permitir ampliar el historial más adelante.

### Compatibilidad con `.hsst` antiguos

Los `.hsst` anteriores basados en texto plano y marcado embebido siguen pudiendo leerse. Al guardarlos se migran al contenedor compuesto actual.

## Formato enriquecido

El formato enriquecido se aplica **solo desde `F2`** sobre una selección. No existe una segunda barra B/I/U ni controles de formato en la isla inferior.

Las acciones disponibles incluyen:

- negrita, cursiva, subrayado y tachado;
- color de texto;
- color de destacador/fondo;
- alineación de párrafo: izquierda, centrada, derecha y justificada.

El formato se guarda en `formatting.json` y nunca se inserta como marcas dentro de `content.txt`.

La isla inferior se mantiene deliberadamente compacta: **Pomodoro · Ω símbolos · cerrar**.

## Página, márgenes y salto de línea

Cada `.hsst` almacena un perfil físico de página.

| Papel | Tamaño |
| --- | --- |
| Carta | 215,9 × 279,4 mm |
| Oficio | 215,9 × 330,2 mm |
| Legal | 215,9 × 355,6 mm |
| A4 | 210 × 297 mm |
| A5 | 148 × 210 mm |

También se almacena:

- orientación vertical u horizontal;
- margen superior;
- margen derecho;
- margen inferior;
- margen izquierdo.

La presentación usa **soft-wrap no destructivo**: una línea larga se adapta al ancho visible sin insertar saltos físicos en `content.txt`. Un párrafo continúa siendo un párrafo aunque ocupe varias filas visuales. Los saltos que sí escribe el autor se conservan literalmente.

## Pegado desde Word y otros editores

El pegado normaliza finales de línea provenientes de Windows y procesadores de texto.

Se reconocen:

- `CRLF`;
- `CR`;
- `LF`;
- separadores manuales/verticales;
- `NEL`;
- `U+2028`;
- `U+2029`.

Los saltos explícitos del texto pegado se conservan. Las líneas largas se muestran mediante soft-wrap; el pegado no reescribe el texto para ajustarlo al ancho de la ventana.

## Corrector ortográfico

Helix-SST incluye un servidor LSP propio para prosa y un diccionario **es-CL**.

Funciones actuales:

- diagnósticos ortográficos inline;
- `F2` para correcciones/acciones;
- diccionario personal;
- completado basado en el documento;
- palabras de otros documentos del mismo proyecto;
- sugerencias ortográficas;
- tokens semánticos utilizados por el formato enriquecido.

Los `.hsst`, `.txt` y archivos de texto sin extensión pueden usar el lenguaje administrado `prose`. Markdown mantiene su soporte de Helix y añade el corrector de Zen Editor.

## Proyectos y capítulos

Los `.hsst` pueden pertenecer a un proyecto lógico mediante:

```text
project
type
chapter
order
```

No es obligatorio distribuir cada proyecto en carpetas separadas. Zen Editor puede ordenar capítulos por metadatos y exportar un documento individual o el proyecto completo.

## Importación

Desde el launcher, `I` importa a `.hsst`:

- TXT y texto plano;
- Markdown;
- DOCX.

La importación DOCX conserva texto y, cuando están presentes, negrita, cursiva, subrayado, tachado, color, resaltado y alineación de párrafo.

## Exportación

Desde el launcher, selecciona un documento y pulsa `E`.

Formatos disponibles:

- **TXT**: texto limpio;
- **DOCX**: Word/OpenXML generado directamente, sin requerir Microsoft Word;
- **PDF**: documento de lectura/entrega.

DOCX y PDF usan el tamaño de papel, orientación y márgenes guardados en el documento. Ambos reconstruyen el formato enriquecido; DOCX y PDF conservan también la alineación de párrafo.

Los archivos generados se escriben en:

```text
exports/
```

Para un `.hsst`, la exportación reconstruye el formato desde `content.txt` + `formatting.json`.

## Helix sigue siendo Helix

Un archivo que no sea `.hsst` no se convierte a ningún formato propietario. Se entrega directamente a Helix.

Esto permite utilizar Helix-SST también con archivos como:

```text
README.md
Cargo.toml
src/main.rs
script.py
config.json
script.sh
notas.txt
```

El resaltado de sintaxis, los modos y las capacidades propias del editor siguen dependiendo del tipo de archivo y de Helix.

## Controles

### Launcher

| Tecla | Acción |
| --- | --- |
| `↑ / ↓` | mover selección |
| `Enter` | abrir archivo o directorio |
| `N` | crear un `.hsst` nuevo |
| `I` | importar TXT, Markdown o DOCX |
| `E` | exportar |
| `Z` | alternar modo normal / Zenmode |
| `R` | refrescar directorio |
| `Backspace` | subir al directorio padre |
| `Esc` | cerrar |

### Editor

| Tecla | Acción |
| --- | --- |
| `Z` en NORMAL | entrar/salir de Zenmode |
| `Shift+← / →` | ampliar selección carácter a carácter |
| `Shift+↑ / ↓` | ampliar selección por líneas |
| `Ctrl+← / →` | mover por palabras sin seleccionar |
| `Ctrl+Shift+← / →` | ampliar selección por palabras |
| `Ctrl+Backspace` | borrar palabra anterior |
| `Ctrl+Supr` | borrar palabra siguiente |
| `Ctrl+Z` | deshacer |
| `Ctrl+Y` o `Ctrl+Shift+Z` | rehacer |
| `Ctrl+A` | seleccionar todo |
| `F2` | corrección ortográfica y formato de la selección |
| `Alt+D` | insertar raya `—` |
| `Ctrl+G` | insertar raya `—` |
| `AltGr+-` | insertar raya `—` |

La selección y navegación se implementan mediante el keymap de Helix; no existe una segunda superficie de edición encima del editor.

## Abrir un archivo directamente

En Windows se puede arrastrar un archivo sobre el ejecutable. Zen Editor recibe la ruta y lo abre directamente.

También puede hacerse desde consola:

```powershell
zen-editor "C:\ruta\archivo.txt"
zen-editor "C:\ruta\Capítulo 1.hsst"
zen-editor --zen "C:\ruta\Capítulo 1.hsst"
```

## Arquitectura

```text
Slint / FemtoVG
      │
      ▼
Zen Editor UI + launcher + perfiles de página
      │
      ▼
PTY / protocolo de terminal
      │
      ▼
Helix 25.07.1
      │
      ├── archivos normales ──► archivo original
      │
      └── .hsst ──────────────► content.txt temporal
                                  │
                                  └── sincronización con contenedor .hsst
```

Componentes principales:

```text
src/app.rs       interfaz y launcher
src/editor.rs    Helix embebido, PTY, teclado, pegado y sincronización
src/document.rs  contenedor .hsst y metadatos
src/format.rs    rangos y estilos
src/page.rs      geometría de página y márgenes
src/spell.rs     LSP/corrector ortográfico
src/export.rs    TXT, DOCX y PDF
src/library.rs   Documentos y exports
```

## Compilación

Objetivo actual: **Windows x86_64**.

Compilación local:

```powershell
cargo build --release
```

El `build.rs` descarga cuando es necesario:

- Helix 25.07.1 para Windows;
- JetBrainsMono Nerd Font;
- diccionario ortográfico es-CL.

Se pueden proporcionar recursos locales mediante:

```text
HELIX_SST_FONT_FILE
HELIX_SST_ARCHIVE
```

GitHub Actions comprueba en Windows:

```text
cargo fmt --all -- --check
cargo clippy --release --target x86_64-pc-windows-msvc -- -D warnings
cargo test --release --target x86_64-pc-windows-msvc
cargo build --release --target x86_64-pc-windows-msvc
```

## Licencias

La información de terceros se encuentra en `THIRD_PARTY.md` y `licenses/`.
