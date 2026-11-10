# sync0

`sync0` es una herramienta descentralizada para compartir archivos P2P, sincronización en tiempo real y distribución de fuentes múltiples que se utiliza para reemplazar a `rsync`: la velocidad medida de la red de área amplia transcontinental es de hasta **6,95×** de `rsync` (velocidad promedio geométrica de un solo punto **2,01×**, velocidad promedio geométrica P2P de doble fuente **3,33×**), y la sincronización de archivos masivos pequeños archivos a través de océanos es tan rápido como **7,19×**.

Cada máquina es a la vez un servidor y un cliente: no hay necesidad de un servidor central o una dirección de red pública, la perforación automática se basa en la dirección de clave pública de la red distribuida y admite la transmisión directa entre dos máquinas, alojamiento de permisos de grupo de uso compartido múltiple, selección múltiple de subdirectorios bajo demanda, sincronización incremental en tiempo real a nivel de milisegundos y aceleración simultánea de múltiples fuentes P2P.

## Instalar

### Método 1: instalación de carga

```bash
cargo install sync0
```

### Método 2: descargar archivos binarios precompilados

Vaya a [GitHub Releases](https://github.com/webc-site/sync0/releases) para descargar el paquete comprimido `.tar.xz` correspondiente al sistema (compatible con Linux, macOS, Windows, Android, iOS, FreeBSD), descomprímalo y colóquelo en `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Comience rápidamente

### 1. Transmisión directa sin configuración entre dos máquinas (`send` / `recv`)

No es necesario configurar un grupo para compartir, transferir archivos o directorios directamente entre dos máquinas a alta velocidad:

```bash
# El extremo receptor escucha en el puerto y escribe en ./backup
sync0 recv 0.0.0.0:9000 ./backup

# Transmisión directa desde el remitente (opcional: eliminar espejo para limpiar archivos sobrantes, límite de velocidad de 50 MB)
sync0 send ./project 192.168.1.10:9000
```

### 2. Compartir publicaciones y extraer/cargar bajo demanda (`share` / `pull` / `push`)

```bash
# Servidor: publique el grupo compartido con una línea de comando (ingrese la dirección del nodo iroh sin IP después del inicio)
sync0 share dataset:./data

# Cliente: extraiga todo el grupo de intercambio de conjuntos de datos al ./local_data local
sync0 extraer conjunto de datos:// ./local_data -p <dirección de nodo>

# Cliente: extraiga múltiples selecciones a pedido (sincronice solo archivos y subdirectorios individuales específicos)
sync0 extrae el conjunto de datos://docs/readme.md,assets/icons,src/core ./local_data -p <dirección de nodo>

# Cliente: cargar archivos o directorios en grupos para compartir autorizados
sync0 push dataset://releases/v1 ./build_output -p <dirección de nodo>
```

### 3. Generación de suscripciones en tiempo real y sincronización bidireccional entre múltiples máquinas (`sub` / `group`)

Después de establecer una conexión larga, si se produce algún archivo **agregar, modificar, eliminar, cambiar permisos o tiempo de modificación** en el extremo remoto, el directorio local se actualizará sincrónicamente en milisegundos en tiempo real (los archivos grandes nuevos/modificados solo extraerán los bloques de diferencia y los actualizarán en su lugar, los archivos eliminados limpiarán automáticamente los archivos locales y los directorios vacíos, y solo cambiarán los permisos o marcarán la hora con cero bloques de datos directamente):

- **`sync0 sub`(Duplicación unidireccional en tiempo real + siembra automática)**: el tiempo real local sigue las adiciones, eliminaciones y modificaciones del extremo compartido y sirve automáticamente como fuente de semilla P2P para proporcionar aceleración simultánea para otros nodos de extracción.
- **`sync0 group`(Sincronización bidireccional en tiempo real entre pares de múltiples máquinas)**: si se agregan, eliminan o modifican archivos en cualquier máquina del grupo, todos los demás nodos en línea se actualizarán sincrónicamente en tiempo real y servirán como semillas entre sí.

```bash
# Suscríbase a todo el grupo para compartir: las adiciones, eliminaciones y modificaciones remotas se sincronizan localmente en tiempo real y se transmiten automáticamente
subconjunto de datos sync0:// ./local_mirror -p <dirección de nodo>

# Suscripción de selección múltiple bajo demanda: sincronice y genere archivos y subdirectorios únicos específicos en tiempo real
subconjunto de datos sync0://models/llama.onnx,configs ./local_mirror -p <dirección de nodo>

# Sincronización bidireccional en tiempo real de grupos de pares de varias máquinas (también admite rutas de selección múltiple completas o por comas)
grupo sync0 team-docs:// ./docs -p <dirección de nodo>
```

### 4. Configuración de permisos de miembro y grupo de uso compartido múltiple del equipo (configuración `.nt`)

El directorio de configuración predeterminado es `~/.config/sync0` (se puede cambiar mediante `-c <directory>`), usando el formato minimalista [NestedText](https://nestedtext.org) (`.nt`):
- **Admite restringir la lectura en la red pública (solo los usuarios autorizados pueden leer)**: siempre que el grupo compartido esté configurado con una lista de miembros `users:` (o la línea de comando especifique `--allow-r` / `--allow-rw` / `--allow-w` / `--auth`), el sistema **activa automáticamente la autenticación privada (`auth: true`)** y prohíbe la enumeración o lectura de nodos anónimos de la red pública, y La suscripción P2P a nodos de siembra también heredará la lista blanca simultáneamente, lo que permitirá que solo los usuarios autorizados lean; Los grupos para compartir que no están configurados con `users` son públicos y de solo lectura de forma predeterminada (si desea ser públicos y de solo lectura y estar abiertos a cargas solo para miembros específicos, puede configurar explícitamente `auth: false`).
- **Permisos detallados de lectura, escritura y entrega de subdirectorios**: los miembros son de solo lectura de forma predeterminada (`r` no necesita escribirse explícitamente) y admiten `r` (solo lectura), `w` (solo escribe en el cuadro desplegable, no legible), `rw` (lectura-escritura) y `write_prefix` (limita el prefijo de subdirectorio permitido para la carga).

**Ejemplo de configuración de miembros (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (solo lectura predeterminada, límite de velocidad 80 MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (permisos de lectura y escritura, solo se permite cargar en el subdirectorio especificado)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (solo permiso de entrega de escritura, está prohibida la lectura)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Ejemplo de configuración de grupo compartido (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Usuarios no configurados: red pública predeterminada de solo lectura
  dataset:
    path: ./data
    rate: 50MB

  # Usuarios configurados: restringe automáticamente la lectura de la red pública (solo Alice y Bob pueden leer, Charlie solo puede entregar y cargar)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Inicie el servicio (cargue el archivo de configuración o use directamente los parámetros de la línea de comando para restringir la lectura de la red pública y autorizar a los miembros):

```bash
# Cargue ~/.config/sync0/config.nt para comenzar
sync0 share --region SHA

# O publique un grupo privado para compartir a través de una línea de comando pura (especifique --allow-* para restringir automáticamente la lectura de la red pública y solo los usuarios autorizados puedan acceder a ella)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Dirección de nodo, tarjeta de presentación y multilenguaje.

```bash
# Ver la dirección del nodo local libre de IP (la clave se genera automáticamente para la primera ejecución)
sync0 id

# Genere una tarjeta de presentación de nodo autofirmada Ed25519 (apodo, código de ciudad, correo electrónico, sitio web, introducción)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# Hay 75 idiomas integrados, que siguen el idioma del sistema de forma predeterminada y se pueden cambiar mediante variables de entorno.
SYNC0_LANG=en sync0 --help
```

## Rendimiento y características principales

### Medición real de WAN transcontinental (Comparación `rsync`)

Entorno de medición real: Estados Unidos `c1` → Europa `n1` (RTT 65 ms), el conjunto de datos es el kernel de Linux `linux-7.3.3`:

#### 1) Modo a la carta vs `rsync`

| conjunto de datos | tamaño | `sync0` Punto único<br>(Mbps) | `rsync`<br>(Mbps) | Aceleración (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(no comprimible) | 155 MB | **121.07** | 107.89 | **1,12×** |
| `linux-7.3.3.tar`(comprimible) | 1.6 GB | **442.07** | 506.62 | **0,87×** |
| `linux-7.3.3.tar`(incremento de cambio local) | 1.6 GB | **8,123.87** | 1,169.49 | **6,95×** |
| `linux-7.3.3/` (95.979 archivos pequeños) | 1.5 GB | **565.49** | 235.27 | **2,40×** |
| **media geométrica** | — | **704.17** | 350.19 | **2,01×** |

#### 2) Modo P2P de doble fuente frente a `rsync`

| conjunto de datos | tamaño | `sync0` P2P de doble fuente<br>(Mbps) | `rsync`<br>(Mbps) | Aceleración (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(no comprimible) | 155 MB | **466.85** | 107.89 | **4,33×** |
| `linux-7.3.3.tar`(comprimible) | 1.6 GB | **992.17** | 506.62 | **1,96×** |
| `linux-7.3.3.tar`(incremento de cambio local) | 1.6 GB | **5,666.84** | 1,169.49 | **4,85×** |
| `linux-7.3.3/` (95.979 archivos pequeños) | 1.5 GB | **703.30** | 235.27 | **2,99×** |
| **media geométrica** | — | **1,165.63** | 350.19 | **3,33×** |

> Nota: En el modo P2P de fuente dual, los nodos europeos `s1` colaboran para la siembra y el RTT de la red troncal transcontinental es de 65 ms.

### Características principales del producto

- **Direccionamiento de clave pública perforación automática**: No es necesario un servidor central ni una dirección de red pública, perforación NAT automática basada en la dirección de clave pública de la red distribuida.
- **Sincronización en tiempo real a nivel de milisegundos y conocimiento de metadatos**: basado en monitoreo de eventos de archivos nativos (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), envío en tiempo real de adiciones y eliminaciones de archivos, permisos POSIX (`mode`) y tiempo de modificación a nivel de milisegundos (`mtime`); Transmisión nula de bloques de datos cuando solo se modifican permisos o tiempo.
- **Descarga y carga simultáneas estilo BT y programación de afinidad regional**: el nodo de descarga no necesita esperar a que se descargue la cantidad total. Todos los intervalos de bloque (`Op::Have`) que han pasado la verificación de hash de 128 bits se bloquean inmediatamente en la memoria/grupo temporal para que otros nodos los extraigan simultáneamente. Junto con el intercambio dinámico de nodos PEX y la preferencia media de nodos lentos (robo de trabajo), se maximiza el rendimiento del clúster.
- **Inmunidad contra la manipulación de nodos maliciosos y el envenenamiento de clústeres**: lista de fuentes autorizadas `root_hash` Bloqueo autoconsistente + aceleración de hardware obligatoria `gxhash128` verificación de resumen antes de colocar cada bloque en el disco y sembrarlo externamente; Si se encuentra un bloque manipulado, el disyuntor de nivel de milisegundos apagará el nodo malicioso y se recuperará automáticamente de la fuente honesta.
- **Incremento in situ de FastCDC y agregación de archivos pequeños**: solo se transfieren bloques diferenciales para la modificación parcial de archivos grandes y se colocan en el disco; El empaquetado masivo de memoria de archivos pequeños y la compresión diferencial de prefijos eliminan la sobrecarga de ida y vuelta archivo por archivo.
- **Autorreparación después de un corte de energía y reanudación después del reinicio**: el formato híbrido RDB + AOF similar a Redis (`.s0_subs.bin`) se utiliza para conservar el estado de inicialización y la lista de archivos, los cambios diarios se agregan en microsegundos y se reescriben y organizan automáticamente con regularidad. Después de reiniciar el servidor, los cambios fuera de línea se envían automáticamente al nodo inicial.

## Arquitectura técnica y documentación.

- **Protocolo de desacoplamiento de doble plano**:
  - **Superficie de control y cambio en tiempo real (`iroh` QUIC/TCP, primer byte `PROTO_VERSION` versión de protocolo de enlace) **: Responsable de la verificación de tarjetas de presentación Ed25519, verificación de permisos, descubrimiento de semillas de enjambre y soporte para procesamiento por lotes de transmisión de cambios masivos de nivel `u64::MAX` y envío por lotes en tiempo real `mzstd` comprimido `SubPush` (`FsOp::Upsert` / `FsOp::Remove` / `FsOp::Meta` Tres primitivas).
  - **Superficie de distribución de datos (UDP nativo + `compio` E/S asíncrona) **: encabezado compacto de 20 B, retroalimentación periódica TACK de 18 B, control de congestión híbrido BBRv3/Copa, recuperación de pérdida de paquetes RACK-TLP, derivación de compresión Zstd adaptativa de entropía de Shannon del primer bloque y verificación de extremo a extremo de bloque `gxhash128`.
- **Más información**:
  - [Informe de medición del desempeño y guión de reproducción.](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Diseño de arquitectura del sistema.](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Protocolo de conexión binaria y especificación de persistencia](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Autenticación de identidad, control de autoridad y verificación de integridad.](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)