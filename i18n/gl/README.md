# sync0

`sync0` é unha ferramenta de compartición de ficheiros P2P descentralizada, sincronización en tempo real e distribución de fontes múltiples que se usa para substituír a `rsync`: a velocidade medida da WAN transcontinental é de ata **5,89×** de `rsync` (velocidade media xeométrica dun punto **2,67×**, velocidade media P2P dual** e xeométrica **. A aceleración P2P da ligazón de alta latencia é de ata **7,68×** (velocidade media xeométrica completa de 24 escenarios de enlace completo **2,56×**).

Cada máquina é tanto un servidor como un cliente: non é necesario un servidor central nin un enderezo de rede pública, a perforación automática baséase no enderezo de chave pública de rede distribuída e admite a transmisión directa entre dúas máquinas, aloxamento de permisos de grupo para compartir múltiples, selección múltiple de subdirectorios baixo demanda, sincronización incremental en tempo real de milisegundos e aceleración simultánea de fontes múltiples P2P.

## Instalar

### Método 1: instalación de carga

```bash
cargo install sync0
```

### Método 2: Descargar binarios precompilados

Vai a [Versións de GitHub](https://github.com/webc-site/sync0/releases) para descargar o paquete comprimido `.tar.xz` correspondente ao sistema (compatible con Linux, macOS, Windows, Android, iOS, FreeBSD), descomprime e colócalo en `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Comeza rapidamente

### 1. Transmisión directa de configuración cero entre dúas máquinas (`send` / `recv`)

Non é necesario configurar un grupo para compartir, transferir directamente ficheiros ou directorios entre dúas máquinas a alta velocidade:

```bash
# O extremo receptor escoita no porto e escribe en ./backup
sync0 recv 0.0.0.0:9000 ./backup

# Transmisión directa do remitente (opcional --eliminar espello para limpar ficheiros en exceso, --rate límite de velocidade de 50 MB)
sync0 send ./project 192.168.1.10:9000
```

### 2. Compartir publicacións e extraer/cargar baixo demanda (`share`/`pull`/`push`)

```bash
# Servidor: publique o grupo de uso compartido cunha liña de comando (saiga o enderezo do nodo iroh sen IP despois do inicio)
sync0 share dataset:./data

# Cliente: extrae todo o grupo de compartición do conxunto de datos ao local ./local_data
sync0 pull dataset:// ./local_data -p <enderezo de nodo>

# Cliente: extrae varias seleccións baixo demanda (só sincroniza ficheiros e subdirectorios únicos especificados)
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <enderezo de nodo>

# Cliente: cargue ficheiros ou directorios a grupos de uso compartido autorizados
sync0 push dataset://releases/v1 ./build_output -p <enderezo de nodo>
```

### 3. Semente de subscricións en tempo real e sincronización bidireccional entre varias máquinas (`sub`/`group`)

Despois de establecer unha conexión longa, se ocorre algún ficheiro **engadindo, modificando, eliminando, cambiando permisos ou tempo de modificación** no extremo remoto, o directorio local actualizarase sincronizadamente en milisegundos en tempo real (os ficheiros grandes novos/modificados só tirarán os bloques de diferenza e actualizalos no seu lugar, eliminar ficheiros limpará automaticamente os ficheiros locais e os directorios baleiros, e só cambiarán directamente os bloques de permisos cero e marcarán os tempos):

- **`sync0 sub`(Replica unidireccional en tempo real + semente automática)**: o tempo real local segue as adicións, eliminacións e modificacións do extremo de uso compartido e serve automaticamente como fonte de semente P2P para proporcionar aceleración simultánea para outros nodos de extracción.
- **`sync0 group`(Sincronización bidireccional en tempo real de punto a punto de varias máquinas)**: se se engaden, eliminan ou modifican ficheiros en calquera máquina do grupo, todos os demais nós en liña actualizaranse de forma sincronizada en tempo real e servirán de semente entre si.

```bash
# Subscríbete a todo o grupo de uso compartido: as adicións, eliminacións e modificacións remotas sincronízanse localmente en tempo real e enséranse automaticamente
sync0 subdataset:// ./local_mirror -p <enderezo de nodo>

# Subscrición de selección múltiple baixo demanda: só sincroniza e semente ficheiros e subdirectorios únicos especificados en tempo real
sync0 subdataset://models/llama.onnx,configs ./local_mirror -p <enderezo de nodo>

# Sincronización bidireccional en tempo real de grupos de pares de varias máquinas (tamén admite rutas de selección múltiple completas ou de coma)
sync0 group team-docs:// ./docs -p <enderezo de nodo>
```

### 4. Configuración de permisos de membros e grupos de uso compartido múltiple do equipo (`.nt` configuración)

O directorio de configuración predeterminado é `~/.config/sync0` (pódese cambiar mediante `-c <directory>`), utilizando o formato minimalista [NestedText](https://nestedtext.org) (`.nt`):
- **Admite restrinxir a lectura da rede pública (só os usuarios autorizados poden ler)**: sempre que o grupo de uso compartido estea configurado cunha lista de membros `users:` (ou a liña de comandos especifique `--allow-r` / `--allow-rw` / `--allow-w` / `--auth`), o sistema **activa automaticamente a rede privada auténtica="identificativa"> enumeración ou lectura de nodos, e a subscrición P2P a nós de semente tamén herdará a lista branca simultaneamente, permitindo só ler os usuarios autorizados; os grupos de uso compartido que non están configurados con `users` son públicos e de só lectura por defecto (se queres ser públicos e só de lectura e abertos para cargas só para membros especificados, podes configurar `auth: false` explícitamente).
- **Permisos finos de lectura-escritura e entrega de subdirectorios**: os membros son de só lectura por defecto (`r` non é necesario que se escriban de forma explícita) e admiten `r` (só lectura), `w` (só escribe no cadro desplegable, non é lexible), `rw` (prefixo directo) e `rw` (prefixo directo) permitido para carga).

**Exemplo de configuración de membros (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (só lectura predeterminada, límite de velocidade 80 MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (permisos de lectura e escritura, só se permite cargar no subdirectorio especificado)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (só permiso de entrega de escritura, prohibida a lectura)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Exemplo de configuración de grupo compartido (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Usuarios non configurados: rede pública predeterminada de só lectura
  dataset:
    path: ./data
    rate: 50MB

  # Usuarios configurados: restrinxe automaticamente a lectura da rede pública (só Alice e Bob poden ler, Charlie só pode entregar e cargar)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Inicie o servizo (cargue o ficheiro de configuración ou use directamente os parámetros da liña de comandos para restrinxir a lectura da rede pública e autorizar os membros):

```bash
# Cargue ~/.config/sync0/config.nt para comezar
sync0 share --region SHA

# Ou publica un grupo privado para compartir mediante unha liña de comandos pura (especifique --allow-* para restrinxir automaticamente a lectura da rede pública e só os usuarios autorizados poden acceder a ela)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Enderezo de nodo, tarxeta de visita e multilingüe

```bash
# Consulta o enderezo do nodo local sen IP (a clave xérase automaticamente na primeira execución)
sync0 id

# Xera tarxeta de visita de nodo autoasinada Ed25519 (alcume, código de cidade, correo electrónico, sitio web, presentación)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# Hai 75 idiomas incorporados, que seguen o idioma do sistema por defecto e pódense cambiar a través de variables de ambiente.
SYNC0_LANG=en sync0 --help
```

## Rendemento e características fundamentais

### Medición real de WAN intercontinental (Comparación `rsync`)

Entorno de medición real: Estados Unidos `c1` → Europa `n1` (RTT 65 ms), o conxunto de datos é o núcleo de Linux `linux-7.3.3`:

#### 1) Modo á carta vs `rsync`

| Conxunto de datos | tamaño | `sync0` Punto único<br>(Mbps) | `rsync`<br>(Mbps) | Acelerar (`sync0`/`rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(non comprimible) | 155 MB | **236.48** | 108.74 | **2,17×** |
| `linux-7.3.3.tar`(comprimible) | 1.6 GB | **843.83** | 385.06 | **2,19×** |
| `linux-7.3.3.tar`(incremento de cambio local) | 1.6 GB | **8,014.15** | 1,361.72 | **5,89×** |
| `linux-7.3.3/` (95.979 ficheiros pequenos) | 1.5 GB | **627.58** | 347.80 | **1,80×** |
| **media xeométrica** | — | **1,000.91** | 375.26 | **2,67×** |

#### 2) Modo de fonte dual P2P vs `rsync`

| Conxunto de datos | tamaño | `sync0` Fonte dual P2P<br>(Mbps) | `rsync`<br>(Mbps) | Acelerar (`sync0`/`rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(non comprimible) | 155 MB | **330.09** | 108.74 | **3,04×** |
| `linux-7.3.3.tar`(comprimible) | 1.6 GB | **1,551.86** | 385.06 | **4,03×** |
| `linux-7.3.3.tar`(incremento de cambio local) | 1.6 GB | **5,474.17** | 1,361.72 | **4,02×** |
| `linux-7.3.3/` (95.979 ficheiros pequenos) | 1.5 GB | **706.56** | 347.80 | **2,03×** |
| **media xeométrica** | — | **1,186.42** | 375.26 | **3,16×** |

> Nota: no modo de dobre fonte P2P, os nodos europeos `s1` colaboran para a sementeira, e o RTT da columna vertebral intercontinental é de 65 ms.

### Características principais do produto

- **Perforación automática de buracos de enderezo de chave pública**: non é necesario un servidor central nin un enderezo de rede pública, perforación automática de buratos NAT baseada no enderezo da chave pública de rede distribuída.
- **Sincronización en tempo real a nivel de milisegundos e conciencia de metadatos**: baseado no seguimento nativo de eventos de ficheiros (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), engadidos de ficheiros en tempo real, eliminacións, permisos POSIX (`mode`) e tempo de modificación a nivel de milisegundos (`mtime`); transmisión de bloque de datos cero cando só se modifican os permisos ou a hora.
- **Descarga e carga simultáneas ao estilo BT e programación de afinidade rexional**: o nodo de descarga non precisa esperar a que se descargue a cantidade total. Todos os intervalos de bloque (`Op::Have`) que pasaron a comprobación de hash de 128 bits están bloqueados inmediatamente na agrupación de memoria/temporal para que outros nodos poidan extraer simultaneamente. Xunto co intercambio dinámico de nodos PEX e a preferencia media de nodos lentos (Work-Stealing), o rendemento do clúster é maximizado.
- **Alteración de nodos anti-maliciosos e inmunidade ao envelenamento de clúster**: lista de fontes autorizadas `root_hash` Bloqueo auto-coherente + aceleración de hardware obrigatoria `gxhash128` verificación de dixestión antes de que cada bloque se coloque no disco e se semente externamente; se se atopa un bloque manipulado, o interruptor de nivel de milisegundos apagará o nodo malicioso e recuperará automaticamente a fonte honesta.
- **Incremento FastCDC in situ e agregación de ficheiros pequenos**: só se transfiren bloques diferenciais para a modificación parcial de ficheiros grandes e colócanse no disco; O envasado masivo de memoria de ficheiros pequenos e a compresión diferencial de prefixos eliminan a sobrecarga de ida e volta ficheiro por ficheiro.
- **Autocuración despois do corte de enerxía e reinicio despois do reinicio**: o formato híbrido RDB + AOF (`.s0_subs.bin`) úsase para manter o estado de semente e a lista de ficheiros, os cambios diarios engádense en microsegundos e reescríbense e organízanse automaticamente regularmente. Despois de reiniciar o servidor, os cambios fóra de liña envíanse automaticamente ao nodo de sementeira.

## Arquitectura técnica e documentación

- **Protocolo de desacoplamento de dobre plano**:
  - **Superficie de control e cambio en tempo real (`iroh` QUIC/TCP, primeiro byte `PROTO_VERSION` axuste de man da versión) **: Responsable da verificación da tarxeta de visita Ed25519, verificación de permisos, descubrimento de sementes de Swarm e compatibilidade con `u64::MAX` lotes de transmisión de cambios masivos de nivel e `mzstd` comprimido en tempo real `mzstd` id="5"/6"> `FsOp::Remove` / `FsOp::Meta` Tres primitivos).
  - **Superficie de distribución de datos (UDP nativa + `compio` E/S asíncrona) **: cabeceira compacta 20B, retroalimentación periódica TACK 18B, control de conxestión híbrido BBRv3/Copa, recuperación da perda de paquetes RACK-TLP, desvío de compresión Zstd adaptativo de entropía Shannon do primeiro bloque e desvío de compresión Zstd adaptativo de entropía de Shannon e verificación final do bloque <b id-="2">.
- **Máis información**:
  - [Informe de medición do rendemento e guión de reprodución](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Deseño de arquitectura do sistema](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Protocolo de cable binario e especificación de persistencia](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Autenticación de identidade, control de autoridade e verificación da integridade](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)