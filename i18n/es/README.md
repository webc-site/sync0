# sync0

`sync0` es una herramienta de sincronización entre múltiples máquinas y para compartir archivos punto a punto (P2P) que compite con Resilio Sync.

Cada máquina es a la vez un servidor y un cliente: cualquier nodo puede publicar múltiples grupos para compartir archivos. Después de que otros nodos se unan, pueden obtener automáticamente nodos semilla disponibles del servidor par y extraerlos simultáneamente de múltiples fuentes. Una vez completada la sincronización, se crearán automáticamente semillas para más nodos para acelerar las descargas.

## Aspectos destacados principales

- **Todos somos servidores y clientes**: Comparable a Resilio Sync, sin dependencia del servidor central. Cualquier nodo puede crear múltiples grupos para compartir archivos y también puede unirse a los grupos para compartir de otras personas; Los nodos intercambian y obtienen automáticamente nodos semilla disponibles para lograr extracción simultánea y sincronización en tiempo real de múltiples máquinas.
- **Dirección de nodo sin IP**: de forma predeterminada, la clave de nodo y la dirección `base64url` codificada `iroh` se generan automáticamente en `~/.config/sync0` sin necesidad de recordar ni exponer la IP pública.
- **Sincronización de selección múltiple bajo demanda**: admite la sincronización de todo el grupo compartido y también admite la sincronización bajo demanda seleccionando varios archivos o subdirectorios al mismo tiempo.
- **Permisos jerárquicos de miembros del grupo y control de directorio de carga**: admite permisos de solo lectura (`ro`), carga de lectura y escritura (`rw`) y administración (`admin`), y puede especificar individualmente el prefijo de ruta que cada miembro puede cargar (de forma predeterminada, se permite la carga en el directorio raíz del recurso compartido).
- **Sincronización incremental y de velocidad completa entre redes**: transmisión simultánea de archivos grandes, agregación de archivos pequeños masivos en la memoria y bloques de datos sincronizados solo cuando los archivos se modifican parcialmente.

## Comparación de velocidad con rsync

### Medición real de fuente única de la red transoceánica (China → Estados Unidos, retraso 260 ms)

| Escenario de transmisión | Tamaño de datos | modelo | `sync0` | `rsync -az` | Aumentar multiplicador |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 archivos pequeños | 1.91 MB | Sincronización completa | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 archivos pequeños | 1.91 MB | Sincronización incremental | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Paquetes de código y archivo (`.tar`) | 10.0 MB | Sincronización completa | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Paquetes de código y archivo (`.tar`) | 10.0 MB | Sincronización incremental | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Archivo de vídeo (`.mp4`) | 15.0 MB | Sincronización completa | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Archivo de vídeo (`.mp4`) | 15.0 MB | Sincronización incremental | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Prueba de extracción de múltiples nodos P2P de la red transoceánica (nodos duales de China → Estados Unidos, retraso de 260 ms)

Comparación medida de la extracción simultánea rayada del nodo semilla de múltiples fuentes:

