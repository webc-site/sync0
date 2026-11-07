# sync0

`sync0` é unha ferramenta de liña de comandos para transferir ficheiros entre redes, sincronizar directorios de forma incremental e distribuír datos simultáneamente a varias máquinas.

## Intención de deseño orixinal

En salas de máquinas, países ou ambientes con atrasos de rede e perda de paquetes, ferramentas como `rsync` e `scp` baséanse na confirmación dunha secuencia de conexión única, o que limita o uso do ancho de banda; cando varias máquinas extraen datos do mesmo servidor ao mesmo tempo, a tarxeta de rede do servidor de orixe pode converterse facilmente nun pescozo de botella.

O obxectivo de `sync0` é resolver dous problemas:

1. Permite que a sincronización de ficheiros e directorios entre as dúas máquinas execute o ancho de banda físico completo e só transfira a parte modificada cando o ficheiro se modifique parcialmente.
2. Varias máquinas poden transferir bloques de datos entre si ao sincronizar o mesmo directorio, reducindo a carga no servidor de orixe e acurtando o tempo global de sincronización.

## Comparación de velocidade con rsync

Entorno de proba: nodo europeo $\leftrightarrow$ nodo norteamericano, atraso de ida e volta de 86 ms, que mide o tempo completo, incluíndo a exploración de directorios, a comparación de deduplicación, a transmisión de rede e a verificación da colocación do disco:

| Escenario de transmisión | Tamaño dos datos | `sync0` | `rsync` | múltiple |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 arquivos pequenos | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Paquetes de código e arquivo (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Ficheiro de vídeo (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Sincronización incremental (modificar o contido do 1 %) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Métodos de mellora do rendemento

- **Transmisión simultánea UDP**: envía bloques de datos a través de UDP e só retransmite os fragmentos perdidos para evitar que unha soa perda de paquete bloquee as transferencias de ficheiros posteriores.
- **Segmentación e deduplicación de contido**: divide o ficheiro en bloques de datos segundo o contido e calcula o valor de verificación. O mesmo contido que xa está no extremo receptor reúsase directamente localmente sen duplicar a rede.
- **Axusta automaticamente a compresión segundo a velocidade da rede**: activa a compresión para contido comprimible como texto e código; salta automaticamente a compresión de vídeos, imaxes, paquetes comprimidos ou en LAN de alta velocidade para reducir o consumo de CPU.
- **Extracción de varios nodos nas proximidades**: cando varias máquinas extraen o mesmo directorio, os bloques de datos extraeranse simultaneamente dos nodos descargados da mesma cidade ou cunha latencia máis baixa.

## Instalar

### Método 1: Instalar a través de Cargo

```bash
cargo install sync0
```

### Método 2: Descargar binarios precompilados

Vaia a [Versións de GitHub](https://github.com/webc-site/sync0/releases) para descargar o paquete comprimido `.tar.xz` para o sistema correspondente (admite Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). Despois de descomprimir, coloque `sync0` no directorio `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Guía de usuario

### 1. Transferir ficheiros ou directorios directamente entre dúas máquinas

Comezar a supervisar na máquina receptora e especificar o camiño de recepción e gardado:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Envía un único ficheiro ou un directorio completo desde a máquina remitente:

```bash
sync0 send ./project 192.168.1.10:9000
```

Parámetros de uso común:

```bash
# Eliminar ficheiros redundantes que existen no extremo receptor pero non no extremo emisor (sincronización espello)
sync0 send ./project 192.168.1.10:9000 --delete

# Limite a taxa de envío (unidade: bytes/segundo, por exemplo 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Modo silencioso (non se mostra a barra de progreso)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Publicación de directorios compartidos e extracción de varios nodos

Publica un directorio compartido (chamado `dataset`) no servidor de orixe:

```bash
sync0 share dataset ./data --region SHA
```

Tire o directorio compartido noutras máquinas (pode encher varios enderezos de nodos ao mesmo tempo, separados por comas para extraer simultaneamente):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Tira só un determinado subdirectorio do directorio compartido:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Envía os cambios locais de volta ao directorio compartido remoto:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Subscríbete ás actualizacións automáticas do catálogo

Subscríbete ao directorio compartido do nodo fonte. Cando se engade ou modifique un ficheiro no directorio de orixe, sincronizarase automaticamente co ordenador local. Ao mesmo tempo, a máquina local servirá automaticamente como un nodo para que outras máquinas tiren:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Sincronización bidireccional de grupos de varias máquinas

Varias máquinas únense ao mesmo nome de grupo e os cambios en calquera directorio de nodos sincronizaranse automaticamente con outros nodos do grupo:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Identidade do nodo e tarxeta de visita persoal

Cada nodo admite a configuración do nome de usuario, cidade, correo electrónico, sitio web e unha sinatura personalizada (máximo 256 caracteres) e pasa a autenticación de sinatura de clave privada local:

```bash
# Ver o ID da chave pública local (especifique o directorio de configuración para que a chave permaneza)
sync0 id -c ~/.config/sync0

# Xera e mostra tarxetas de visita de nodos con sinaturas personalizadas
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` tamén se poden iniciar con estes parámetros de perfil:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Cambio multilingüe

`sync0` Admite 75 idiomas. De xeito predeterminado, a axuda da liña de comandos e as solicitudes no idioma correspondente móstranse automaticamente segundo o ambiente lingüístico do sistema operativo; se o idioma do sistema non está na lista admitida, úsase inglés por defecto.

Para especificar o idioma manualmente, configure a variable de ambiente `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```