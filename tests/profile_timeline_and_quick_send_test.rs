//! Profile effect timeline & popout quick send comprehensive test suite
//! Parity requirements:
//! - 5.1: is_layer_visible_at pure function tested across multiple t (with and without loop)
//! - 5.1: visible_layers_at sorts layers by z-index and filters by timeline
//! - 5.1: Reduced motion accessibility support
//! - 5.2: send_profile_popout_quick_message_async:
//!   - Empty field does not send
//!   - Sends to correct DM channel (creates/opens user DM)
//!   - No optimistic message prior to API confirmation
//!   - Error appears on popout when API fails
//!   - Field clears only on success (preserved on error)
//!   - Popout closes only on success

use discord_rs::app::state::AppState;
use discord_rs::components::profile_effect::{
    is_layer_visible_at, visible_layers_at, ProfileEffectConfig, ProfileEffectLayer,
};
use discord_rs::domain::profile::UserProfileData;
use discord_rs::protocol::ClientManager;

#[test]
fn test_is_layer_visible_at_timeline_pure_function() {
    // 1. Zero duration layer is never visible
    let zero_layer = ProfileEffectLayer {
        src: "zero.png".into(),
        is_looping: false,
        duration: 0,
        start: 0,
        loop_delay: 0,
        z_index: 0,
        randomized_sources: vec![],
    };
    assert!(!is_layer_visible_at(&zero_layer, 0));
    assert!(!is_layer_visible_at(&zero_layer, 500));

    // 2. Non-looping layer: start=500, duration=1500 (active in [500, 2000))
    let non_looping = ProfileEffectLayer {
        src: "burst.png".into(),
        is_looping: false,
        duration: 1500,
        start: 500,
        loop_delay: 0,
        z_index: 10,
        randomized_sources: vec![],
    };
    assert!(!is_layer_visible_at(&non_looping, 0));
    assert!(!is_layer_visible_at(&non_looping, 499));
    assert!(is_layer_visible_at(&non_looping, 500));
    assert!(is_layer_visible_at(&non_looping, 1000));
    assert!(is_layer_visible_at(&non_looping, 1999));
    assert!(!is_layer_visible_at(&non_looping, 2000));
    assert!(!is_layer_visible_at(&non_looping, 3000));

    // 3. Looping layer without loop_delay: start=200, duration=800, is_looping=true, loop_delay=0
    let looping_no_delay = ProfileEffectLayer {
        src: "infinite.png".into(),
        is_looping: true,
        duration: 800,
        start: 200,
        loop_delay: 0,
        z_index: 20,
        randomized_sources: vec![],
    };
    assert!(!is_layer_visible_at(&looping_no_delay, 0));
    assert!(!is_layer_visible_at(&looping_no_delay, 199));
    assert!(is_layer_visible_at(&looping_no_delay, 200));
    assert!(is_layer_visible_at(&looping_no_delay, 999));
    assert!(is_layer_visible_at(&looping_no_delay, 1000)); // Start of loop 2
    assert!(is_layer_visible_at(&looping_no_delay, 50000));

    // 4. Looping layer with loop_delay: start=1000, duration=2000, loop_delay=1000 (period=3000)
    // Cycle 0: active [1000, 3000), pause [3000, 4000)
    // Cycle 1: active [4000, 6000), pause [6000, 7000)
    let looping_with_delay = ProfileEffectLayer {
        src: "periodic.png".into(),
        is_looping: true,
        duration: 2000,
        start: 1000,
        loop_delay: 1000,
        z_index: 30,
        randomized_sources: vec![],
    };
    assert!(!is_layer_visible_at(&looping_with_delay, 0));
    assert!(!is_layer_visible_at(&looping_with_delay, 999));
    assert!(is_layer_visible_at(&looping_with_delay, 1000));
    assert!(is_layer_visible_at(&looping_with_delay, 2500));
    assert!(is_layer_visible_at(&looping_with_delay, 2999));
    // During loopDelay pause:
    assert!(!is_layer_visible_at(&looping_with_delay, 3000));
    assert!(!is_layer_visible_at(&looping_with_delay, 3500));
    assert!(!is_layer_visible_at(&looping_with_delay, 3999));
    // Next cycle:
    assert!(is_layer_visible_at(&looping_with_delay, 4000));
    assert!(is_layer_visible_at(&looping_with_delay, 5999));
    assert!(!is_layer_visible_at(&looping_with_delay, 6000));
    assert!(!is_layer_visible_at(&looping_with_delay, 6500));
    // Third cycle:
    assert!(is_layer_visible_at(&looping_with_delay, 7000));
}

