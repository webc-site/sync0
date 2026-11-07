# sync0

`sync0` é uma ferramenta de linha de comando para transferência de arquivos entre redes, sincronização incremental de diretórios e distribuição simultânea de dados para várias máquinas.

## Intenção de design original

Em salas entre máquinas, países ou ambientes com atrasos de rede e perda de pacotes, ferramentas como `rsync` e `scp` baseiam-se na confirmação de sequência de conexão única, o que limita a utilização da largura de banda; quando várias máquinas extraem dados do mesmo servidor ao mesmo tempo, a placa de rede do servidor de origem pode facilmente se tornar um gargalo.

O objetivo de `sync0` é resolver dois problemas:

1. Deixe a sincronização de arquivos e diretórios entre as duas máquinas executar largura de banda física total e só transfira a parte alterada quando o arquivo for parcialmente modificado.
2. Várias máquinas podem transferir blocos de dados entre si ao sincronizar o mesmo diretório, reduzindo a carga no servidor de origem e diminuindo o tempo geral de sincronização.

## Comparação de velocidade com rsync

Ambiente de teste: nó europeu $\leftrightarrow$ nó norte-americano, atraso de ida e volta de 86 ms, medindo o tempo total, incluindo verificação de diretório, comparação de desduplicação, transmissão de rede e verificação de posicionamento de disco:

| Cenário de transmissão | Tamanho dos dados | `sync0` | `rsync` | múltiplo |
| :--- | :---: | :---: | :---: | :---: |
| 10.000 arquivos pequenos | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| Pacotes de código e arquivo (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| Arquivo de vídeo (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| Sincronização incremental (modificar 1% do conteúdo) | 512 MB | 0.12 s | 1.85 s | 15.4x |

## Métodos de melhoria de desempenho

- **Transmissão simultânea UDP**: Envie blocos de dados por meio de UDP e retransmita apenas fragmentos perdidos para evitar que uma única perda de pacote bloqueie transferências de arquivos subsequentes.
- **Segmentação e desduplicação de conteúdo**: divida o arquivo em blocos de dados de acordo com o conteúdo e calcule o valor de verificação. O mesmo conteúdo já recebido é reutilizado diretamente localmente, sem duplicar a rede.
- **Ajuste automaticamente a compactação de acordo com a velocidade da rede**: ative a compactação para conteúdo compactável, como texto e código; pula automaticamente a compactação de vídeos, fotos, pacotes compactados ou em LAN de alta velocidade para reduzir o consumo de CPU.
- **Extração próxima de vários nós**: quando várias máquinas extraem o mesmo diretório, os blocos de dados serão extraídos simultaneamente de nós baixados na mesma cidade ou com menor latência.

## Instalar

### Método 1: Instalar via Cargo

```bash
cargo install sync0
```

### Método 2: Baixe binários pré-compilados

Acesse [GitHub Releases](https://github.com/webc-site/sync0/releases) para baixar o pacote compactado `.tar.xz` para o sistema correspondente (suporta Linux x86_64/ARM64, macOS Apple Silicon/Intel, Windows x64/ARM64, Android ARM64/x86_64, FreeBSD, NetBSD). Após descompactar, coloque `sync0` no diretório `PATH`:

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## Guia do usuário

### 1. Transfira arquivos ou diretórios diretamente entre duas máquinas

Inicie o monitoramento na máquina receptora e especifique o caminho de recebimento e salvamento:

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

Envie um único arquivo ou um diretório inteiro da máquina remetente:

```bash
sync0 send ./project 192.168.1.10:9000
```

Parâmetros comumente usados:

```bash
# Exclua arquivos redundantes que existem no terminal de recebimento, mas não no terminal de envio (sincronização de espelho)
sync0 send ./project 192.168.1.10:9000 --delete

# Limitar taxa de envio (unidade: bytes/segundo, por exemplo 50 MB/s)
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# Modo silencioso (nenhuma barra de progresso mostrada)
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. Publicação de diretórios compartilhados e extração de vários nós

Publique um diretório compartilhado (denominado `dataset`) no servidor de origem:

```bash
sync0 share dataset ./data --region SHA
```

Extraia o diretório compartilhado em outras máquinas (você pode preencher vários endereços de nós ao mesmo tempo, separados por vírgulas para extrair simultaneamente):

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

Extraia apenas um determinado subdiretório no diretório compartilhado:

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

Envie as alterações locais de volta para o diretório compartilhado remoto:

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. Assine as atualizações automáticas do catálogo

Assine o diretório compartilhado do nó de origem. Quando um arquivo é adicionado ou modificado no diretório de origem, ele será automaticamente sincronizado com o computador local. Ao mesmo tempo, a máquina local servirá automaticamente como um nó para outras máquinas extrairem:

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. Sincronização bidirecional de grupos de várias máquinas

Várias máquinas ingressam no mesmo nome de grupo e as alterações em qualquer diretório de nó serão automaticamente sincronizadas com outros nós do grupo:

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. Identidade do nó e cartão de visita pessoal

Cada nó suporta a configuração do nome de usuário, cidade, e-mail, site e uma assinatura personalizada (máximo de 256 caracteres), e passa pela autenticação de assinatura de chave privada local:

```bash
# Visualize o ID da chave pública local (especifique o diretório de configuração para persistir a chave)
sync0 id -c ~/.config/sync0

# Gere e exiba cartões de visita de nós com assinaturas personalizadas
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

`share`, `pull`, `sub`, `group` também podem ser iniciados com estes parâmetros de perfil:

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. Comutação multilíngue

`sync0` Suporta 75 idiomas. Por padrão, a ajuda da linha de comando e os prompts no idioma correspondente são exibidos automaticamente de acordo com o ambiente de idioma do sistema operacional; se o idioma do sistema não estiver na lista de suporte, o inglês será usado por padrão.

Para especificar o idioma manualmente, defina a variável de ambiente `SYNC0_LANG`:

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```