| Escenario de transmisión | Tamaño de datos | Siembra de un solo nodo | Siembra simultánea P2P de dos nodos | Aumentar multiplicador |
| :--- | :---: | :---: | :---: | :---: |
| 500 archivos pequeños | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Paquetes de código y archivo (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Archivo de vídeo (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Medición real de la línea dedicada transcontinental de alta velocidad (Europa → América del Norte, retraso 86 ms)

| Escenario de transmisión | Tamaño de datos | `sync0` | `rsync` | Aumentar multiplicador |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 archivos pequeños | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Paquetes de código y archivo (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Archivo de vídeo (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Sincronización incremental (modificar el 1% del contenido) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Instalar

### Método 1: instalar mediante Cargo

```bash
cargo install sync0
```

### Método 2: descargar archivos binarios precompilados

Vaya a [GitHub Releases](https://github.com/webc-site/sync0/releases) para descargar el paquete comprimido `.tar.xz` para el sistema correspondiente (compatible con Linux, macOS, Windows, Android, FreeBSD, NetBSD), descomprímalo y colóquelo en el directorio `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Comience rápidamente

`sync0` usa `~/.config/sync0` como directorio de configuración de forma predeterminada (si necesita cambiar a otros directorios, puede agregar `-c <directorio>`).

### 1. Ver la dirección del nodo local y configurar la tarjeta de presentación

```bash
# Ver la dirección del nodo local (la clave se genera automáticamente en ~/.config/sync0 por primera vez)
sync0 id

# Genere una tarjeta de presentación de nodo personal (admite nombre de usuario, ciudad, correo electrónico, sitio web y firma en 256 palabras)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Publicar varios grupos para compartir archivos y establecer permisos para los miembros del grupo (`.nt` Configuración)

`sync0` está configurado en el formato [NestedText](https://nestedtext.org) (`.nt`). Cada usuario tiene un archivo de configuración `.nt` independiente. Puede especificar el directorio del perfil de usuario a través de `users_dir` en `config.nt` (que incluye automáticamente todos los archivos `.nt` en este directorio). Tanto el usuario como el prefijo de ruta que se permite cargar utilizan una lista desordenada (`-`). **Cuando el prefijo de la ruta de carga no está completo, se permite la carga al directorio raíz del recurso compartido de forma predeterminada**.

1. Cree archivos de configuración `.nt` separados para cada miembro en el directorio `~/.config/sync0/users/`:

`~/.config/sync0/users/alice.nt` (miembro de solo lectura, tarifa limitada `80MB`, `rate` admite unidades `GB`, `MB`, `KB`):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (miembros de carga restringida: solo se les permite cargar en los directorios `uploads/bob` y `shared/common`):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Carga de miembro predeterminada: cuando se omite `write_prefix`, se permite la carga en el directorio raíz del recurso compartido de forma predeterminada):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. Configure el directorio de usuarios y varios grupos para compartir archivos en `~/.config/sync0/config.nt`:

```yaml
users_dir: ./users

groups:
  dataset:
    path: ./data
    auth: false
    perm: ro
    rate: 50MB

  models:
    path: ./models
    auth: false
    perm: ro
    rate: 1GB

  team-vault:
    path: ./vault
    auth: true
    perm: ro
    rate: 512KB
    users:
      - alice
      - bob
      - charlie
```

Comenzar a compartir el servicio:

```bash
# Inicio directo (carga automáticamente todas las configuraciones de usuario .nt en los directorios ~/.config/sync0/config.nt y users_dir)
sync0 share --region SHA

# O publique rápidamente varios grupos para compartir y autorice a los miembros del grupo directamente a través de la línea de comando
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Extraiga todo el contenido o seleccione varios archivos y directorios (`sync0 pull`)

Conéctese a cualquier nodo par para obtener automáticamente todos los nodos semilla disponibles en el grupo compartido y extraiga de varios nodos semilla simultáneamente:

```bash
# Lleve todo el grupo de intercambio de conjuntos de datos al ./local_data local
sync0 extraer conjunto de datos:// ./local_data -p <dirección de nodo>

# Sincronización de selección múltiple: extraiga simultáneamente el archivo único docs/readme.md y los subdirectorios activos/iconos, src/core
sync0 extrae el conjunto de datos://docs/readme.md,assets/icons,src/core ./local_data -p <dirección de nodo>
```

### 4. Cargar un archivo o directorio a un grupo compartido (`sync0 push`)

Los miembros con permisos de carga (`rw` o `admin`) pueden cargar archivos o directorios locales en grupos para compartir de forma remota:

```bash
# Miembros con permiso de carga del directorio raíz: cargar directamente al directorio raíz compartido
sync0 push team-vault:// ./new_release -p <dirección de nodo>

# Solo miembros autorizados a un prefijo específico (como uploads/bob): cargar al prefijo de ruta permitido
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <dirección de nodo>
```

### 5. Suscríbete a sincronización y propagación automáticas (`sync0 sub`)

Después de suscribirse al grupo compartido de un determinado nodo, el servidor local es al mismo tiempo un cliente receptor y un nuevo servidor inicial; Cuando se actualiza el archivo del nodo de origen, se sincroniza automáticamente y proporciona descargas fragmentadas para otros nodos:

```bash
# Suscríbete a todo el grupo para compartir y siembra automáticamente
subconjunto de datos sync0:// ./local_mirror -p <dirección de nodo>

# Solo realice una selección múltiple para suscribirse a archivos y subdirectorios específicos y generarlos
subconjunto de datos sync0://models/llama.onnx,configs ./local_mirror -p <dirección de nodo>
```

### 6. Sincronización entre pares de grupos de varias máquinas (`sync0 group`)

Varias máquinas se unen al mismo grupo compartido, cada una descubre automáticamente los nodos semilla disponibles y mantiene una sincronización bidireccional (admite sincronización completa o selección múltiple de archivos y directorios):

```bash
# Únase completamente al grupo y sincronización bidireccional.
grupo sync0 team-docs:// ./docs -p <dirección de nodo>

# Solo el directorio wal y el archivo meta.json en el grupo de sincronización de selección múltiple
grupo sync0 team-docs://wal,meta.json ./docs -p <dirección de nodo>
```

### 7. Transmisión directa punto a punto entre dos máquinas (`send` / `recv`)

No es necesario preconfigurar un grupo y puede transferir temporalmente archivos o directorios directamente entre dos máquinas:

```bash
# Monitoreo del receptor
sync0 recv 0.0.0.0:9000 ./backup

# El extremo emisor transfiere directamente archivos o directorios (opcional: eliminar espejo para limpiar archivos sobrantes, límite de velocidad de tasa)
sync0 send ./project 192.168.1.10:9000
```

### 8. Cambio de varios idiomas

`sync0` admite 75 idiomas y cambia automáticamente según el entorno de idioma del sistema de forma predeterminada; también se puede especificar manualmente a través de `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```