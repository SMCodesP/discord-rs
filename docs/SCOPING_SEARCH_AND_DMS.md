# Especificação de Engenharia: Conversas Privadas (DMs), Pesquisa Global/Canal e Rotas de Backend
**Discord RS — Arquitetura de Domínio, Motor de Busca e Paridade com Discord Oficial**

- **Autor:** Engenheiro Especialista em Chat, Conversas Privadas (DMs), Mecanismo de Busca e Rotas de Backend
- **Documento:** `docs/SCOPING_SEARCH_AND_DMS.md`
- **Data:** Outubro de 2026
- **Status:** Aprovado para Implementação

---

## 1. Arquitetura de Conversas Privadas (Direct Messages & Group DMs)

O subsistema de conversas privadas do Discord opera sobre os tipos de canal `ChannelType::Dm` (Discord tipo 1) e `ChannelType::GroupDm` (Discord tipo 3). Diferente de canais em guildas/servidores, canais de DM não possuem permissões baseadas em cargos (roles), operando exclusivamente com base em listas de destinatários (`recipients: Vec<User>`).

### 1.1. Ciclo de Vida de DMs Individuais (1-on-1)
- **Abertura e Resolução (Cache First):**
  - Ao clicar em um perfil, amigo ou botão "Conversar", o cliente primeiro varre o cache local (`AppState::dm_channels`). Se já existir um canal com `channel_type == ChannelType::Dm` cujo destinatário corresponda ao alvo, seleciona-se o canal imediatamente sem latência de rede.
  - Caso não exista em memória, dispara-se chamada REST:
    - **Endpoint:** `POST /api/v10/users/@me/channels`
    - **Payload:** `{ "recipient_id": "<snowflake_id>" }`
    - **Resposta:** Objeto completo do canal (tipo 1), populando `last_message_id`, `recipients` e metadados.
- **Ocultação e Reabertura:**
  - `DELETE /api/v10/channels/{channel_id}`: Remove o canal da lista visível do usuário sem apagar mensagens no servidor. Caso qualquer um dos participantes envie uma nova mensagem (`MESSAGE_CREATE`), o canal reaparece automaticamente no topo da lista.

### 1.2. DMs em Grupo (Group DMs)
- **Regras Oficiais:**
  - Máximo de **10 participantes** por grupo (incluindo o criador).
  - Tipo de canal: `ChannelType::GroupDm` (código 3).
- **Criação e Adição de Participantes:**
  - **Criação direta:** `POST /api/v10/users/@me/channels` com `{ "access_tokens": [...], "nicks": { "<id>": "Apelido" } }`.
  - **Escalação de DM 1-on-1:** Ao adicionar um terceiro participante em uma DM individual existente, o Discord não altera a DM original; em vez disso, cria um novo `GroupDm` e preserva a DM original intacta.
  - **Inclusão em Grupo:** `PUT /api/v10/channels/{channel_id}/recipients/{user_id}`.
- **Gestão de Metadados:**
  - **Nome dinâmico:** Se nenhum nome customizado foi definido (`name.is_none()`), o cliente monta o título unindo os nomes dos participantes separados por vírgula (`"Alice, Bob, Carol"`).
  - **Nome customizado:** `PATCH /api/v10/channels/{channel_id}` com `{ "name": "Projeto Discord-RS" }`.
  - **Ícone do Grupo:** Suporte a upload de imagem customizada ou renderização de avatar composto (mosaico dos primeiros participantes).
  - **Remoção e Saída:**
    - Expulsão pelo proprietário (`owner_id`) ou saída voluntária: `DELETE /api/v10/channels/{channel_id}/recipients/{user_id}`.

