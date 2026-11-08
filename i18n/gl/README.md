# sync0

`sync0` é unha ferramenta de sincronización entre varias máquinas e para compartir ficheiros punto a punto (P2P) que compite con Resilio Sync.

Cada máquina é tanto un servidor como un cliente: calquera nodo pode publicar varios grupos para compartir ficheiros. Despois de que outros nodos se unan, poden obter automaticamente nodos de semente dispoñibles do servidor de pares e tiralos simultaneamente de varias fontes. Despois de completar a sincronización, as sementes crearanse automaticamente para máis nodos para acelerar as descargas.

## Aspectos básicos

- **Todo o mundo é un servidor e un cliente**: comparable a Resilio Sync, sen dependencia do servidor central. Calquera nodo pode crear varios grupos de compartición de ficheiros e tamén pode unirse a grupos de uso compartido doutras persoas; os nodos intercambian e obteñen automaticamente nodos de semente dispoñibles para lograr o tirado simultáneo e a sincronización en tempo real de varias máquinas.
- **Enderezo de nodo sen IP**: de forma predeterminada, a clave de nodo e o enderezo `base64url` codificado `iroh` xéranse automaticamente en `~/.config/sync0` sen necesidade de lembrar ou expor a IP pública.
- **Sincronización de selección múltiple baixo demanda**: admite a sincronización de todo o grupo de uso compartido e tamén admite a sincronización baixo demanda seleccionando varios ficheiros ou subdirectorios ao mesmo tempo.
- **Permisos xerárquicos dos membros do grupo e control do directorio de carga**: admite permisos de só lectura (`ro`), carga de lectura e escritura (`rw`) e xestión (`admin`), e pode especificar individualmente o prefixo de ruta que cada membro pode cargar (por defecto, a carga do directorio raíz está permitida).
- **Sincronización incremental e a toda velocidade en redes**: transmisión simultánea de ficheiros grandes, agregación de ficheiros pequenos masivos na memoria e só bloques de datos sincronizados cando os ficheiros se modifican parcialmente.

## Comparación de velocidade con rsync

### Medición real dunha fonte única da rede transoceánica (China → Estados Unidos, atraso de 260 ms)

| Escenario de transmisión | Tamaño dos datos | modelo | `sync0` | `rsync -az` | Aumentar o multiplicador |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 arquivos pequenos | 1.91 MB | Sincronización total | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 arquivos pequenos | 1.91 MB | Sincronización incremental | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Paquetes de código e arquivo (`.tar`) | 10.0 MB | Sincronización total | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Paquetes de código e arquivo (`.tar`) | 10.0 MB | Sincronización incremental | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Ficheiro de vídeo (`.mp4`) | 15.0 MB | Sincronización total | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Ficheiro de vídeo (`.mp4`) | 15.0 MB | Sincronización incremental | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Proba de extracción de múltiples nodos P2P da rede transoceánica (nodos duales China → Estados Unidos, atraso de 260 ms)

Comparación medida do nodo de semente de fontes múltiples tirado simultáneamente:

