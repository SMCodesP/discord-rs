//! Discord RS - Dicionário Português do Brasil (pt-BR)
//! Termos oficiais e fiéis à interface em português brasileiro do Discord.

/// Dicionário estático de pares chave-valor para localização pt-BR
pub const DICTIONARY: &[(&str, &str)] = &[
    // App & Janela
    ("app.title", "Discord"),
    ("app.tagline", "Seu lugar para conversar e ficar de boa"),
    ("app.loading", "Conectando ao Discord..."),

    // Status do Usuário
    ("status.online", "Online"),
    ("status.idle", "Ausente"),
    ("status.ausente", "Ausente"),
    ("status.dnd", "Não perturbar"),
    ("status.invisible", "Invisível"),
    ("status.offline", "Offline"),
    ("status.custom", "Definir status personalizado"),
    ("status.clear", "Limpar status"),

    // Barra Lateral de Servidores (Rail)
    ("server.direct_messages", "Mensagens diretas"),
    ("server.add", "Adicionar um servidor"),
    ("server.explore", "Descobrir servidores públicos"),
    ("server.settings", "Configurações do Servidor"),
    ("server.create_channel", "Criar canal"),
    ("server.create_category", "Criar categoria"),
    ("server.notification_settings", "Configurações de notificações"),
    ("server.privacy_settings", "Configurações de privacidade"),
    ("server.edit_profile", "Editar perfil do servidor"),
    ("server.hide_muted", "Ocultar canais silenciados"),
    ("server.invite", "Convidar pessoas"),
    ("server.leave", "Sair do servidor"),
    ("server.delete", "Excluir servidor"),

    // Lista de Canais
    ("channels.text_channels", "CANAIS DE TEXTO"),
    ("channels.voice_channels", "CANAIS DE VOZ"),
    ("channels.announcement_channels", "CANAIS DE AVISOS"),
    ("channels.browse", "Explorar canais"),
    ("channels.customize", "Canais e cargos"),
    ("channels.create", "Criar canal"),
    ("channels.edit", "Editar canal"),
    ("channels.delete", "Excluir canal"),
    ("channels.mute", "Silenciar canal"),
    ("channels.unmute", "Reativar notificações do canal"),
    ("channels.rules", "Regras"),
    ("channels.welcome", "Boas-vindas"),

    // Chat e Mensagens
    ("chat.placeholder", "Conversar em #{channel}"),
    ("chat.placeholder_dm", "Conversar com @{user}"),
    ("chat.no_permission", "Você não tem permissão para enviar mensagens neste canal."),
    ("chat.reply", "Responder"),
    ("chat.edit", "Editar mensagem"),
    ("chat.delete", "Excluir mensagem"),
    ("chat.pin", "Fixar mensagem"),
    ("chat.unpin", "Desafixar mensagem"),
    ("chat.pinned_messages", "Mensagens fixadas"),
    ("chat.no_pinned", "Não há mensagens fixadas ainda."),
    ("chat.thread_create", "Criar tópico"),
    ("chat.copy_text", "Copiar texto"),
    ("chat.copy_id", "Copiar ID da mensagem"),
    ("chat.mark_unread", "Marcar como não lida"),
    ("chat.add_reaction", "Adicionar reação"),
    ("chat.new_messages", "Novas mensagens"),
    ("chat.jump_to_present", "Pular para o presente"),
    ("chat.edited", "(editado)"),
    ("chat.today_at", "Hoje às {time}"),
    ("chat.yesterday_at", "Ontem às {time}"),

    // Barra de Ferramentas de Entrada (Input bar)
    ("input.gift", "Enviar presente Nitro"),
    ("input.gif", "Abrir seletor de GIFs"),
    ("input.sticker", "Abrir seletor de figurinhas"),
    ("input.emoji", "Selecionar emoji"),
    ("input.upload", "Enviar arquivo"),

    // Painel Inferior do Usuário
    ("user.mute", "Silenciar"),
    ("user.unmute", "Desativar mudo"),
    ("user.deafen", "Ensurdecer"),
    ("user.undeafen", "Desativar ensurdecer"),
    ("user.settings", "Configurações de Usuário"),
    ("user.copy_username", "Copiar nome de usuário"),
    ("user.voice_connected", "Voz conectada"),
    ("user.voice_disconnect", "Desconectar"),
    ("user.rtc_connecting", "Conectando ao RTC..."),
    ("user.rtc_connected", "RTC Conectado"),

    // Lista de Membros
    ("members.title", "Membros"),
    ("members.online_count", "ONLINE — {count}"),
    ("members.offline_count", "OFFLINE — {count}"),
    ("members.bot_count", "BOTS — {count}"),
    ("members.toggle", "Lista de membros"),

    // Barra Superior (Header)
    ("topbar.search", "Buscar"),
    ("topbar.inbox", "Caixa de Entrada"),
    ("topbar.pinned", "Mensagens Fixadas"),
    ("topbar.notifications", "Configurações de Notificação"),
    ("topbar.threads", "Tópicos"),
    ("topbar.help", "Ajuda"),

    // Ações Gerais e Modais
    ("common.cancel", "Cancelar"),
    ("common.save", "Salvar alterações"),
    ("common.confirm", "Confirmar"),
    ("common.close", "Fechar"),
    ("common.back", "Voltar"),
    ("common.loading", "Carregando..."),
    ("common.search_placeholder", "Buscar"),
    ("common.done", "Concluído"),
    ("common.okay", "Entendi"),

    // Autenticação e Conexão (Login & Token)
    ("auth.login", "Entrar no Discord"),
    ("auth.enter_token", "Inserir Token"),
    ("auth.token_type_hint", "Token de Bot ou Usuário"),
    ("auth.token_label", "Token de Bot ou Usuário"),
    ("auth.save_token", "Salvar Token"),
    ("auth.connect", "Conectar"),
    ("auth.connecting", "Conectando..."),
    ("auth.connected", "Conectado"),
    ("auth.mock_mode", "Modo Demonstração (Mock)"),
    ("auth.demo_mode", "Modo Demonstração (Mock)"),
    ("auth.disconnect", "Desconectar"),
    ("auth.failed", "Falha ao autenticar"),
    ("auth.auth_failed", "Falha ao autenticar"),
    ("auth.description", "Insira seu token para carregar seus servidores e canais reais"),
    ("auth.prompt", "Insira seu token para carregar seus servidores e canais reais"),

    // Status de Conexão
    ("connection.status", "Status da Conexão"),
    ("connection.connect", "Conectar"),
    ("connection.connecting", "Conectando..."),
    ("connection.connected", "Conectado"),
    ("connection.disconnected", "Desconectado"),
    ("connection.mock", "Modo Demonstração (Mock)"),
    ("connection.disconnect", "Desconectar"),
    ("connection.failed", "Falha ao autenticar"),

    // Perfil de Usuário
    ("profile.about_me", "Sobre mim"),
    ("profile.discord_member_since", "Membro do Discord desde"),
    ("profile.server_member_since", "Membro do servidor desde"),
    ("profile.mutual_guilds", "Servidores em comum"),
    ("profile.mutual_friends", "Amigos em comum"),
    ("profile.message_placeholder", "Conversar com @{username}"),
    ("profile.loading", "Carregando perfil..."),
    ("profile.error", "Não foi possível carregar o perfil."),
];

/// Procura uma chave no dicionário pt-BR.
pub fn lookup(key: &str) -> Option<&'static str> {
    DICTIONARY
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| *v)
}
