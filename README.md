# Helix-SST Zenmode

Aplicación independiente basada en la versión de **Helix-SST integrada en Shell Shock Tool**, sin retirar ni modificar esa integración.

La aplicación abre una **terminal dedicada al editor**. No inicia una shell ni muestra comandos de SST.

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

Al abrir o crear un archivo, **la misma ventana pasa a Helix**. Al cerrar Helix, vuelve al selector y conserva el directorio.

## Helix-SST incluido

La primera versión independiente conserva la configuración de Helix-SST 0.2.3:

- Helix 25.07.1.
- Tema Gruvbox.
- Corrector ortográfico es-CL.
- Diagnóstico ortográfico virtual debajo de la línea activa:
  ```text
  palabraa
       └─ Posible error ortográfico: «palabraa»
  ```
- `F2` para acciones ortográficas.
- `Alt+D` y `Ctrl+G` para insertar `—`.
- Portapapeles del sistema mediante proveedor propio.
- Puente PTY/VT con normalización de LF desnudo para evitar el repaint en escalera observado dentro de SST.
- UTF-8 y true color.

## Compilación

El proyecto está pensado inicialmente para Windows.

```powershell
cargo build --release
```

Durante la compilación se obtiene la versión fijada de Helix, JetBrainsMono Nerd Font y el diccionario es-CL. Se pueden proporcionar archivos locales mediante:

```text
HELIX_SST_ARCHIVE
HELIX_SST_FONT_FILE
```

El workflow de GitHub es **manual** (`workflow_dispatch`); no compila automáticamente cada push.

## Estado

Implementación inicial subida sin compilar ni ejecutar pruebas por parte del asistente, siguiendo la política de trabajo del proyecto.
