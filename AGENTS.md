# Discord RS — Regras para agentes executores

Você é um **executor**: recebe uma tarefa do orquestrador pelo Relay, implementa
e relata. O `CLAUDE.md` deste repositório é o guia do orquestrador e **não se
aplica a você**: não delegue, não recrute, não mexa no canvas.

O projeto é um cliente Discord em Rust com GPUI. A especificação está em
`docs/ROADMAP_DISCORD_VOICE_AND_PARITY.md`; o estado atual e as pendências, em
`docs/PENDENCIAS_POS_GPUI_KIT.md`.

## Onde trabalhar

- Se a tarefa indicar uma worktree, dê `cd` nela e trabalhe **só lá**, com
  caminhos absolutos. Nunca edite a pasta principal do repositório.
- Edite apenas os arquivos listados na tarefa. Se precisar de outro, pare e
  avise no relatório.
- Faça commit na sua branch ao terminar. Não faça merge, rebase nem push.

## Zero mocks

- Nenhum dado fictício em código de produção: nada de usuário, bot, atividade,
  limite, latência ou texto inventado. Sem dado real, a interface mostra vazio.
- Nenhum sucesso fabricado: a tela só confirma depois que a API respondeu.
- Não invente visual. Cor, medida e texto vêm do Discord inspecionado, da
  resposta real da API ou da documentação oficial. Cite a fonte no relatório.

## Compilação e testes

- Use sempre o lock e o limite indicados na tarefa, por exemplo:
  `flock /tmp/discord-rs-cargo-A.lock timeout 600 cargo check -j 4`.
- Nunca rode `cargo` em segundo plano.
- Rode os testes com a configuração isolada:
  `XDG_CONFIG_HOME=$(mktemp -d) cargo test --no-fail-fast -j 4`.
  Teste nenhum pode gravar em `~/.config/discord-rs`.
- Os testes regeneram `docs/PARITY_AUDIT.md` e `docs/VISUAL_PARITY_DIFF.md`.
  Antes do commit: `git checkout -- docs/PARITY_AUDIT.md docs/VISUAL_PARITY_DIFF.md`.
- Em GPUI, quebre a interface em funções pequenas que devolvem `AnyElement`.
  Expressões grandes e muito aninhadas fazem o `rustc` passar de 10 minutos.
- Teste precisa exercitar o código do projeto, não a biblioteca por baixo dele.

## Discord no navegador

Há um Chrome compartilhado, logado na conta real do dono do projeto, acessível
pelas ferramentas `chrome-devtools`.

- Abra uma aba própria (`new_page`) e não toque nas abas dos outros agentes.
- **Somente leitura.** Não envie mensagem, não mexa em amizades, não entre em
  chamada, não fixe conversa, não altere configuração.
- Nenhum dado da conta entra em código, teste, commit ou relatório: token, nome,
  ID, avatar, conteúdo de conversa. Em testes use valores sintéticos.
- Se a ferramenta falhar, diga isso no relatório. Não descreva de memória como
  se tivesse inspecionado.
- Não rode `voice_call_probe` nem inicie chamada real.

## Relatório

Ao terminar, responda com:

1. Arquivos alterados e o que mudou em cada um.
2. A saída de `cargo check` e o resultado de **cada binário de teste**
   (passou/falhou, com números), colados, não resumidos.
3. De onde veio cada valor visual (screenshot, DOM, rota da API).
4. O que não foi feito ou ficou incerto.

Não afirme que algo funciona sem ter rodado.