#[test]
fn test_visible_layers_at_timeline_sorting_and_filtering() {
    let config = ProfileEffectConfig {
        sku_id: "test_sku_sort".into(),
        title: Some("Sort Test".into()),
        description: None,
        accessibility_label: None,
        reduced_motion_src: Some("https://cdn.discordapp.com/static.png".into()),
        thumbnail_preview_src: Some("https://cdn.discordapp.com/thumb.png".into()),
        effects: vec![
            ProfileEffectLayer {
                src: "top_layer.png".into(),
                is_looping: false,
                duration: 2000,
                start: 0,
                loop_delay: 0,
                z_index: 100,
                randomized_sources: vec![],
            },
            ProfileEffectLayer {
                src: "bottom_layer.png".into(),
                is_looping: false,
                duration: 2000,
                start: 0,
                loop_delay: 0,
                z_index: 5,
                randomized_sources: vec![],
            },
            ProfileEffectLayer {
                src: "mid_delayed_layer.png".into(),
                is_looping: false,
                duration: 2000,
                start: 1500,
                loop_delay: 0,
                z_index: 50,
                randomized_sources: vec![],
            },
        ],
        animation_type: Some(1),
        effect_type: Some(1),
    };

    // At t=500: bottom_layer (z=5) and top_layer (z=100) are active, sorted ascending by z_index
    let visible_500 = visible_layers_at(&config, 500);
    assert_eq!(visible_500.len(), 2);
    assert_eq!(visible_500[0].src, "bottom_layer.png");
    assert_eq!(visible_500[1].src, "top_layer.png");

    // At t=1800: all 3 layers are active, sorted by z_index
    let visible_1800 = visible_layers_at(&config, 1800);
    assert_eq!(visible_1800.len(), 3);
    assert_eq!(visible_1800[0].src, "bottom_layer.png");
    assert_eq!(visible_1800[1].src, "mid_delayed_layer.png");
    assert_eq!(visible_1800[2].src, "top_layer.png");

    // At t=2500: only mid_delayed_layer is active
    let visible_2500 = visible_layers_at(&config, 2500);
    assert_eq!(visible_2500.len(), 1);
    assert_eq!(visible_2500[0].src, "mid_delayed_layer.png");

    // At t=4000: none active
    let visible_4000 = visible_layers_at(&config, 4000);
    assert_eq!(visible_4000.len(), 0);
}

#[tokio::test]
async fn test_quick_send_empty_field_does_not_send() {
    let mut state = AppState::new();
    let (cm, _rx) = ClientManager::new_mock();
    state.client_manager = Some(cm);

    // Open popout for user
    state.open_user_profile("user_test_empty", None);
    assert!(state.profile_popout.is_some());

    // Case 1: Empty string
    state.profile_popout.as_mut().unwrap().quick_message = "".to_string();
    assert!(state.begin_profile_popout_quick_message().is_none());
    assert!(state.profile_popout.is_some(), "Popout must remain open on empty input");

    // Case 2: Only whitespace
    state.profile_popout.as_mut().unwrap().quick_message = "    \n\t  ".to_string();
    assert!(state.begin_profile_popout_quick_message().is_none());
    assert!(state.profile_popout.is_some(), "Popout must remain open on whitespace input");

    // Verify no message was injected into any channel
    assert!(state.messages_by_channel.is_empty());
}

