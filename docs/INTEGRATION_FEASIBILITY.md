# Estudo de Viabilidade Técnica e Arquitetura de Protocolo: Discord RS

**Autor:** Nexus (Engenheiro de Protocolo, Estado e Modelos)  
**Projeto:** Discord RS (Cliente Desktop Nativo em Rust + GPUI)  
**Status:** Aprovado para Implementação  
**Data:** 2026-09-22  

---

## 1. Sumário Executivo

O objetivo do **Discord RS** é fornecer um cliente desktop de altíssimo desempenho, com inicialização instantânea, baixo consumo de memória RAM (< 80 MB vs. > 500 MB do Electron) e renderização fluida a 120+ FPS via GPU usando o framework **GPUI** (criado pela Zed Industries).

Para garantir estabilidade, desacoplamento e testabilidade, a arquitetura foi dividida em:
- **`domain`**: Modelos puros e imutáveis representando entidades do Discord (Usuários, Servidores, Canais, Mensagens, Reações, Voz).
- **`protocol`**: Camada assíncrona de transporte baseada na trait `DiscordTransport`, suportando múltiplos provedores:
  - `MockDiscordProvider`: Provedor determinístico e offline para desenvolvimento ágil da interface GPUI.
  - `DiscordApiClient`: Cliente de produção integrando REST API v10 e Gateway WebSocket v10 em tempo real.
- **`auth`**: Gestão segura de credenciais com permissões Unix restritas (`0600`) e mascaramento de segredos em logs.

Este documento avalia a viabilidade técnica de integração com a API oficial do Discord, conformidade jurídica com os Termos de Serviço (ToS), detalhes operacionais do Gateway v10 e a especificação do protocolo de transmissão de voz e áudio via UDP/RTP.

---

## 2. Termos de Serviço (ToS) e Considerações Legais

### 2.1. Tokens de Bot vs. Tokens de Usuário ("Self-bots")

