# sync0

`sync0` é uma ferramenta descentralizada de compartilhamento de arquivos P2P, sincronização em tempo real e distribuição multifonte usada para substituir `rsync` - a velocidade medida da rede transcontinental de longa distância é de até **6,95×** de `rsync` (velocidade média geométrica de ponto único **2,01×**, velocidade média geométrica de fonte dupla P2P **3,33×**) e a sincronização de arquivos pequenos e massivos entre oceanos é tão rápido quanto **7,19×**.

Cada máquina é um servidor e um cliente: não há necessidade de um servidor central ou um endereço de rede pública, a perfuração automática é baseada no endereço de chave pública da rede distribuída e suporta transmissão direta entre duas máquinas, hospedagem de permissão de grupo de compartilhamento múltiplo, seleção múltipla de subdiretórios sob demanda, sincronização incremental em tempo real em nível de milissegundos e aceleração simultânea de múltiplas fontes P2P.

## Instalar

### Método 1: Instalação de carga

```bash
cargo install sync0
```

### Método 2: Baixe binários pré-compilados

Acesse [GitHub Releases](https://github.com/webc-site/sync0/releases) para baixar o pacote compactado `.tar.xz` correspondente ao sistema (suporta Linux, macOS, Windows, Android, iOS, FreeBSD), descompacte e coloque em `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Comece rapidamente

### 1. Transmissão direta de configuração zero entre duas máquinas (`send` / `recv`)

Não há necessidade de configurar um grupo de compartilhamento, transferir arquivos ou diretórios diretamente entre duas máquinas em alta velocidade:

```bash
# A extremidade receptora escuta na porta e grava em ./backup
sync0 recv 0.0.0.0:9000 ./backup

# Transmissão direta do remetente (opcional --delete mirror para limpar arquivos em excesso, --rate limite de velocidade de 50 MB)
sync0 send ./project 192.168.1.10:9000
```

### 2. Compartilhamento de postagem e pull/upload sob demanda (`share` / `pull` / `push`)

```bash
# Servidor: publique o grupo de compartilhamento com uma linha de comando (gere o endereço do nó iroh sem IP após a inicialização)
sync0 share dataset:./data

# Cliente: extraia todo o grupo de compartilhamento do conjunto de dados para o local ./local_data
sync0 pull dataset:// ./local_data -p <endereço do nó>

# Cliente: Extraia múltiplas seleções sob demanda (sincronize apenas arquivos e subdiretórios únicos especificados)
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <endereço do nó>

# Cliente: Faça upload de arquivos ou diretórios para grupos de compartilhamento autorizados
sync0 push dataset://releases/v1 ./build_output -p <endereço do nó>
```

### 3. Propagação de assinatura em tempo real e sincronização bidirecional de várias máquinas (`sub` / `group`)

Depois de estabelecer uma conexão longa, se qualquer arquivo **adicionar, modificar, excluir, alterar permissões ou horário de modificação** ocorrer na extremidade remota, o diretório local será atualizado de forma síncrona em milissegundos em tempo real (arquivos grandes novos/modificados apenas extrairão os blocos de diferença e os atualizarão no lugar, os arquivos excluídos limparão automaticamente os arquivos locais e diretórios vazios e apenas alterarão as permissões ou carimbo de data e hora zero blocos de dados diretamente):

- **`sync0 sub`(Espelhamento unidirecional em tempo real + propagação automática)**: O tempo real local segue as adições, exclusões e modificações do final do compartilhamento e serve automaticamente como uma fonte de propagação P2P para fornecer aceleração simultânea para outros nós de extração.
- **`sync0 group`(sincronização bidirecional ponto a ponto de várias máquinas)**: se arquivos forem adicionados, excluídos ou modificados em qualquer máquina do grupo, todos os outros nós on-line serão atualizados de forma síncrona em tempo real e servirão como sementes uns para os outros.

```bash
# Assine todo o grupo de compartilhamento: adições, exclusões e modificações remotas são sincronizadas localmente em tempo real e propagadas automaticamente
sync0 sub dataset:// ./local_mirror -p <endereço do nó>

# Assinatura de seleção múltipla sob demanda: sincronize e propague apenas arquivos únicos e subdiretórios especificados em tempo real
sync0 sub dataset://models/llama.onnx,configs ./local_mirror -p <endereço do nó>

# Sincronização bidirecional em tempo real de grupo de pares de várias máquinas (também suporta caminhos de seleção múltipla completos ou vírgulas)
grupo sincronizado team-docs:// ./docs -p <endereço do nó>
```

### 4. Configuração de grupo de compartilhamento múltiplo da equipe e permissão de membros (configuração `.nt`)

O diretório de configuração padrão é `~/.config/sync0` (pode ser alternado via `-c <directory>`), usando o formato minimalista [NestedText](https://nestedtext.org) (`.nt`):
- **Suporta restrição de leitura de rede pública (somente usuários autorizados podem ler)**: contanto que o grupo de compartilhamento esteja configurado com uma lista de membros `users:` (ou a linha de comando especifique `--allow-r` / `--allow-rw` / `--allow-w` / `--auth`), o sistema **ativa automaticamente a autenticação privada (`auth: true`)** e proíbe a enumeração ou leitura de nós anônimos de rede pública e assinatura P2P aos nós de propagação também herdarão a lista de permissões simultaneamente, permitindo que apenas usuários autorizados leiam; grupos de compartilhamento que não estão configurados com `users` são públicos e somente leitura por padrão (se você quiser ser público e somente leitura e aberto para uploads apenas para membros específicos, você pode definir explicitamente `auth: false`).
- **Permissões refinadas de leitura-gravação e entrega de subdiretórios**: os membros são somente leitura por padrão (`r` não precisa ser escrito explicitamente) e suportam `r` (somente leitura), `w` (apenas gravação na caixa suspensa, não legível), `rw` (leitura-gravação) e `write_prefix` (limita o prefixo de subdiretório permitido para upload).

**Exemplo de configuração de membro (`~/.config/sync0/users/`):**

```yaml
# ~/.config/sync0/users/alice.nt (padrão somente leitura, limite de velocidade de 80 MB)
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt (permissões de leitura e gravação, permitido apenas fazer upload para o subdiretório especificado)
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt (apenas permissão de entrega de gravação, leitura é proibida)
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**Exemplo de configuração de grupo de compartilhamento (`~/.config/sync0/config.nt`):**

```yaml
users_dir: ./users

groups:
  # Usuários não configurados: rede pública padrão somente leitura
  dataset:
    path: ./data
    rate: 50MB

  # Usuários configurados: restringem automaticamente a leitura da rede pública (somente Alice e Bob podem ler, Charlie só pode entregar e fazer upload)
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

Inicie o serviço (carregue o arquivo de configuração ou use diretamente os parâmetros da linha de comando para restringir a leitura da rede pública e autorizar os membros):

```bash
# Carregue ~/.config/sync0/config.nt para iniciar
sync0 share --region SHA

# Ou publique um grupo de compartilhamento privado através de linha de comando pura (especifique --allow-* para restringir automaticamente a leitura da rede pública e apenas usuários autorizados podem acessá-lo)
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. Endereço do nó, cartão de visita e multilíngue

```bash
# Visualize o endereço do nó local sem IP (a chave é gerada automaticamente para a primeira execução)
sync0 id

# Gere cartão de visita do nó autoassinado Ed25519 (apelido, código da cidade, e-mail, site, introdução)
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# Existem 75 idiomas integrados, que seguem o idioma do sistema por padrão e podem ser alternados por meio de variáveis ​​de ambiente.
SYNC0_LANG=en sync0 --help
```

## Desempenho e recursos principais

### Medição real de WAN entre continentes (comparação `rsync`)

Ambiente de medição real: Estados Unidos `c1` → Europa `n1` (RTT 65 ms), o conjunto de dados é o kernel Linux `linux-7.3.3`:

#### 1) Modo à la carte vs `rsync`

| Conjunto de dados | tamanho | `sync0` Ponto único<br>(Mbps) | `rsync`<br>(Mbps) | Aceleração (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(não compactável) | 155 MB | **121.07** | 107.89 | **1,12×** |
| `linux-7.3.3.tar`(compressível) | 1.6 GB | **442.07** | 506.62 | **0,87×** |
| `linux-7.3.3.tar`(incremento de alteração local) | 1.6 GB | **8,123.87** | 1,169.49 | **6,95×** |
| `linux-7.3.3/` (95.979 arquivos pequenos) | 1.5 GB | **565.49** | 235.27 | **2,40×** |
| **média geométrica** | - | **704.17** | 350.19 | **2,01×** |

#### 2) Modo de fonte dupla P2P vs `rsync`

| Conjunto de dados | tamanho | `sync0` Fonte Dupla P2P<br>(Mbps) | `rsync`<br>(Mbps) | Aceleração (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`(não compactável) | 155 MB | **466.85** | 107.89 | **4,33×** |
| `linux-7.3.3.tar`(compressível) | 1.6 GB | **992.17** | 506.62 | **1,96×** |
| `linux-7.3.3.tar`(incremento de alteração local) | 1.6 GB | **5,666.84** | 1,169.49 | **4,85×** |
| `linux-7.3.3/` (95.979 arquivos pequenos) | 1.5 GB | **703.30** | 235.27 | **2,99×** |
| **média geométrica** | - | **1,165.63** | 350.19 | **3,33×** |

> Nota: No modo de fonte dupla P2P, os nós europeus `s1` colaboram para a propagação e o RTT do backbone intercontinental é de 65 ms.

### Principais recursos do produto

- **Endereçamento de chave pública com perfuração automática**: Não há necessidade de um servidor central e endereço de rede pública, perfuração NAT automática com base no endereço de chave pública da rede distribuída.
- **Sincronização em tempo real em nível de milissegundos e reconhecimento de metadados**: com base no monitoramento de eventos de arquivos nativos (`FSEvents` / `inotify` / `ReadDirectoryChangesW`), envio em tempo real de adições e exclusões de arquivos, permissões POSIX (`mode`) e tempo de modificação em nível de milissegundos (`mtime`); zero transmissão de bloco de dados quando apenas as permissões ou o tempo são modificados.
- **Download e upload simultâneos no estilo BT e agendamento de afinidade regional**: O nó de download não precisa esperar que o valor total seja baixado. Todos os intervalos de bloco (`Op::Have`) que passaram na verificação de hash de 128 bits são imediatamente bloqueados no pool de memória/temporário para outros nós extrairem simultaneamente. Juntamente com a troca dinâmica de nós PEX e a meia preempção de nós lentos (Work-Stealing), o rendimento do cluster é maximizado.
- **Imunidade anti-violação de nós mal-intencionados e envenenamento de cluster**: lista de fontes autorizadas `root_hash` Bloqueio autoconsistente + aceleração de hardware obrigatória `gxhash128` verificação de resumo antes de cada bloco ser colocado no disco e propagado externamente; se um bloco violado for encontrado, o disjuntor de nível de milissegundos bloqueará o nó malicioso e se recuperará automaticamente da fonte honesta.
- **Incremento in-situ FastCDC e agregação de arquivos pequenos**: somente blocos diferenciais são transferidos para modificação parcial de arquivos grandes e colocados no disco; o empacotamento massivo de memória de arquivos pequenos e a compactação diferencial de prefixo eliminam a sobrecarga de ida e volta arquivo por arquivo.
- **Autocorreção após queda de energia e retomada após reinicialização**: o formato híbrido RDB + AOF semelhante ao Redis (`.s0_subs.bin`) é usado para persistir o status de propagação e a lista de arquivos, as alterações diárias são anexadas em microssegundos e são automaticamente reescritas e organizadas regularmente. Depois que o servidor for reiniciado, as alterações off-line serão enviadas automaticamente para o nó de propagação.

## Arquitetura técnica e documentação

- **Protocolo de desacoplamento de plano duplo**:
  - **Controle e superfície de mudança em tempo real (`iroh` QUIC / TCP, primeiro byte `PROTO_VERSION` handshake de versão) **: Responsável pela verificação de cartão de visita Ed25519, verificação de permissão, descoberta de sementes Swarm e suporte para `u64::MAX` lote de streaming de mudança em massa de nível e `mzstd` `SubPush` compactado push em tempo real (`FsOp::Upsert` / `FsOp::Remove` / `FsOp::Meta` Três primitivas).
  - **Superfície de distribuição de dados (UDP nativo + `compio` E/S assíncrona) **: cabeçalho compacto de 20B, feedback periódico TACK de 18B, controle de congestionamento híbrido BBRv3/Copa, recuperação de perda de pacotes RACK-TLP, bypass de compressão Zstd adaptativo de entropia Shannon de primeiro bloco e `gxhash128` bloco de verificação ponta a ponta.
- **Saber mais**:
  - [Relatório de medição de desempenho e roteiro de reprodução](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [Projeto de arquitetura do sistema](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [Especificação de protocolo e persistência de fio binário](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [Autenticação de identidade, controle de autoridade e verificação de integridade](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)