#[test]
fn test_quick_send_disconnected_sets_error_and_preserves_field() {
    let mut state = AppState::new();
    // client_manager is None (ZERO MOCKS: no fake offline message allowed)

    state.open_user_profile("user_no_net", None);
    let draft = "Mensagem que não deve ser inventada offline".to_string();
    state.profile_popout.as_mut().unwrap().quick_message = draft.clone();

    let req = state.begin_profile_popout_quick_message();
    assert!(req.is_none(), "Sem conexão não deve criar requisição de envio");

    let popout = state.profile_popout.as_ref().expect("Popout deve continuar aberto");
    assert_eq!(popout.error.as_deref(), Some("Não conectado ao Discord."));
    assert_eq!(popout.quick_message, draft, "Campo deve ser preservado quando desconectado");
    assert!(state.messages_by_channel.is_empty(), "Nenhuma mensagem falsa pode ser injetada");
}

#[tokio::test]
async fn test_quick_send_success_with_mock_client_manager() {
    let mut state = AppState::new();
    let (cm, _rx) = ClientManager::new_mock();
    state.client_manager = Some(cm);

    let target_uid = "user_ferris";
    state.open_user_profile(target_uid, None);
    let profile_data = UserProfileData {
        user: discord_rs::domain::user::User::new(target_uid, "ferris_crab"),
        bio: None,
        pronouns: None,
        banner: None,
        banner_color: None,
        accent_color: None,
        profile_effect_id: None,
        badges: vec![],
        connected_accounts: vec![],
        mutual_guilds: vec![],
        mutual_friends_count: 0,
        created_at: None,
        joined_server_at: None,
    };
    state.set_profile_popout_data(target_uid, profile_data);

    let test_msg_text = "Olá Ferris, tudo bem?";
    state.profile_popout.as_mut().unwrap().quick_message = test_msg_text.to_string();

    // Verify BEFORE sending: no optimistic message exists
    assert!(state.messages_by_channel.is_empty(), "Sem resposta da API não pode haver mensagem");

    // Phase 1: Begin send
    let req = state
        .begin_profile_popout_quick_message()
        .expect("Requisição válida deve ser criada");

    // In-flight guard: sending must be true, blocking duplicate Enter while sending
    assert!(state.is_profile_popout_quick_message_sending());
    assert!(
        state.begin_profile_popout_quick_message().is_none(),
        "Enter repetido enquanto envio está em curso deve ser bloqueado"
    );

    // Phase 2: Execute async outside AppState
    let result = req.execute().await;
    assert!(result.is_ok(), "Envio deve ter sucesso com ClientManager mock: {:?}", result);

    // Phase 3: Finish send
    state.finish_profile_popout_quick_message(&req, result);

    // Popout must be closed on success
    assert!(state.profile_popout.is_none(), "Popout deve fechar no sucesso");

    // DM channel must exist for recipient
    let dm_chan = state.dm_channels.iter().find(|c| c.recipient_ids.contains(&target_uid.to_string()));
    assert!(dm_chan.is_some(), "DM channel para o usuário deve existir");

    // Messages must contain confirmed message
    let all_msgs = state.messages_by_channel.values().flatten().collect::<Vec<_>>();
    assert!(all_msgs.iter().any(|m| m.content == test_msg_text));
}

#[tokio::test]
async fn test_quick_send_failure_displays_error_and_preserves_field() {
    let mut state = AppState::new();
    // ClientManager::new() has IdleTransport, which rejects send_message with error
    let (cm, _rx) = ClientManager::new();
    state.client_manager = Some(cm);

    let target_uid = "user_offline_target";
    state.open_user_profile(target_uid, None);

    let draft_text = "Esta mensagem vai falhar porque o transport está desconectado";
    state.profile_popout.as_mut().unwrap().quick_message = draft_text.to_string();

    // Verify BEFORE sending: no message in UI
    assert!(state.messages_by_channel.is_empty());

    // Phase 1: Begin send
    let req = state.begin_profile_popout_quick_message().expect("Requisição válida");

    // Phase 2: Execute send - fails with IdleTransport error
    let result = req.execute().await;
    assert!(result.is_err(), "Envio com transport desconectado deve falhar");

    // Phase 3: Finish send with error
    state.finish_profile_popout_quick_message(&req, result);

    // CRITICAL: Sem resposta da API não aparece mensagem otimista
    assert!(
        state.messages_by_channel.is_empty(),
        "Nenhuma mensagem pode ter sido injetada em caso de erro"
    );

    // CRITICAL: Popout continua aberto
    assert!(state.profile_popout.is_some(), "Popout deve continuar aberto no erro");

    let popout = state.profile_popout.as_ref().unwrap();

    // CRITICAL: O campo limpa só no sucesso (no erro deve permanecer intacto)
    assert_eq!(
        popout.quick_message, draft_text,
        "O campo deve manter o texto digitado quando há falha"
    );

    // CRITICAL: Erro aparece no popout
    assert!(
        popout.error.is_some(),
        "Erro deve ser gravado em popout.error para exibição na UI"
    );
    let err_str = popout.error.as_ref().unwrap();
    assert!(!err_str.is_empty(), "Mensagem de erro não deve ser vazia");
    assert!(!state.is_profile_popout_quick_message_sending(), "in_flight sending state deve ser resetado no término");
}