### 1.3. Indicador de Digitação em Tempo Real com Coalescing
- **Evento de Entrada:** `TransportEvent::TypingStart { channel_id, user_id, timestamp }` (disparado pelo Gateway quando um usuário digita).
- **Comportamento e Coalescing:**
  - **Janela de TTL:** O Discord oficial impõe uma janela de expiração de **9 a 10 segundos** para o estado de digitação.
  - **Coalescing:** Múltiplos eventos `TYPING_START` do mesmo usuário em intervalos curtos (ex: a cada 4 segundos enquanto o usuário digita) apenas atualizam o timestamp do registro em `HashMap<UserId, Instant>`, evitando flickering na UI e múltiplos timers redundantes.
  - **Invalidação Reativa Instantânea:**
    - Ao receber `MESSAGE_CREATE` do respectivo usuário no canal, o estado de digitação desse usuário é **imediatamente removido**.
    - Ao trocar de canal ativo, a visualização limpa ou alterna para os digitadores do novo canal.
- **Formatação de Idioma Natural:**
  - 1 usuário: `"{Nome} está digitando..."`
  - 2 usuários: `"{Nome 1} e {Nome 2} estão digitando..."`
  - 3 usuários: `"{Nome 1}, {Nome 2} e {Nome 3} estão digitando..."`
  - 4+ usuários: `"Várias pessoas estão digitando..."`

### 1.4. Paginação de Mensagens e Pre-fetching
- **Rotas de Paginação da API Discord:**
  - `GET /api/v10/channels/{channel_id}/messages?limit=50&before={oldest_message_id}` (rolagem para o passado/topo).
  - `GET /api/v10/channels/{channel_id}/messages?limit=50&after={newest_message_id}` (rolagem para o futuro ao pular para resultado de busca).
  - `GET /api/v10/channels/{channel_id}/messages?limit=50&around={target_message_id}` (carregamento centrado em uma mensagem de busca ou citação).
- **Pre-fetching em Scroll Virtualizado:**
  - **Threshold:** Quando a janela visível no `gpui::list` atinge menos de **15 itens do topo** (índice visível `< 15`), dispara-se requisição assíncrona em background com `before={first_id}`.
  - **Preservação de Âncora (Scroll Anchoring):** A inserção de blocos antigos no topo não deve causar "pulo" visual para o usuário. O deslocamento do scroll deve ser compensado mantendo o elemento que estava no topo da viewport estático.
  - **Deduplicação Estrita:** A mesclagem no buffer de mensagens utiliza chave primária Snowflake, descartando duplicatas e garantindo ordem cronológica crescente.

---

## 2. Sistema de Pesquisa Avançada (Ctrl+F) e Paridade com Discord Oficial

O sistema de busca do Discord oficial oferece filtragem multifacetada em tempo real com sintaxe rica de operadores.

### 2.1. Sintaxe de Filtros Suportada

| Operador | Sintaxe | Descrição / Exemplo |
| :--- | :--- | :--- |
| `from:` | `from:usuario` ou `from:@usuario` | Filtra mensagens enviadas por um usuário específico |
| `mentions:` | `mentions:usuario` | Filtra mensagens que contêm menção a determinado usuário |
| `has:` | `has:link`, `has:embed`, `has:file`, `has:video`, `has:image`, `has:sound` | Filtra por presença de tipos específicos de mídia ou links |
| `in:` | `in:canal` ou `in:#canal` | Filtra mensagens dentro de um canal específico (útil em busca de guilda) |
| `before:` | `before:YYYY-MM-DD` | Mensagens postadas antes da data limite |
| `during:` | `during:YYYY-MM-DD` | Mensagens postadas exatamente naquele dia |
| `after:` | `after:YYYY-MM-DD` | Mensagens postadas após a data limite |
| `pinned:` | `pinned:true` ou `pinned:false` | Mensagens marcadas como fixadas no canal |
| *Texto Livre* | `termo1 termo2` | Busca por termos de texto no corpo da mensagem |

### 2.2. Rotas de Backend da API do Discord

