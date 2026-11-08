# sync0

`sync0` é uma ferramenta de compartilhamento de arquivos ponto a ponto (P2P) e sincronização de várias máquinas que compete com o Resilio Sync.

Cada máquina é um servidor e um cliente: qualquer nó pode publicar vários grupos de compartilhamento de arquivos. Após a adesão de outros nós, eles podem obter automaticamente os nós iniciais disponíveis do servidor peer e extraí-los simultaneamente de várias fontes. Após a conclusão da sincronização, as sementes serão criadas automaticamente para mais nós para acelerar os downloads.

## Destaques principais

- **Todo mundo é servidor e cliente**: comparável ao Resilio Sync, sem dependência de servidor central. Qualquer nó pode criar vários grupos de compartilhamento de arquivos e também ingressar em grupos de compartilhamento de outras pessoas; os nós trocam e obtêm automaticamente os nós iniciais disponíveis para obter extração simultânea e sincronização em tempo real de várias máquinas.
- **Endereço de nó sem IP**: por padrão, a chave do nó e o endereço `base64url` codificado em `iroh` são gerados automaticamente em `~/.config/sync0` sem a necessidade de lembrar ou expor o IP público.
- **Sincronização de seleção múltipla sob demanda**: oferece suporte à sincronização de todo o grupo de compartilhamento e também à sincronização sob demanda, selecionando vários arquivos ou subdiretórios ao mesmo tempo.
- **Permissões hierárquicas de membros do grupo e controle de diretório de upload**: suporta permissões somente leitura (`ro`), upload de leitura e gravação (`rw`) e gerenciamento (`admin`), e pode especificar individualmente o prefixo do caminho que cada membro tem permissão para fazer upload (por padrão, é permitido fazer upload para o diretório raiz do compartilhamento).
- **Sincronização incremental e em velocidade total entre redes**: transmissão simultânea de arquivos grandes, agregação de arquivos pequenos e massivos na memória e blocos de dados sincronizados apenas quando os arquivos são parcialmente modificados.

## Comparação de velocidade com rsync

### Medição real de fonte única da rede transoceânica (China → Estados Unidos, atraso de 260 ms)

| Cenário de transmissão | Tamanho dos dados | modelo | `sync0` | `rsync -az` | Aumentar multiplicador |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 arquivos pequenos | 1.91 MB | Sincronização completa | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 arquivos pequenos | 1.91 MB | Sincronização incremental | 27.15 Mbps | 3.83 Mbps | 7.1x |
| Pacotes de código e arquivo (`.tar`) | 10.0 MB | Sincronização completa | 139.70 Mbps | 24.97 Mbps | 5.6x |
| Pacotes de código e arquivo (`.tar`) | 10.0 MB | Sincronização incremental | 83.38 Mbps | 26.02 Mbps | 3.2x |
| Arquivo de vídeo (`.mp4`) | 15.0 MB | Sincronização completa | 54.06 Mbps | 27.04 Mbps | 2.0x |
| Arquivo de vídeo (`.mp4`) | 15.0 MB | Sincronização incremental | 216.83 Mbps | 21.51 Mbps | 10.1x |

### Teste de tração de vários nós P2P da rede transoceânica (China → Estados Unidos, nós duplos, atraso de 260 ms)

Comparação medida de pull simultâneo distribuído de nó de semente de múltiplas fontes:

| Cenário de transmissão | Tamanho dos dados | Sementeira de nó único | Sementeira simultânea P2P de dois nós | Aumentar multiplicador |
| :--- | :---: | :---: | :---: | :---: |
| 500 arquivos pequenos | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| Pacotes de código e arquivo (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| Arquivo de vídeo (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### Medição real da linha dedicada transcontinental de alta velocidade (Europa → América do Norte, atraso de 86 ms)

| Cenário de transmissão | Tamanho dos dados | `sync0` | `rsync` | Aumentar multiplicador |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 arquivos pequenos | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| Pacotes de código e arquivo (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| Arquivo de vídeo (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| Sincronização incremental (modificar 1% do conteúdo) | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## Instalar

### Método 1: Instalar via Cargo

```bash
cargo install sync0
```

### Método 2: Baixe binários pré-compilados

Acesse [GitHub Releases](https://github.com/webc-site/sync0/releases) para baixar o pacote compactado `.tar.xz` para o sistema correspondente (suporta Linux, macOS, Windows, Android, FreeBSD, NetBSD), descompacte e coloque-o no diretório `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Comece rapidamente

`sync0` usa `~/.config/sync0` como diretório de configuração por padrão (se precisar mudar para outros diretórios, você pode adicionar `-c <directory>`).

### 1. Veja o endereço do nó local e configure o cartão de visita

```bash
# Veja o endereço do nó local (a chave é gerada automaticamente em ~/.config/sync0 pela primeira vez)
sync0 id

# Gere um cartão de visita de nó pessoal (suporta nome de usuário, cidade, e-mail, site e assinatura em até 256 palavras)
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. Publique vários grupos de compartilhamento de arquivos e defina permissões de membros do grupo (`.nt` Configuração)

`sync0` é configurado no formato [NestedText](https://nestedtext.org) (`.nt`). Cada usuário possui um arquivo de configuração `.nt` separado. Você pode especificar o diretório do perfil do usuário através de `users_dir` em `config.nt` (que inclui automaticamente todos os arquivos `.nt` neste diretório). Tanto o usuário quanto o prefixo do caminho com permissão para upload usam uma lista não ordenada (`-`). **Quando o prefixo do caminho de upload não é preenchido, o upload para o diretório raiz do compartilhamento é permitido por padrão**.

1. Crie arquivos de configuração `.nt` separados para cada membro no diretório `~/.config/sync0/users/`:

`~/.config/sync0/users/alice.nt` (membro somente leitura, taxa limitada `80MB`, `rate` suporta unidades `GB`, `MB`, `KB`):

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt` (membros de upload restritos: só é permitido fazer upload para os diretórios `uploads/bob` e `shared/common`):

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt` (Upload de membro padrão: quando `write_prefix` é omitido, o upload para o diretório raiz do compartilhamento é permitido por padrão):

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. Configure o diretório de usuários e vários grupos de compartilhamento de arquivos em `~/.config/sync0/config.nt`:

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

Comece a compartilhar o serviço:

```bash
# Inicialização direta (carrega automaticamente todas as configurações de usuário .nt nos diretórios ~/.config/sync0/config.nt e users_dir)
sync0 share --region SHA

# Ou publique rapidamente vários grupos de compartilhamento e autorize os membros do grupo diretamente pela linha de comando
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. Extraia todo o conteúdo ou selecione vários arquivos e diretórios (`sync0 pull`)

Conecte-se a qualquer nó peer para obter automaticamente todos os nós iniciais disponíveis no grupo de compartilhamento e extrair de vários nós iniciais simultaneamente:

```bash
# Puxe todo o grupo de compartilhamento do conjunto de dados para o local ./local_data
sync0 pull dataset:// ./local_data -p <endereço do nó>

# Sincronização de seleção múltipla: extraia simultaneamente o arquivo único docs/readme.md e os subdiretórios assets/icons, src/core
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <endereço do nó>
```

### 4. Carregar um arquivo ou diretório para um grupo de compartilhamento (`sync0 push`)

Membros com permissões de upload (`rw` ou `admin`) podem fazer upload de arquivos ou diretórios locais para grupos de compartilhamento remoto:

```bash
# Membros com permissão de upload do diretório raiz: faça upload diretamente para o diretório raiz compartilhado
sync0 push team-vault:// ./new_release -p <endereço do nó>

# Somente membros autorizados para um prefixo específico (como uploads/bob): faça upload para o prefixo de caminho permitido
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <endereço do nó>
```

### 5. Assine a sincronização e propagação automáticas (`sync0 sub`)

Depois de se inscrever no grupo de compartilhamento de um determinado nó, o servidor local é tanto um cliente receptor quanto um novo servidor de propagação; quando o arquivo do nó de origem é atualizado, ele é sincronizado automaticamente e fornece downloads em partes para outros nós:

```bash
# Inscreva-se em todo o grupo de compartilhamento e propague automaticamente
sync0 sub dataset:// ./local_mirror -p <endereço do nó>

# Somente seleção múltipla para assinar arquivos e subdiretórios especificados e propagá-los
sync0 sub dataset://models/llama.onnx,configs ./local_mirror -p <endereço do nó>
```

### 6. Sincronização peer de grupo de várias máquinas (`sync0 group`)

Várias máquinas ingressam no mesmo grupo de compartilhamento, umas às outras descobrem automaticamente os nós iniciais disponíveis e mantêm a sincronização bidirecional (suporta sincronização completa ou seleção múltipla de arquivos e diretórios):

```bash
# Junte-se totalmente ao grupo e sincronização bidirecional
grupo sincronizado team-docs:// ./docs -p <endereço do nó>

# Somente o diretório wal e o arquivo meta.json no grupo de sincronização de seleção múltipla
grupo sincronizado team-docs://wal,meta.json ./docs -p <endereço do nó>
```

### 7. Transmissão direta ponto a ponto entre duas máquinas (`send` / `recv`)

Não há necessidade de pré-configurar um grupo e você pode transferir temporariamente arquivos ou diretórios diretamente entre duas máquinas:

```bash
# Monitoramento do receptor
sync0 recv 0.0.0.0:9000 ./backup

# O remetente transfere diretamente arquivos ou diretórios (opcional --delete mirror para limpar arquivos em excesso, --rate speed limit)
sync0 send ./project 192.168.1.10:9000
```

### 8. Comutação multilíngue

`sync0` suporta 75 idiomas e alterna automaticamente de acordo com o ambiente de idioma do sistema por padrão; também pode ser especificado manualmente através de `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```