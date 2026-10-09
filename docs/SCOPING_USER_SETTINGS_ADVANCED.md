# Especificação de Engenharia: Configurações Avançadas de Usuário, Faturamento/Nitro e Conexões
**Discord RS — Arquitetura de Domínio e Interface com Paridade ao Discord Oficial**

- **Autor:** Engenheiro Especialista em Configurações Avançadas, Faturamento e Contas
- **Documento:** `docs/SCOPING_USER_SETTINGS_ADVANCED.md`
- **Data:** Outubro de 2026
- **Status:** Aprovado para Implementação

---

## 1. Mapeamento de Funcionalidades Profundas

Esta especificação aborda os subsistemas de conta, faturamento, integrações externas e personalização profunda de acessibilidade e renderização, atingindo 100% de paridade com o cliente oficial do Discord conectado.

### 1.1. Nitro, Assinaturas e Impulso de Servidor
- **Níveis de Assinatura (Tiers):**
  - `Nitro Basic`: Emojis e figurinhas personalizados em qualquer servidor, uploads de até 50 MB, badge especial Nitro no perfil.
  - `Nitro Full`: Uploads de até 500 MB, streaming em 4K a 60 FPS, 2 Impulsos de Servidor (Server Boosts) inclusos + 30% de desconto em impulsos adicionais, temas personalizados e cores de perfil com gradiente, badges animados.
- **Gestão de Impulsos de Servidor (Server Boosts):**
  - Lista de servidores impulsionados pelo usuário, contagem de impulsos disponíveis, data de renovação e cooldown de transferência de impulso (7 dias após aplicação).
- **Inventário de Presentes e Cobrança:**
  - Histórico de transações, métodos de pagamento cadastrados (Cartão de Crédito, PayPal) e inventário de presentes/códigos de resgate promocionais.

### 1.2. Conexões de Contas (OAuth2 / Linked Accounts)
- **Plataformas Suportadas:**
  - GitHub (`github`), Spotify (`spotify`), Steam (`steam`), YouTube (`youtube`), Twitch (`twitch`), PlayStation Network (`playstation`), Xbox (`xbox`), Reddit (`reddit`), TikTok (`tiktok`), Battle.net (`battlenet`).
- **Controles Granulares por Conexão:**
  - `Exibir no perfil` (`show_on_profile: bool`): Exibe o link público e nome da conta no card de perfil do usuário.
  - `Exibir status como atividade` (`show_activity: bool`): Transmite em tempo real atividade rica (ex: ouvindo música no Spotify com botão "Ouvir junto", jogando título da Steam/PlayStation).
  - `Distintivo verificado` (`verified: bool`): Badge oficial da plataforma verificado via OAuth2.

### 1.3. Centro da Família e Aplicativos Autorizados
- **Centro da Família (Family Center):**
  - Painel de transparência e segurança familiar:
    - Vínculos supervisionados ativos (contas de responsáveis e adolescentes).
    - Resumo de atividades dos últimos 30 dias: novos amigos adicionados, servidores acessados e tempo de chamada.
    - Controles de restrições de chamadas e mensagens diretas com remetentes desconhecidos.
- **Aplicativos Autorizados (Authorized Apps):**
  - Lista de integrações OAuth2 de terceiros concedidas pelo usuário.
  - Exibição de escopos (`identify`, `email`, `guilds`, `connections`, `rpc`).
  - Ação de revogação imediata de token (`Revogar Acesso`).

### 1.4. Aparência Avançada
- **Temas Disponíveis:**
  - `Escuro` (`#313338` - Clássico).
  - `Claro` (`#ffffff` - Alto contraste claro).
  - `Midnight` (`#000000` - OLED puro de alto contraste para economia de bateria e telas AMOLED).
  - `Sincronizar com o Computador` (Segue preferência do sistema operacional via DBus/FreeDesktop Dark Mode).
- **Exibição de Mensagens:**
  - `Aconchegante (Cozy)`: Avatares grandes de 40px, timestamps relativos e espaçamento moderno.
  - `Compacto (Compact)`: Linhas condensadas estilo IRC clássico, timestamp à esquerda e avatares suprimidos.
- **Escala de Zoom da Interface:**
  - Sliders com passos de 5% de 70% a 125% (Padrão 100%).

### 1.5. Acessibilidade
- **Controle de Movimento:**
  - `Redução de Movimento` (`reduce_motion: bool`): Desativa transições elásticas, animações de morph e efeitos de hover rápidos no GPUI.
  - `Reprodução Automática de Emojis e Stickers`: Opções de ativar, passar o mouse para animar ou desativar totalmente.
- **Tamanho de Fonte do Chat:**
  - Slider contínuo de 12px a 24px (Padrão: 16px).
- **Contraste e Cores:**
  - `Controle de Saturação de Cores`: Slider de 0% (preto e branco) a 100% (saturação normal).
  - `Sincronizar com Contraste do Sistema`.

---

## 2. Mapeamento de Rotas da API REST do Discord

| Rota / Endpoint | Método HTTP | Finalidade e Payloads |
| :--- | :---: | :--- |
| `/users/@me/billing/subscriptions` | `GET` | Retorna assinaturas ativas de Nitro, planos anuais/mensais e data de expiração. |
| `/users/@me/guilds/premium/subscriptions` | `GET`, `POST`, `DELETE` | Lista impulsos de servidor ativos, aplica novo boost ou cancela impulso existente. |
| `/users/@me/connections` | `GET` | Lista todas as contas de terceiros conectadas ao usuário autenticado. |
| `/users/@me/connections/{type}/{id}` | `PATCH`, `DELETE` | Atualiza flags de visibilidade no perfil ou desconecta uma conta de terceiro. |
| `/users/@me/profile` | `GET`, `PATCH` | Obtém e atualiza dados profundos de perfil (banner Nitro, bio, badges, efeitos). |
| `/oauth2/tokens` | `GET` | Lista tokens de aplicativos OAuth2 autorizados com escopos e timestamps. |
| `/oauth2/tokens/{token_id}` | `DELETE` | Revoga imediatamente a autorização de um aplicativo de terceiros. |
| `/users/@me/settings` | `GET`, `PATCH` | Sincroniza configurações globais (tema, acessibilidade, zoom, exibição de chat). |
| `/family-center/summary` | `GET` | Recupera métricas e alertas de segurança do Centro da Família. |

---

## 3. Arquitetura de Domínio em Rust (`src/domain/settings.rs`)

Os modelos de domínio são projetados como estruturas imutáveis e serializáveis via `serde`, garantindo consistência com as respostas da API e desacoplamento da camada de renderização do GPUI.