| Escenario de transmisión | Tamaño dos datos | Sementeira dun só nodo | Sementeira simultánea P2P de dous nodos | Aumentar o multiplicador |
| :--- | :---: | :---: | :---: | :---: |
| 500 arquivos pequenos | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Paquetes de código e arquivo (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Ficheiro de vídeo (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Medición real da liña dedicada transcontinental de alta velocidade (Europa → América do Norte, atraso 86 ms)

| Escenario de transmisión | Tamaño dos datos | `sync0` | `rsync` | Aumentar o multiplicador |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 arquivos pequenos | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Paquetes de código e arquivo (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Ficheiro de vídeo (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Sincronización incremental (modificar o contido do 1 %) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Instalar

### Método 1: Instalar a través de Cargo

```bash
cargo install sync0
```

### Método 2: Descargar binarios precompilados

Vai a [Versións de GitHub](https://github.com/webc-site/sync0/releases) para descargar o paquete comprimido `.tar.xz` para o sistema correspondente (compatible con Linux, macOS, Windows, Android, FreeBSD, NetBSD), descomprime e colócao no directorio `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Comeza rapidamente

`sync0` usa `~/.config/sync0` como directorio de configuración por defecto (se precisa cambiar a outros directorios, pode engadir `-c <directory>`).

### 1. Consulta o enderezo do nodo local e a configuración da tarxeta de visita

```bash
# Ver o enderezo do nodo local (a chave xérase automaticamente en ~/.config/sync0 por primeira vez)
sync0 id

# Xera unha tarxeta de visita de nodo persoal (admite nome de usuario, cidade, correo electrónico, sitio web e sinatura en 256 palabras)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Publica varios grupos para compartir ficheiros e establece os permisos dos membros do grupo (`.nt` Configuración)

`sync0` está configurado no formato [NestedText](https://nestedtext.org) (`.nt`). Cada usuario ten un ficheiro de configuración `.nt` separado. Podes especificar o directorio do perfil de usuario mediante `users_dir` en `config.nt` (que inclúe automaticamente todos os ficheiros `.nt` deste directorio). Tanto o usuario como o prefixo de ruta que se permite cargar usan unha lista sen orde (`-`). **Cando o prefixo da ruta de carga non se enche, a carga no directorio raíz do recurso compartido está permitida por defecto**.

1. Crea ficheiros de configuración `.nt` separados para cada membro do directorio `~/.config/sync0/users/`:

`~/.config/sync0/users/alice.nt` (membro de só lectura, tarifa limitada `80MB`, `rate` admite unidades `GB`, `MB`, `KB`):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (membros de carga restrinxida: só se permite cargar nos directorios `uploads/bob` e `shared/common`):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Carga de membros predeterminada: cando se omite `write_prefix`, a carga no directorio raíz do recurso compartido está permitida de forma predeterminada):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. Configure o directorio de usuarios e varios grupos para compartir ficheiros en `~/.config/sync0/config.nt`:

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

Comezar a compartir o servizo:

```bash
# Inicio directo (carga automaticamente todas as configuracións de usuario .nt nos directorios ~/.config/sync0/config.nt e users_dir)
sync0 share --region SHA

# Ou publica rapidamente varios grupos para compartir e autoriza os membros do grupo directamente a través da liña de comandos
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Extrae todo o contido ou selecciona varios ficheiros e directorios (`sync0 pull`)

Conéctese a calquera nodo de pares para obter automaticamente todos os nodos de semente dispoñibles no grupo de uso compartido e tira de varios nodos de semente ao mesmo tempo:

```bash
# Tire todo o grupo de compartición do conxunto de datos ao local ./local_data
sync0 pull dataset:// ./local_data -p <enderezo de nodo>

# Sincronización de selección múltiple: tira simultáneamente o único ficheiro docs/readme.md e subdirectorios activos/iconas, src/core
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <enderezo de nodo>
```

### 4. Carga un ficheiro ou directorio a un grupo para compartir (`sync0 push`)

Os membros con permisos de carga (`rw` ou `admin`) poden cargar ficheiros ou directorios locais a grupos de uso compartido remoto:

```bash
# Membros con permiso de carga do directorio raíz: carga directamente no directorio raíz compartido
sync0 push team-vault:// ./new_release -p <enderezo de nodo>

# Só os membros autorizados a usar un prefixo específico (como cargas/bob): carga ao prefixo de camiño permitido
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <enderezo de nodo>
```

### 5. Subscríbete á sincronización e semente automáticas (`sync0 sub`)

Despois de subscribirse ao grupo de compartición dun determinado nodo, o servidor local é tanto un cliente receptor como un novo servidor de semente; cando se actualiza o ficheiro do nodo de orixe, sincronízase automaticamente e ofrece descargas fragmentadas para outros nodos:

```bash
# Subscríbete a todo o grupo de compartición e semente automaticamente
sync0 subdataset:// ./local_mirror -p <enderezo de nodo>

# Só a selección múltiple para subscribirse a ficheiros e subdirectorios especificados e sementes
sync0 subdataset://models/llama.onnx,configs ./local_mirror -p <enderezo de nodo>
```

### 6. Sincronización de pares de grupos de varias máquinas (`sync0 group`)

Varias máquinas únense ao mesmo grupo de uso compartido, unhas outras descobren automaticamente os nós de semente dispoñibles e mantén a sincronización bidireccional (soporta a sincronización completa ou a selección múltiple de ficheiros e directorios):

```bash
# Únete completamente ao grupo e sincronización bidireccional
sync0 group team-docs:// ./docs -p <enderezo de nodo>

# Só o directorio wal e o ficheiro meta.json no grupo de sincronización de selección múltiple
sync0 group team-docs://wal,meta.json ./docs -p <enderezo de nodo>
```

### 7. Transmisión directa punto a punto entre dúas máquinas (`send` / `recv`)

Non é necesario preconfigurar un grupo e pode transferir temporalmente ficheiros ou directorios directamente entre dúas máquinas:

```bash
# Monitorización do receptor
sync0 recv 0.0.0.0:9000 ./backup

# O extremo de envío transfire directamente ficheiros ou directorios (opcional --delete mirror para limpar ficheiros en exceso, --rate límite de velocidade)
sync0 send ./project 192.168.1.10:9000
```

### 8. Cambio multilingüe

`sync0` admite 75 idiomas e cambia automaticamente segundo o ambiente de idioma do sistema por defecto; tamén se pode especificar manualmente mediante `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```