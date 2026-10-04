# Helix-SST Zenmode

Editor de escritura basado en Helix con formato nativo `.hsst`, biblioteca propia, proyectos lógicos, corrector es-CL y exportación.

## Biblioteca

Al abrir Zenmode sin argumentos usa por defecto:

```text
Documentos\Helix SST\
├── documents\
└── exports\
```

La ruta puede cambiarse con `HELIX_SST_LIBRARY`.

El launcher solo muestra directorios y documentos de texto relevantes. Los documentos `.hsst` pueden pertenecer a proyectos aunque estén físicamente sueltos en `documents\`.

## Formato .hsst

Los documentos nuevos se crean como `.hsst` e incluyen metadata TOML:

```text
+++
format = 1
id = "..."
title = "Capítulo 1"
project = "Puerto Ámbar"
type = "chapter"
chapter = 1
order = 10
language = "es-CL"
status = "draft"
+++

Texto del capítulo...
```

`id` es estable y permite reconocer el documento aunque cambie de nombre.

Marcado enriquecido inicial:

- `**negrita**`
- `*cursiva*`
- `==destacado==`
- `# Título`

## Proyectos

Los campos `project`, `type`, `chapter` y `order` forman proyectos lógicos sin obligar a mover los archivos a carpetas separadas. La exportación de proyecto respeta ese orden.

## Exportación

En el launcher selecciona un documento y pulsa `E`.

Se puede exportar el documento actual o el proyecto completo a:

- TXT: texto limpio.
- DOCX: Word/OpenXML sin requerir Microsoft Word; conserva títulos, negrita, cursiva y destacados.
- PDF: salida de lectura/entrega.

Los archivos generados quedan en `Documentos\Helix SST\exports\`.

## Corrector y completado

Los `.hsst` usan el lenguaje `prose` de Helix-SST y cargan automáticamente `helix-sst-spell`.

- diagnósticos ortográficos inline en la línea activa;
- `F2` para correcciones;
- diccionario personal;
- completado LSP con palabras del documento, palabras de otros documentos del mismo proyecto y sugerencias ortográficas;
- tokens semánticos para metadata, títulos, negrita y destacados.

## Arrastrar un archivo sobre el ejecutable

En Windows, arrastrar un archivo de texto sobre `helix-sst-zen.exe` hace que Windows entregue la ruta como argumento. Zenmode abre directamente Helix con ese archivo cargado, sin pasar por el selector ni esperar la pantalla de bienvenida.

También puede hacerse desde consola:

```powershell
helix-sst-zen "C:\ruta\Capítulo 1.hsst"
helix-sst-zen --zen "C:\ruta\Capítulo 1.hsst"
```

## Controles principales

Launcher:

- `↑ / ↓`: selección.
- `Enter`: elegir/abrir.
- `N`: documento nuevo `.hsst`.
- `E`: exportar.
- `Z`: alternar modo normal/Zenmode real.
- `R`: refrescar.
- `Backspace`: subir de directorio sin escapar de la biblioteca predeterminada.
- `Esc`: cerrar.

Editor:

- `Z` en NORMAL: entrar/salir de Zenmode real.
- `Ctrl+← / Ctrl+→`: mover por palabras.
- `Ctrl+Backspace / Ctrl+Supr`: borrar palabra.
- `F2`: corrección ortográfica.
- `Alt+D` / `Ctrl+G`: insertar raya `—`.

## Compilación

```powershell
cargo build --release
```

GitHub Actions comprueba formato, Clippy, tests y build de Windows.

## Licencias

La información de terceros está en `THIRD_PARTY.md` y `licenses/`.