#[tokio::test]
async fn test_double_enter_generates_single_send_and_subsequent_send_accepted_after_completion() {
    let mut state = AppState::new();
    let (cm, _rx) = ClientManager::new_mock();
    state.client_manager = Some(cm);

    let target_uid = "user_double_enter_target";
    state.open_user_profile(target_uid, None);
    state.profile_popout.as_mut().unwrap().quick_message = "Primeira mensagem".to_string();

    // 1. Primeiro Enter: gera envio
    let req1 = state
        .begin_profile_popout_quick_message()
        .expect("Primeiro Enter deve gerar ProfileQuickSend");
    assert!(state.is_profile_popout_quick_message_sending(), "Deve marcar envio em andamento");

    // 2. Segundo Enter imediato enquanto o primeiro está em voo: DEVE ser bloqueado (retorna None)
    let req2 = state.begin_profile_popout_quick_message();
    assert!(req2.is_none(), "Segundo Enter repetido em voo deve ser bloqueado");

    // 3. Simula falha do primeiro envio: finish desmarca in_flight, exibe erro e preserva campo
    state.finish_profile_popout_quick_message(&req1, Err("Falha simulada de rede".to_string()));
    assert!(!state.is_profile_popout_quick_message_sending(), "Flag de envio deve ser desmarcada no erro");
    assert!(state.profile_popout.is_some(), "Popout deve continuar aberto no erro");
    assert_eq!(
        state.profile_popout.as_ref().unwrap().error.as_deref(),
        Some("Falha simulada de rede")
    );
    assert_eq!(
        state.profile_popout.as_ref().unwrap().quick_message,
        "Primeira mensagem"
    );

    // 4. Novo Enter após erro: DEVE ser aceito!
    let req3 = state
        .begin_profile_popout_quick_message()
        .expect("Novo Enter após erro deve ser aceito");
    assert!(state.is_profile_popout_quick_message_sending());

    // Enter concorrente durante req3 também deve ser bloqueado
    assert!(state.begin_profile_popout_quick_message().is_none());

    // 5. Sucesso do envio req3: executa e finaliza
    let res3 = req3.execute().await;
    assert!(res3.is_ok());
    state.finish_profile_popout_quick_message(&req3, res3);

    // Popout fecha e flag é desmarcada
    assert!(state.profile_popout.is_none(), "Popout deve fechar no sucesso");
    assert!(!state.is_profile_popout_quick_message_sending());

    // 6. Novo envio posterior para o mesmo usuário (ex.: reabrindo o popout): DEVE ser aceito!
    state.open_user_profile(target_uid, None);
    state.profile_popout.as_mut().unwrap().quick_message = "Segunda mensagem após sucesso".to_string();

    let req4 = state
        .begin_profile_popout_quick_message()
        .expect("Novo envio após sucesso deve ser aceito");
    assert!(state.is_profile_popout_quick_message_sending());

    // Enter concorrente durante req4 bloqueado
    assert!(state.begin_profile_popout_quick_message().is_none());

    // Finaliza req4 com sucesso
    let res4 = req4.execute().await;
    state.finish_profile_popout_quick_message(&req4, res4);
    assert!(state.profile_popout.is_none());
    assert!(!state.is_profile_popout_quick_message_sending());
}
