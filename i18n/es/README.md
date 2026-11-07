# sync0

`sync0` es una herramienta de línea de comandos para transferir archivos a través de redes, sincronizar directorios de forma incremental y distribuir datos simultáneamente a varias máquinas.

## Intención de diseño original

En salas de máquinas, países o entornos con retrasos en la red y pérdida de paquetes, herramientas como `rsync` y `scp` se basan en la confirmación de una secuencia de conexión única, lo que limita la utilización del ancho de banda; Cuando varias máquinas extraen datos del mismo servidor al mismo tiempo, la tarjeta de red del servidor de origen puede convertirse fácilmente en un cuello de botella.

El objetivo de `sync0` es resolver dos problemas:

1. Deje que la sincronización de archivos y directorios entre las dos máquinas ejecute todo el ancho de banda físico y solo transfiera la parte modificada cuando el archivo esté parcialmente modificado.
2. Varias máquinas pueden transferir bloques de datos entre sí al sincronizar el mismo directorio, lo que reduce la carga en el servidor de origen y acorta el tiempo general de sincronización.

## Comparación de velocidad con rsync

Entorno de prueba: nodo europeo $\leftrightarrow$ nodo norteamericano, retraso de ida y vuelta de 86 ms, medición del tiempo completo que incluye escaneo de directorio, comparación de deduplicación, transmisión de red y verificación de ubicación del disco:

| Escenario de transmisión | Tamaño de datos | `sync0` | `rsync` | múltiple |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 archivos pequeños | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Paquetes de código y archivo (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Archivo de vídeo (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Sincronización incremental (modificar el 1% del contenido) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Métodos de mejora del rendimiento.

- **Transmisión simultánea UDP**: envíe bloques de datos a través de UDP y solo retransmita fragmentos perdidos para evitar la pérdida de un solo paquete que bloquee las transferencias de archivos posteriores.
- **Segmentación y deduplicación de contenido**: divida el archivo en bloques de datos según el contenido y calcule el valor de verificación. El mismo contenido que ya se encuentra en el extremo receptor se reutiliza directamente localmente sin duplicar la red.
- **Ajuste automáticamente la compresión según la velocidad de la red**: active la compresión para contenido comprimible como texto y código; omita automáticamente la compresión de videos, imágenes, paquetes comprimidos o en LAN de alta velocidad para reducir el consumo de CPU.
- **Extracción cercana de varios nodos**: cuando varias máquinas extraen el mismo directorio, los bloques de datos se extraerán simultáneamente de los nodos descargados en la misma ciudad o con menor latencia.

## Instalar

### Método 1: instalar mediante Cargo

```bash
cargo install sync0
```

### Método 2: descargar archivos binarios precompilados

Vaya a [Versiones de GitHub](https://github.com/webc-site/sync0/releases) para descargar el paquete comprimido `.tar.xz` para el sistema correspondiente (compatible con Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). Después de descomprimir, coloque `sync0` en el directorio `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Guía del usuario

### 1. Transfiera archivos o directorios directamente entre dos máquinas

Inicie el monitoreo en la máquina receptora y especifique la ruta de recepción y guardado:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Envíe un solo archivo o un directorio completo desde la máquina emisora:

```bash
sync0 send ./project 192.168.1.10:9000
```

Parámetros comúnmente utilizados:

```bash
# Eliminar archivos redundantes que existen en el extremo receptor pero no en el extremo emisor (sincronización espejo)
sync0 send ./project 192.168.1.10:9000 --delete

# Limitar la velocidad de envío (unidad: bytes/segundo, por ejemplo 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Modo silencioso (no se muestra ninguna barra de progreso)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Publicar directorios compartidos y extraer de múltiples nodos

Publique un directorio compartido (llamado `dataset`) en el servidor de origen:

```bash
sync0 share dataset ./data --region SHA
```

Extraiga el directorio compartido en otras máquinas (puede completar varias direcciones de nodo al mismo tiempo, separadas por comas para extraer al mismo tiempo):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Extraiga solo un subdirectorio determinado en el directorio compartido:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Envíe los cambios locales al directorio compartido remoto:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Suscríbete a las actualizaciones automáticas del catálogo.

Suscríbase al directorio compartido del nodo de origen. Cuando se agrega o modifica un archivo en el directorio de origen, se sincronizará automáticamente con la computadora local. Al mismo tiempo, la máquina local servirá automáticamente como un nodo para que otras máquinas extraigan:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Sincronización bidireccional de grupos de varias máquinas

Varias máquinas se unen al mismo nombre de grupo y los cambios en cualquier directorio de nodos se sincronizarán automáticamente con otros nodos del grupo:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Identidad de nodo y tarjeta de presentación personal.

Cada nodo admite la configuración del nombre de usuario, ciudad, correo electrónico, sitio web y una firma personalizada (máximo 256 caracteres), y pasa la autenticación de firma de clave privada local:

```bash
# Ver el ID de la clave pública local (especifique el directorio de configuración para conservar la clave)
sync0 id -c ~/.config/sync0

# Genere y muestre tarjetas de presentación de nodos con firmas personalizadas
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` también se pueden iniciar con estos parámetros de perfil:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Cambio de varios idiomas

`sync0` Admite 75 idiomas. De forma predeterminada, la ayuda y las indicaciones de la línea de comandos en el idioma correspondiente se muestran automáticamente de acuerdo con el entorno de idioma del sistema operativo; Si el idioma del sistema no está en la lista admitida, se utiliza el inglés de forma predeterminada.

Para especificar el idioma manualmente, establezca la variable de entorno `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```