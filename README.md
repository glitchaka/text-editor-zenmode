# Helix-SST Zenmode

Editor de texto sin distracciones basado en Helix, con lanzador de archivos integrado, terminal propia y herramientas de escritura en español.

## Flujo de inicio

Al abrir `helix-sst-zen.exe`, la ventana muestra los archivos y directorios del directorio actual:

```text
HELIX-SST
C:\Users\...\Documents\Novela

> [ Nuevo archivo ]
  ▸ notas/
    capitulo-01.txt
    capitulo-02.txt
    worldbuilding.md
```

Controles del selector:

- `↑` / `↓`: mover la selección.
- `Enter`: abrir el archivo seleccionado o entrar a un directorio.
- `N`: crear un archivo nuevo.
- `Backspace`: subir al directorio padre.
- `R`: refrescar el listado.
- `Esc`: cerrar el lanzador.

Al abrir o crear un archivo, la misma ventana pasa al editor. Al cerrar Helix, vuelve al selector y conserva el directorio.

## Características

- Helix 25.07.1.
- Interfaz minimalista orientada a escritura sin distracciones.
- Tema Gruvbox.
- Corrector ortográfico es-CL.
- Diagnóstico ortográfico virtual debajo de la línea activa:
  ```text
  palabraa
       └─ Posible error ortográfico: «palabraa»
  ```
- `F2` para acciones ortográficas.
- Diccionario personal para aceptar palabras.
- `Alt+D` y `Ctrl+G` para insertar `—`.
- Portapapeles del sistema.
- Pegado multilínea.
- UTF-8 y true color.
- Ventana y terminal propias.
- PTY dedicado para ejecutar Helix.
- Redimensionado dinámico de filas y columnas.
- Normalización de salida VT para evitar desplazamientos incorrectos del cursor durante repaints complejos.

## Abrir un archivo directamente

También se puede iniciar el editor con una ruta:

```powershell
helix-sst-zen capitulo-03.txt
```

## Compilación

El proyecto está pensado inicialmente para Windows.

```powershell
cargo build --release
```

Durante la compilación se obtiene la versión fijada de Helix, JetBrainsMono Nerd Font y el diccionario es-CL.

Se pueden proporcionar archivos locales mediante:

```text
HELIX_SST_ARCHIVE
HELIX_SST_FONT_FILE
```

## Licencias

La información de componentes de terceros está disponible en `THIRD_PARTY.md` y en el directorio `licenses/`.