A Discord Inc. impõe distinções rígidas entre tipos de contas em seus [Termos de Serviço para Desenvolvedores](https://discord.com/developers/docs/policies-and-agreements/developer-terms-of-service):

| Característica | Bot Accounts (Recomendado) | User Accounts ("Self-botting") |
| :--- | :--- | :--- |
| **Conformidade com ToS** | **100% Permitido e Oficial** | **Expressamente Proibido** (Violação do ToS) |
| **Autenticação** | Cabeçalho `Authorization: Bot <token>` | Cabeçalho `Authorization: <token>` |
| **Riscos** | Sem risco de suspensão por uso normal | **Suspensão imediata e banimento de conta** |
| **Vetor de Detecção** | Nenhum (tráfego oficial esperado) | TLS Fingerprinting, Cloudflare Turnstile, Telemetria |
| **Escopo de Acesso** | Servidores onde o Bot foi adicionado | Todos os servidores e DMs do usuário |

#### Vetores de Detecção de Clientes Alternativos de Usuário:
1. **TLS / JA3 / JA4 Fingerprinting:** A biblioteca `reqwest` / `rustls` produz uma ordem de Cipher Suites e extensões TLS distinta do Chromium/Electron oficial. O Cloudflare da Discord detecta essa divergência e exige validação via Captcha/Turnstile.
2. **WebSocket Heartbeat & Jitter Heuristics:** O cliente oficial envia eventos analíticos específicos de telemetria ("Science events") periódicos pelo Gateway. Clientes terceiros que não emulam 100% dessa telemetria são classificados como autômatos.
3. **Restrições de API:** Endpoints sensíveis como leitura de DMs sem consentimento de app e listagem em massa de membros acionam triggers automáticos de fraude.

### 2.2. Diretriz Arquitetural de Conformidade do Discord RS
1. **Suporte Nativo a Bot Tokens:** O Discord RS trata tokens de Bot como cidadãos de primeira classe. Desenvolvedores e comunidades podem utilizá-lo como painel administrativo de alta performance e interface de monitoramento.
2. **Isolamento de Transporte via Trait:** Toda a UI do GPUI comunica-se exclusivamente com a trait assíncrona `DiscordTransport`. O mock determinístico (`MockDiscordProvider`) permite o desenvolvimento completo da aplicação sem necessidade de conexão com a API de produção.
3. **Transparência e Avisos ao Usuário:** A estrutura `AuthToken` e a documentação alertam explicitamente sobre os riscos caso o usuário opte por configurar uma credencial de usuário pessoal.

---

## 3. Arquitetura da REST API v10

A REST API v10 é utilizada para operações pontuais de leitura histórica, envio de mensagens e mutação de estado.

### 3.1. Identificadores Snowflake de 64 bits
Todas as entidades do Discord (usuários, mensagens, canais, servidores) utilizam identificadores **Snowflake** compostos por 64 bits:
- **Timestamp (42 bits):** Milissegundos desde a Epoch do Discord (`1420070400000` - 01/01/2015).
- **Worker ID interno (5 bits)**
- **Process ID interno (5 bits)**
- **Incremento sequencial (12 bits)**

> **Atenção Técnica em Rust:** Em JavaScript/JSON, números acima de $2^{53} - 1$ sofrem perda de precisão IEEE-754. O Discord envia Snowflakes como strings no JSON. No Discord RS, modelamos esses campos explicitamente como `String` nos tipos de domínio (`id: String`), garantindo integridade e interoperabilidade sem overflow.

### 3.2. Rate Limiting e Gestão de Buckets
O Discord utiliza um sistema de rate limiting baseado em buckets retornados via cabeçalhos HTTP:
- `X-RateLimit-Limit`: Número máximo de requisições na janela.
- `X-RateLimit-Remaining`: Requisições restantes na janela atual.
- `X-RateLimit-Reset`: Timestamp UTC Unix do reset da janela.
- `X-RateLimit-Reset-After`: Tempo restante em segundos (formato float).
- `X-RateLimit-Bucket`: Hash identificador único da rota/recurso.

Em caso de `HTTP 429 Too Many Requests`, o cliente lê o campo JSON `retry_after` e suspende novas requisições para o bucket correspondente com jitter aleatório para evitar stampedes.

---

## 4. Arquitetura do Discord Gateway v10 (WebSocket)

O Gateway v10 provê o feed bidirecional em tempo real de mensagens, status de presença e sinalização de chamadas de voz.

```
+--------------------------------------------------------------------------+
|                           DISCORD GATEWAY v10                            |
+--------------------------------------------------------------------------+
       |                                                    ^
       | 1. Op 10 HELLO (heartbeat_interval_ms)             |
       v                                                    |
+-----------------------+                                   |
|   Client Connection   |                                   |
+-----------------------+                                   |
       |                                                    |
       | 2. Op 2 IDENTIFY (Token, Intents, Properties)      |
       v                                                    |
+-----------------------+                                   |
|  Gateway Verification |                                   |
+-----------------------+                                   |
       |                                                    |
       | 3. Op 0 DISPATCH (READY: user, guilds, session_id) |
       v                                                    |
+-------------------------------------------------------------------+
|                        ESTABLISHED SESSION                        |
|                                                                   |
|   Loop Periódico:                                                 |
|   Client  --[ Op 1 Heartbeat (seq) ]--------------------------->  |
|   Gateway <--[ Op 11 Heartbeat ACK ]----------------------------  |
|                                                                   |
|   Eventos em Tempo Real:                                          |
|   Gateway --[ Op 0 MESSAGE_CREATE ]---------------------------->  |
|   Gateway --[ Op 0 REACTION_ADD ]------------------------------>  |
|   Gateway --[ Op 0 VOICE_STATE_UPDATE ]------------------------>  |
+-------------------------------------------------------------------+
```

### 4.1. Ciclo de Vida da Conexão e Opcodes
1. **Conexão Inicial:** Conecta a `wss://gateway.discord.gg/?v=10&encoding=json`.
2. **Op 10 Hello:** O servidor envia o intervalo de batimentos cardíacos (`heartbeat_interval`).
3. **Op 2 Identify:** O cliente envia payload contendo:
   - `token`: Credencial de autenticação.
   - `intents`: Bitmask declarando os eventos que o cliente deseja escutar.
   - `properties`: Dados da plataforma (`$os: "linux"`, `$browser: "discord-rs"`).
4. **Heartbeat Loop (Op 1 & Op 11):**
   - O cliente dispara periodicamente `{"op": 1, "d": sequence}`.
   - Se o servidor não responder com `Op 11 (Heartbeat ACK)` antes do próximo tick, a conexão é declarada "zumbi", abortada e restabelecida.
5. **Op 6 Resume:** Caso a conexão caia temporariamente, o cliente reconecta enviando `session_id` e o último `seq` recebido, evitando o reenvio dispendioso de todo o estado do servidor.

### 4.2. Gateway Intents
Para receber o conteúdo textual das mensagens, o cliente precisa habilitar o **Message Content Intent**:
- `GUILDS` (1 << 0)
- `GUILD_MESSAGES` (1 << 9)
- `MESSAGE_CONTENT` (1 << 15) *(Intent Privilegiado para Bots em mais de 100 servidores)*
- `GUILD_VOICE_STATES` (1 << 7)

---

## 5. Protocolo de Voz (WebRTC, Voice Gateway e Áudio UDP)

O protocolo de voz do Discord opera em duas camadas desacopladas: sinalização e mídia.

```
+-----------------------------------------------------------------------+
|                         SINALIZAÇÃO DE VOZ                            |
+-----------------------------------------------------------------------+
 1. Client envia Op 4 Voice State Update no Gateway Principal
 2. Gateway Principal envia VOICE_SERVER_UPDATE (endpoint, token)
 3. Client abre WebSocket dedicado para wss://<endpoint>/?v=8
 4. Voice Op 0 IDENTIFY (server_id, user_id, session_id, token)
 5. Voice Op 2 READY (SSRC, IP do servidor de voz, porta UDP, ciphers)
                                 |
                                 v
+-----------------------------------------------------------------------+
|                    DESCOBERTA DE IP & NEGOCIAÇÃO                      |
+-----------------------------------------------------------------------+
 6. Client envia pacote UDP de 74 bytes para descobrir IP público e porta NAT
 7. Voice Op 1 SELECT PROTOCOL (IP público, porta, cipher: aead_xchacha20_poly1305_rtpsize)
 8. Voice Op 4 SESSION DESCRIPTION (chave secreta de 32 bytes)
                                 |
                                 v
+-----------------------------------------------------------------------+
|                     STREAMING DE ÁUDIO RTP / UDP                      |
+-----------------------------------------------------------------------+
 9. Voice Op 5 SPEAKING (flags: 1 = microfone ativo)
 10. Captura de microfone via CPAL (48kHz estéreo, frames de 20ms = 960 amostras)
 11. Codificação Opus (audiopus / opus crate)
 12. Enquadramento RTP (Header de 12 bytes + Payload encriptado ChaCha20-Poly1305)
 13. Envio contínuo via UDP Socket de alta prioridade
```

### 5.1. Criptografia de Áudio
O Discord exige criptografia AEAD em todos os pacotes de voz UDP. Os modos modernos obrigatórios são:
- `aead_xchacha20_poly1305_rtpsize` (Padrão ouro de segurança e performance).
- `aead_aes256_gcm_rtpsize`.

### 5.2. Ecossistema Rust para Áudio
A implementação completa do pipeline de voz no Discord RS é plenamente viável utilizando crates maduras do ecossistema Rust:
- **`cpal`**: Abstração cross-platform de hardware de áudio (suporta ALSA, PulseAudio e PipeWire no Linux, CoreAudio no macOS e WASAPI no Windows).
- **`audiopus`** ou **`opus`**: Bindings para a libopus oficial, garantindo codificação de ultra-baixa latência (frames de 20ms).
- **`aws-lc-rs`** ou **`chacha20poly1305`**: Criptografia de alta velocidade com aceleração SIMD/AVX2 para os pacotes RTP.
- **`tokio::net::UdpSocket`**: I/O de rede não-bloqueante para envio e recepção de datagramas UDP.

---

## 6. Integração com a UI do GPUI

O **GPUI** utiliza um loop de eventos de renderização imediata com sincronização em GPU. Para garantir fluidez visual (60-120+ FPS) e evitar congelamento da tela:

```
[ Thread de Rede (Tokio) ]                 [ Thread de UI (GPUI Main Loop) ]
       |                                                   |
       |-- Gateway WebSocket Loop                          |
       |   (Recebe evento MESSAGE_CREATE)                  |
       |                                                   |
       |-- tokio::sync::broadcast::Sender                  |
       |                 |                                 |
       |                 v (Event channel)                 |
       |       cx.spawn(|this, mut cx| {                   |
       |           while let Ok(event) = rx.recv() {       |
       |               cx.update(|model, cx| { ----------->| Renderiza frame novo
       |                   model.append_message(msg);      | com animação suave!
       |                   cx.notify();                    |
       |               });                                 |
       |           }                                       |
       |       })                                          |
```

1. **Separação de Threads:** Toda a deserialização JSON, conexões WebSocket, renovação de tokens e I/O de rede ocorrem em threads dedicadas do Tokio.
2. **Canais de Broadcast:** O `DiscordTransport` emite variantes da enum `TransportEvent` (`MessageCreate`, `ReactionAdd`, `VoiceStateUpdate`, etc.).
3. **Reatividade GPUI:** Os modelos de visão observáveis assinam o canal via `cx.spawn` e notificam a interface com `cx.notify()`, atualizando os nós visuais com custo mínimo de diffing.

---

## 7. Roteiro de Implementação Recomendado

1. **Fase 1: Fundação de Domínio e UI Offline (Concluída)**
   - Modelos em `src/domain/` (`User`, `Guild`, `Channel`, `Message`, `Voice`).
   - Provedor determinístico `MockDiscordProvider` com dados realistas dos servidores *Rust Brasil* e *GPUI Hub*.
   - Gestão de credenciais `AuthToken` e persistência segura em `src/protocol/auth.rs`.
2. **Fase 2: Conexão Live com o Gateway e REST (Em andamento)**
   - Validação da autenticação via REST v10.
   - Sincronização de mensagens em tempo real através do WebSocket loop em `src/protocol/discord_api.rs`.
3. **Fase 3: Transmissão de Voz e Chamadas**
   - Integração da captura de áudio via `cpal`.
   - Handshake do Voice Gateway v8 e streaming de pacotes UDP/RTP com criptografia ChaCha20-Poly1305.

---

## 8. Conclusão

A arquitetura estabelecida nos módulos `src/domain/` e `src/protocol/` atende plenamente aos requisitos de engenharia do Discord RS. O isolamento garantido pela trait `DiscordTransport` oferece segurança técnica e jurídica, permitindo uma transição fluida entre prototipação determinística de interface com GPUI e comunicação de alta performance com a infraestrutura oficial do Discord.