| Rota / Endpoint | Método | Parâmetros de Query |
| :--- | :---: | :--- |
| `/api/v10/channels/{channel_id}/messages/search` | `GET` | `content`, `author_id`, `has`, `min_id`, `max_id`, `pinned`, `offset` |
| `/api/v10/guilds/{guild_id}/messages/search` | `GET` | `content`, `channel_id`, `author_id`, `has`, `min_id`, `max_id`, `pinned`, `offset` |

- **Paginação:** A API de busca do Discord utiliza paginação baseada em `offset` (em lotes de 25 mensagens por página: `offset=0`, `offset=25`, `offset=50`), diferente da paginação de histórico por cursor `before`/`after`.
- **Ordenação:** Por padrão decrescente por data (`sort_by=timestamp&sort_order=desc`).

### 2.3. Formato do Payload JSON Retornado pela API

A API do Discord agrupa os resultados em blocos contextuais. Cada item do vetor `messages` é uma lista de mensagens onde a mensagem que deu "match" possui `hit: true`, acompanhada de mensagens adjacentes (anteriores e posteriores) para contextualização visual na barra lateral de pesquisa:

```json
{
  "total_results": 42,
  "analytics_id": "9b1287c4a1e94473852077e6f8ad6651",
  "messages": [
    [
      {
        "id": "119876543210987650",
        "channel_id": "100000000000000001",
        "author": { "id": "200001", "username": "alice", "bot": false },
        "content": "Como configuro o banco de dados no Rust?",
        "timestamp": "2026-10-03T14:20:00Z"
      },
      {
        "id": "119876543210987651",
        "channel_id": "100000000000000001",
        "author": { "id": "200002", "username": "bob", "bot": false },
        "content": "Basta usar sqlx ou diesel com o Tokio runtime.",
        "timestamp": "2026-10-03T14:20:30Z",
        "hit": true
      },
      {
        "id": "119876543210987652",
        "channel_id": "100000000000000001",
        "author": { "id": "200001", "username": "alice", "bot": false },
        "content": "Perfeito, funcionou perfeitamente!",
        "timestamp": "2026-10-03T14:21:05Z"
      }
    ]
  ],
  "threads": [],
  "members": []
}
```

---

## 3. Arquitetura de Implementação em Rust

### 3.1. Módulo `src/chat/search.rs`
1. **Parser Sintático (`SearchQuery::parse`):**
   - Extração por regex ou tokenização linear de expressões como `from:user`, `has:link`, `in:geral`, `pinned:true`, `before:2026-10-01` e acumulação de termos livres.
2. **Filtros Estruturados (`HasFilter`):**
   - Identificação de URLs http/https, anexos de imagem/áudio/vídeo e embeds ricos.
3. **Motor de Busca em Memória (`SearchEngine::search_local`):**
   - Permite busca ultraveloz (< 1ms) sobre o buffer de mensagens locais do canal atual ou de todos os canais da guilda.
   - Gera grupos contextuais (`SearchResultGroup`) com a mensagem correspondente marcada como `hit` e mensagens vizinhas pré/pós para pré-visualização.
4. **Estrutura de Resultados da API REST:**
   - DTOs compatíveis com serde (`SearchPayload`, `SearchResultGroup`, `SearchGroupMessage`) para integração direta com `/messages/search`.

### 3.2. Integração com `AppState`
1. **Controle de Digitação Coalescida:**
   - Mapa `typing_tracker: HashMap<String, HashMap<String, (String, std::time::Instant)>>` (channel_id -> user_id -> (username, timestamp)).
   - Métodos: `record_typing(channel_id, user_id, username)`, `clear_typing(channel_id, user_id)`, `get_active_typing_text(channel_id) -> Option<String>`.
2. **Estado Reativo de Pesquisa:**
   - Campos: `search_query: String`, `is_search_open: bool`, `search_results: Vec<SearchResultGroup>`, `is_searching: bool`.
   - Métodos: `open_search()`, `close_search()`, `execute_search(query)`, `clear_search()`.
3. **Gerenciamento de DMs:**
   - Métodos auxiliares: `open_dm_with_user(user_id, username)`, `create_group_dm(user_ids, name)`.
