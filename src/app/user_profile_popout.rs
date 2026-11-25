//! Discord RS - Canonical User Profile Components (Fase 4 & Modal Parity)
//! 1:1 Parity with Official Discord Desktop Client:
//! - UserProfilePopout: 300px anchored popout card, no backdrop, dismissible on click-outside or Esc
//! - UserProfileModal: 600x557px centered Dialog with backdrop, 3 tabs (Sobre mim, Servidores em comum, Amigos em comum)
//!
//! Features:
//! - Popout opens anchored to the clicked element/position and snaps to window edges
//! - Popout has NO close button (closes on click outside / Esc key)
//! - Opens full modal when clicking avatar/name inside popout or "Ver perfil completo"
//! - Modal is a single 600x557 r8 box with a single close button (native DialogClose)
//! - Bio text breaks lines smoothly within 300px width
//! - Quick message input has show_paste(false) and ellipsis placeholder
//! - Canonical Avatar component with decor & presence support throughout
//! - Official Button and IconButton with icons
//! - Shared colors come from the documented Discord token palette
//! - Zero fake mocks (empty when real data is absent, no fake fallback names)

use std::collections::HashMap;

use chrono::{DateTime, Datelike, Utc};
use gpui::component::dialog::Dialog;
use gpui::component::input::InputState;
use gpui::component::tooltip::Tooltip;
use gpui::prelude::*;
use gpui::*;

use crate::app::state::{AppState, ProfileModalTab};
use crate::components::TextInput;
use crate::components::avatar::{Avatar, AvatarSize};
use crate::components::button::{Button, ButtonSize};
use crate::components::profile_effect::{
    ProfileEffectCatalog, UserProfileEffect, render_profile_effect_for_user_with_motion,
};
pub use crate::domain::profile::{
    ConnectedAccount, MutualGuild, ProfileBadge, ProfilePopoutState, ProfileTab, UserProfileData,
    snowflake_to_datetime,
};
use crate::domain::user::{ActivityType, CustomStatus, Presence, PresenceStatus};
use crate::i18n::t;
use crate::tokens::colors::white;
use crate::tokens::colors::*;
use crate::tokens::icons::*;
use crate::tokens::spacing::{radius_sm, space_2, space_3, space_4};

// ============================================================================
// Helper Utilities
// ============================================================================

/// Parses an official `#RRGGBB` banner color into a `u32` RGB value.
pub fn parse_hex_color(hex_str: &str) -> Option<u32> {
    let clean = hex_str.trim().strip_prefix('#').unwrap_or(hex_str.trim());
    if clean.len() != 6 || !clean.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(clean, 16).ok()
}

fn profile_banner_color(popout: &ProfilePopoutState) -> u32 {
    popout
        .data
        .as_ref()
        .and_then(|data| data.banner_color.as_deref().and_then(parse_hex_color))
        .or_else(|| {
            popout
                .data
                .as_ref()
                .and_then(|data| data.accent_color)
                .filter(|color| *color <= 0xffffff)
        })
        .unwrap_or(HEX_BG_FLOATING)
}

fn profile_banner_image_url(popout: &ProfilePopoutState, size: u32) -> Option<String> {
    let data = popout.data.as_ref()?;
    let banner = data
        .banner
        .as_deref()
        .map(str::trim)
        .filter(|banner| !banner.is_empty())?;
    Some(
        if banner.starts_with("http://") || banner.starts_with("https://") {
            banner.to_string()
        } else {
            format!(
                "https://cdn.discordapp.com/banners/{}/{}.png?size={size}",
                data.user.id, banner
            )
        },
    )
}

/// Formats a month name in pt-BR
fn pt_month_short(month: u32) -> &'static str {
    match month {
        1 => "jan.",
        2 => "fev.",
        3 => "mar.",
        4 => "abr.",
        5 => "mai.",
        6 => "jun.",
        7 => "jul.",
        8 => "ago.",
        9 => "set.",
        10 => "out.",
        11 => "nov.",
        12 => "dez.",
        _ => "",
    }
}

/// Formats a DateTime into canonical Discord display ("20 de out. de 2023")
pub fn format_member_date(dt: DateTime<Utc>) -> String {
    format!(
        "{} de {} de {}",
        dt.day(),
        pt_month_short(dt.month()),
        dt.year()
    )
}

// ============================================================================
// Shared Content Renderers (Popout & Modal Deduplication)
// ============================================================================

/// Renders the profile bio with the same Discord Markdown parser as chat.
fn render_bio_content(bio: &str) -> AnyElement {
    div()
        .w_full()
        .min_w(px(0.0))
        .text_size(px(14.0))
        .line_height(px(20.0))
        .child(crate::chat::markdown::render_markdown(bio))
        .into_any_element()
}

/// Renders mutual guilds using only names/icons resolved from real guild state.
fn render_mutual_guilds_list(
    guilds: &[MutualGuild],
    guild_views: &GuildProfileViews,
    is_modal: bool,
) -> AnyElement {
    if guilds.is_empty() {
        return div()
            .py(if is_modal { px(0.0) } else { px(8.0) })
            .text_size(if is_modal { px(14.0) } else { px(12.0) })
            .text_color(legacy_949ba4())
            .child("Nenhum servidor em comum.")
            .into_any_element();
    }

    let item_h = if is_modal { px(40.0) } else { px(32.0) };
    let icon_sz = if is_modal { px(24.0) } else { px(20.0) };
    let text_sz = if is_modal { px(13.0) } else { px(12.0) };

    let mut list = div().flex().flex_col().gap(px(4.0));
    let mut rendered = 0usize;
    for guild in guilds {
        let view = guild_views.get(&guild.id);
        let display_name = view
            .map(|(name, _)| name.trim())
            .filter(|name| !name.is_empty())
            .or_else(|| {
                guild
                    .nick
                    .as_deref()
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
            });
        let Some(display_name) = display_name else {
            continue;
        };
        rendered += 1;

        let mut row = div()
            .h(item_h)
            .px(px(6.0))
            .rounded(px(4.0))
            .bg(bg_server())
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0));
        if let Some((_, Some(icon_url))) = view {
            row = row.child(
                img(icon_url.clone())
                    .size(icon_sz)
                    .rounded_full()
                    .object_fit(ObjectFit::Cover),
            );
        }
        list = list.child(
            row.child(
                div()
                    .text_size(text_sz)
                    .text_color(text_primary())
                    .child(display_name.to_string()),
            ),
        );
    }

    if rendered == 0 {
        // The API confirmed mutual guilds, but their display data is not loaded.
        // Show an empty list instead of inventing a name or an initial.
        div().into_any_element()
    } else {
        list.into_any_element()
    }
}

/// Renders mutual friends count summary shared between popout and modal tabs
fn render_mutual_friends_content(count: u32, is_modal: bool) -> AnyElement {
    if count == 0 {
        return div()
            .py(if is_modal { px(0.0) } else { px(8.0) })
            .text_size(if is_modal { px(14.0) } else { px(12.0) })
            .text_color(legacy_949ba4())
            .child("Nenhum amigo em comum.")
            .into_any_element();
    }

    div()
        .py(if is_modal { px(0.0) } else { px(8.0) })
        .text_size(if is_modal { px(14.0) } else { px(12.0) })
        .text_color(text_primary())
        .child(format!("{} amigos em comum com este usuário.", count))
        .into_any_element()
}

/// Renders member dates: snowflake-derived account creation date and joined server date
fn render_member_dates_content(
    created_at: Option<DateTime<Utc>>,
    joined_server_at: Option<DateTime<Utc>>,
) -> Option<AnyElement> {
    if created_at.is_none() && joined_server_at.is_none() {
        return None;
    }

    let mut dates_col = div().flex().flex_col().gap(px(8.0));

    if let Some(created_at) = created_at {
        dates_col = dates_col.child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(text_secondary())
                        .child(t("profile.discord_member_since").to_uppercase()),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .child(icon_info(px(14.0), legacy_949ba4()))
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(text_primary())
                                .child(format_member_date(created_at)),
                        ),
                ),
        );
    }

    if let Some(joined_at) = joined_server_at {
        dates_col = dates_col.child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(text_secondary())
                        .child(t("profile.server_member_since").to_uppercase()),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .child(icon_hash(px(14.0), legacy_949ba4()))
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(text_primary())
                                .child(format_member_date(joined_at)),
                        ),
                ),
        );
    }

    Some(dates_col.into_any_element())
}

// ============================================================================
// Component 1: Anchored User Profile Popout (300px)
// ============================================================================

const PROFILE_POPOUT_WIDTH: f32 = 300.0;
const PROFILE_POPOUT_GAP: f32 = 8.0;
const PROFILE_POPOUT_MARGIN: f32 = 8.0;

type GuildProfileViews = HashMap<String, (String, Option<String>)>;

fn guild_profile_views(guilds: &[crate::app::state::GuildItemState]) -> GuildProfileViews {
    guilds
        .iter()
        .map(|guild| (guild.id.clone(), (guild.name.clone(), guild.icon_url())))
        .collect()
}

/// Resolves the opening side from the actual viewport, not a hard-coded width.
fn resolve_profile_popout_anchor(
    anchor: Option<Point<Pixels>>,
    viewport: Size<Pixels>,
) -> (Point<Pixels>, Anchor) {
    let margin = px(PROFILE_POPOUT_MARGIN);
    let Some(clicked_at) = anchor else {
        let right_limit = (viewport.width - margin).max(margin);
        return (point(right_limit, px(60.0)), Anchor::TopRight);
    };

    let x = clicked_at.x.max(px(0.0)).min(viewport.width);
    let y = clicked_at.y.max(px(0.0)).min(viewport.height);
    let right_edge = x + px(PROFILE_POPOUT_WIDTH + PROFILE_POPOUT_GAP + PROFILE_POPOUT_MARGIN);
    let opens_left = right_edge > viewport.width;
    let position = if opens_left {
        point(x - px(PROFILE_POPOUT_GAP), y)
    } else {
        point(x + px(PROFILE_POPOUT_GAP), y)
    };
    let corner = if opens_left {
        Anchor::TopRight
    } else {
        Anchor::TopLeft
    };
    (position, corner)
}

struct ProfilePopoutFocusState {
    focus_handle: FocusHandle,
    initialized: bool,
    modal_open: bool,
}

/// Element state keeps a stable focus handle for the lifetime of the popout.
/// This makes Esc work even when the click that opened the profile left focus
/// in the chat composer, and restores focus after the full modal closes.
struct ProfilePopoutElement {
    app_entity: Entity<AppState>,
    popout: ProfilePopoutState,
    anchor: Option<Point<Pixels>>,
    catalog: ProfileEffectCatalog,
    presence: Option<Presence>,
    quick_state: Entity<InputState>,
}

impl IntoElement for ProfilePopoutElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for ProfilePopoutElement {
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(ElementId::Name("profile-popout-layer".into()))
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        window.with_element_state(
            global_id.expect("profile popout has a stable id"),
            |state, window| {
                let modal_open = self.app_entity.read(cx).is_profile_modal_open();
                let reduce_motion = self
                    .app_entity
                    .read(cx)
                    .accessibility_settings
                    .reduce_motion;
                let quick_send_in_flight = self
                    .app_entity
                    .read(cx)
                    .is_profile_popout_quick_message_sending();
                let mut focus_state: ProfilePopoutFocusState =
                    state.unwrap_or_else(|| ProfilePopoutFocusState {
                        focus_handle: cx.focus_handle(),
                        initialized: false,
                        modal_open: true,
                    });
                let should_focus =
                    !modal_open && (!focus_state.initialized || focus_state.modal_open);
                focus_state.initialized = true;
                focus_state.modal_open = modal_open;
                let focus_handle = focus_state.focus_handle.clone();

                let (anchor_pos, anchor_corner) =
                    resolve_profile_popout_anchor(self.anchor, window.viewport_size());
                let guild_views = guild_profile_views(&self.app_entity.read(cx).guilds);
                let content = render_profile_popout_content(
                    &self.app_entity,
                    &self.popout,
                    &self.catalog,
                    self.presence.as_ref(),
                    &self.quick_state,
                    reduce_motion,
                    quick_send_in_flight,
                    &guild_views,
                    anchor_pos,
                    anchor_corner,
                );

                let entity_key = self.app_entity.clone();
                let layer = div()
                    .id("profile-popout-layer")
                    .key_context("ProfilePopout")
                    .track_focus(&focus_handle)
                    .on_key_down(move |event: &KeyDownEvent, _window, cx| {
                        if event.keystroke.key == "escape" {
                            cx.stop_propagation();
                            entity_key.update(cx, |state, cx| {
                                state.handle_escape();
                                cx.notify();
                            });
                        }
                    })
                    .absolute()
                    .inset_0()
                    .child(content);

                if should_focus {
                    window.focus(&focus_handle, cx);
                }
                let mut layer = layer.into_any_element();
                ((layer.request_layout(window, cx), layer), focus_state)
            },
        )
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        element: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        gpui::Element::prepaint(element, id, inspector_id, bounds, &mut (), window, cx);
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        element: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        gpui::Element::paint(
            element,
            id,
            inspector_id,
            bounds,
            &mut (),
            _prepaint,
            window,
            cx,
        );
    }
}

/// Renders the 300px User Profile Popout anchored to the click position, with no backdrop.
pub fn render_user_profile_popout(
    app_entity: Entity<AppState>,
    popout: &ProfilePopoutState,
    anchor: Option<Point<Pixels>>,
    catalog: &ProfileEffectCatalog,
    presence: Option<&Presence>,
    quick_state: &Entity<InputState>,
) -> impl IntoElement {
    ProfilePopoutElement {
        app_entity,
        popout: popout.clone(),
        anchor,
        catalog: catalog.clone(),
        presence: presence.cloned(),
        quick_state: quick_state.clone(),
    }
}

fn render_profile_popout_content(
    app_entity: &Entity<AppState>,
    popout: &ProfilePopoutState,
    catalog: &ProfileEffectCatalog,
    presence: Option<&Presence>,
    quick_state: &Entity<InputState>,
    reduce_motion: bool,
    quick_send_in_flight: bool,
    guild_views: &GuildProfileViews,
    anchor_pos: Point<Pixels>,
    anchor_corner: Anchor,
) -> AnyElement {
    let entity_dismiss = app_entity.clone();
    div()
        .size_full()
        // Transparent dismiss backdrop: no dimming and no fabricated overlay color.
        .child(
            div()
                .id("profile-popout-dismiss-backdrop")
                .absolute()
                .inset_0()
                .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                    entity_dismiss.update(cx, |state, cx| {
                        state.close_profile_popout();
                        cx.notify();
                    });
                }),
        )
        .child(deferred(
            anchored()
                .position(anchor_pos)
                .anchor(anchor_corner)
                .snap_to_window_with_margin(px(PROFILE_POPOUT_MARGIN))
                .child(
                    div()
                        .on_mouse_down(MouseButton::Left, |_event, _window, cx| {
                            cx.stop_propagation();
                        })
                        .child(render_profile_card(
                            app_entity.clone(),
                            popout,
                            catalog,
                            presence,
                            quick_state,
                            reduce_motion,
                            quick_send_in_flight,
                            guild_views,
                        )),
                ),
        ))
        .into_any_element()
}

/// Renders the 300px Popout Card container
fn render_profile_card(
    app_entity: Entity<AppState>,
    popout: &ProfilePopoutState,
    catalog: &ProfileEffectCatalog,
    presence: Option<&Presence>,
    quick_state: &Entity<InputState>,
    reduce_motion: bool,
    quick_send_in_flight: bool,
    guild_views: &GuildProfileViews,
) -> AnyElement {
    let mut card = div()
        .id("profile-popout-card")
        .w(px(300.0))
        .max_h(px(580.0))
        .rounded(px(8.0))
        .bg(bg_user_bar())
        .border_1()
        .border_color(bg_server())
        .shadow_lg()
        .overflow_hidden()
        .flex()
        .flex_col();

    card = card.child(render_popout_banner(popout, catalog, reduce_motion));
    card = card.child(render_popout_avatar_row(
        app_entity.clone(),
        popout,
        presence,
    ));

    if popout.is_loading && popout.data.is_none() {
        return card
            .child(
                div()
                    .p(px(24.0))
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(px(14.0))
                            .text_color(text_muted())
                            .child(t("profile.loading")),
                    ),
            )
            .into_any_element();
    }

    if let Some(ref err) = popout.error {
        if popout.data.is_none() {
            return card
                .child(
                    div()
                        .p(px(24.0))
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(px(14.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(legacy_fa777c())
                                .child(t("profile.error")),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(text_muted())
                                .child(err.clone()),
                        ),
                )
                .into_any_element();
        }
    }

    card = card.child(render_popout_body(
        app_entity.clone(),
        popout,
        presence,
        guild_views,
    ));
    card = card.child(render_popout_quick_input(
        popout,
        quick_state,
        quick_send_in_flight,
    ));

    card.into_any_element()
}

/// Renders the 105px top banner with profile effect (sem botão X no popout)
/// Nota: O literal antigo rgba(0x00000088) do botão fechar foi removido pois o popout do Discord não tem botão X
fn render_popout_banner(
    popout: &ProfilePopoutState,
    catalog: &ProfileEffectCatalog,
    reduce_motion: bool,
) -> AnyElement {
    let banner_color_val = profile_banner_color(popout);
    let banner_image_url = profile_banner_image_url(popout, 300);

    let mut banner = div()
        .relative()
        .w(px(300.0))
        .h(px(105.0))
        .bg(rgb(banner_color_val))
        .flex_shrink_0();

    if let Some(ref img_url) = banner_image_url {
        banner = banner.child(
            img(img_url.clone())
                .w_full()
                .h_full()
                .object_fit(ObjectFit::Cover),
        );
    }

    let profile_effect_id = popout
        .data
        .as_ref()
        .and_then(|d| d.profile_effect_id.as_deref());
    let user_effect = profile_effect_id.map(UserProfileEffect::new);
    let effect_overlay = render_profile_effect_for_user_with_motion(
        user_effect.as_ref(),
        catalog,
        px(105.0),
        Some(px(PROFILE_POPOUT_WIDTH)),
        reduce_motion,
    );
    banner = banner.child(effect_overlay);

    banner.into_any_element()
}

fn render_profile_badges(badges: &[ProfileBadge], is_modal: bool) -> Div {
    let badge_size = if is_modal { px(24.0) } else { px(20.0) };
    let icon_size = if is_modal { px(18.0) } else { px(16.0) };
    let mut container = div()
        .flex()
        .flex_row()
        .flex_wrap()
        .gap(px(4.0))
        .items_center();
    if !is_modal {
        container = container.pb(px(4.0));
    }

    for (index, badge) in badges
        .iter()
        .filter(|badge| !badge.icon.trim().is_empty())
        .enumerate()
    {
        let description = badge.description.trim().to_string();
        let mut element = div()
            .id(ElementId::Name(
                format!(
                    "profile-badge-{}-{index}",
                    if is_modal { "modal" } else { "popout" }
                )
                .into(),
            ))
            .size(badge_size)
            .rounded(px(4.0))
            .bg(opacity_white_8())
            .flex()
            .items_center()
            .justify_center()
            .child(img(badge.icon_url()).size(icon_size));
        if !description.is_empty() {
            element = element
                .tooltip(move |window, cx| Tooltip::new(description.clone()).build(window, cx));
        }
        container = container.child(element);
    }
    container
}

/// Renders the overlapping avatar (clickable to open full modal) and badges container
fn render_popout_avatar_row(
    app_entity: Entity<AppState>,
    popout: &ProfilePopoutState,
    presence: Option<&Presence>,
) -> AnyElement {
    let avatar_user = popout.data.as_ref().map(|d| &d.user);
    let avatar_url = avatar_user.and_then(|u| u.avatar_url());
    let display_name = popout
        .data
        .as_ref()
        .map(|data| data.user.display_name().trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_default();

    let presence_status = presence
        .map(|p| p.status)
        .or_else(|| avatar_user.map(|u| u.presence))
        .unwrap_or(PresenceStatus::Offline);

    let avatar_initial = display_name
        .chars()
        .next()
        .unwrap_or('?')
        .to_uppercase()
        .to_string();

    let avatar = Avatar::new(avatar_initial)
        .size(AvatarSize::Px80)
        .opt_src(avatar_url)
        .opt_status(Some(presence_status))
        .opt_decoration(avatar_user.and_then(|u| u.avatar_decoration_data.clone()))
        .cutout_color(bg_user_bar());

    let entity_av_modal = app_entity;
    let avatar_container = div()
        .relative()
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
            entity_av_modal.update(cx, |s, cx| {
                s.open_user_profile_modal();
                cx.notify();
            });
        })
        .child(avatar);

    let badges_container = render_profile_badges(
        popout
            .data
            .as_ref()
            .map(|data| data.badges.as_slice())
            .unwrap_or(&[]),
        false,
    );

    div()
        .px(px(16.0))
        .mt(px(-40.0))
        .flex()
        .flex_row()
        .justify_between()
        .items_end()
        .child(avatar_container)
        .child(badges_container)
        .into_any_element()
}

/// Renders the scrollable popout body
fn render_popout_body(
    app_entity: Entity<AppState>,
    popout: &ProfilePopoutState,
    presence: Option<&Presence>,
    guild_views: &GuildProfileViews,
) -> AnyElement {
    let avatar_user = popout.data.as_ref().map(|data| &data.user);
    let display_name = avatar_user
        .map(|user| user.display_name().trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_default();
    let username = avatar_user
        .map(|user| user.username.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_default();

    let mut body = div()
        .id("profile-popout-body")
        .flex_1()
        .overflow_y_scroll()
        .p(px(16.0))
        .flex()
        .flex_col()
        .gap(px(10.0));

    // Display Name (clickable to open full modal)
    let entity_name_modal = app_entity.clone();
    body = body.child(
        div()
            .text_size(px(20.0))
            .font_weight(FontWeight::BOLD)
            .text_color(white())
            .cursor_pointer()
            .hover(|s| s.underline())
            .on_mouse_down(MouseButton::Left, move |_ev, _win, cx| {
                entity_name_modal.update(cx, |s, cx| {
                    s.open_user_profile_modal();
                    cx.notify();
                });
            })
            .child(display_name),
    );

    // @username and optional pronouns; never turn a snowflake into a fake handle.
    let mut tag_row = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .children((!username.is_empty()).then(|| {
            div()
                .text_size(px(14.0))
                .text_color(legacy_949ba4())
                .child(format!("@{}", username))
        }));

    if let Some(pronouns) = popout
        .data
        .as_ref()
        .and_then(|data| data.pronouns.as_deref())
        .map(str::trim)
        .filter(|pronouns| !pronouns.is_empty())
    {
        tag_row = tag_row.child(
            div()
                .px(px(6.0))
                .py(px(2.0))
                .rounded(px(4.0))
                .bg(opacity_white_8())
                .text_size(px(12.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(text_primary())
                .child(pronouns.to_string()),
        );
    }
    body = body.child(tag_row);

    if let Some(status_el) = render_popout_status(popout, presence) {
        body = body.child(status_el);
    }
    if let Some(game_el) = render_popout_activity(popout, presence) {
        body = body.child(game_el);
    }

    body = body.child(div().h_px().bg(channel_hover()).my(px(2.0)));

    if let Some(bio_el) = render_popout_bio(popout) {
        body = body.child(bio_el);
        body = body.child(div().h_px().bg(channel_hover()).my(px(2.0)));
    }

    let created_at = popout
        .data
        .as_ref()
        .and_then(|d| d.created_at)
        .or_else(|| snowflake_to_datetime(&popout.user_id));
    let joined_server_at = popout.data.as_ref().and_then(|d| d.joined_server_at);

    if let Some(dates_el) = render_member_dates_content(created_at, joined_server_at) {
        body = body.child(dates_el);
        body = body.child(div().h_px().bg(channel_hover()).my(px(2.0)));
    }

    body = body.child(render_popout_tabs(app_entity.clone(), popout, guild_views));

    // "Ver perfil completo" action button
    let entity_full = app_entity;
    body = body.child(
        Button::ghost("popout-btn-view-full", "Ver perfil completo")
            .size(ButtonSize::Sm)
            .full_width()
            .on_click(move |_ev, _win, cx| {
                entity_full.update(cx, |s, cx| {
                    s.open_user_profile_modal();
                    cx.notify();
                });
            }),
    );

    body.into_any_element()
}

/// Renders custom status text/emoji if present
fn render_popout_status(
    popout: &ProfilePopoutState,
    presence: Option<&Presence>,
) -> Option<AnyElement> {
    let avatar_user = popout.data.as_ref().map(|d| &d.user);
    let custom_status: Option<CustomStatus> =
        presence.and_then(|p| p.custom_status.clone()).or_else(|| {
            avatar_user
                .and_then(|u| u.activity.as_ref())
                .and_then(|act| act.state.as_ref().map(|s| CustomStatus::from_text(s)))
        });

    if let Some(cs) = custom_status {
        let emoji_el: Option<AnyElement> = cs.emoji.as_ref().and_then(|emoji| {
            if let Some(url) = emoji.image_url() {
                Some(img(url).size(px(16.0)).into_any_element())
            } else if !emoji.name.trim().is_empty() {
                Some(
                    div()
                        .text_size(px(14.0))
                        .child(emoji.name.clone())
                        .into_any_element(),
                )
            } else {
                None
            }
        });
        let status_text = cs
            .text
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty());

        if status_text.is_some() || emoji_el.is_some() {
            return Some(
                div()
                    .p(px(8.0))
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(channel_hover())
                    .bg(bg_server())
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.0))
                    .children(emoji_el)
                    .children(status_text.map(|text| {
                        div()
                            .text_size(px(13.0))
                            .text_color(text_primary())
                            .child(text.to_string())
                    }))
                    .into_any_element(),
            );
        }
    }
    None
}

/// Renders live game activity if present
fn render_popout_activity(
    popout: &ProfilePopoutState,
    presence: Option<&Presence>,
) -> Option<AnyElement> {
    let avatar_user = popout.data.as_ref().map(|d| &d.user);
    let game = presence
        .and_then(|presence| presence.activity.as_ref())
        .or_else(|| avatar_user.and_then(|user| user.activity.as_ref()))
        .filter(|activity| {
            activity.activity_type != ActivityType::Custom && !activity.name.trim().is_empty()
        })?;
    let activity_label = match game.activity_type {
        ActivityType::Playing => "JOGANDO",
        ActivityType::Streaming => "TRANSMITINDO",
        ActivityType::Listening => "OUVINDO",
        ActivityType::Watching => "ASSISTINDO",
        ActivityType::Competing => "COMPETINDO",
        ActivityType::Custom => return None,
    };

    Some(
        div()
            .p(px(8.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(channel_hover())
            .bg(bg_server())
            .flex()
            .flex_col()
            .gap(px(2.0))
            .child(
                div()
                    .text_size(px(10.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(status_online())
                    .child(activity_label),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(white())
                    .child(game.name.clone()),
            )
            .children(game.details.as_ref().map(|d| {
                div()
                    .text_size(px(12.0))
                    .text_color(text_primary())
                    .child(d.clone())
            }))
            .children(game.state.as_ref().map(|s| {
                div()
                    .text_size(px(12.0))
                    .text_color(legacy_949ba4())
                    .child(s.clone())
            }))
            .into_any_element(),
    )
}

/// Renders "Sobre mim" bio section with automatic line wrapping within 300px popout
fn render_popout_bio(popout: &ProfilePopoutState) -> Option<AnyElement> {
    let bio = popout.data.as_ref().and_then(|d| d.bio.as_ref())?;
    if bio.trim().is_empty() {
        return None;
    }

    Some(
        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(
                div()
                    .text_size(px(11.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(text_secondary())
                    .child(t("profile.about_me").to_uppercase()),
            )
            .child(render_bio_content(bio))
            .into_any_element(),
    )
}

/// Renders tabs and tab content in the popout
fn render_popout_tabs(
    app_entity: Entity<AppState>,
    popout: &ProfilePopoutState,
    guild_views: &GuildProfileViews,
) -> AnyElement {
    let mutual_guilds = popout
        .data
        .as_ref()
        .map(|d| d.mutual_guilds.as_slice())
        .unwrap_or(&[]);
    let mutual_guilds_count = mutual_guilds.len();
    let mutual_friends_count = popout
        .data
        .as_ref()
        .map(|d| d.mutual_friends_count)
        .unwrap_or(0);

    let is_guilds_tab = popout.active_tab == ProfileTab::MutualGuilds;
    let entity_tab_guilds = app_entity.clone();
    let entity_tab_friends = app_entity;

    let tab_header = div()
        .flex()
        .flex_row()
        .gap(px(12.0))
        .border_b_1()
        .border_color(channel_hover())
        .pb(px(4.0))
        .child(
            div()
                .cursor_pointer()
                .text_size(px(12.0))
                .font_weight(FontWeight::BOLD)
                .text_color(if is_guilds_tab {
                    white()
                } else {
                    legacy_949ba4()
                })
                .border_b_2()
                .border_color(if is_guilds_tab {
                    blurple()
                } else {
                    transparent()
                })
                .pb(px(4.0))
                .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                    entity_tab_guilds.update(cx, |s, cx| {
                        if let Some(ref mut p) = s.profile_popout {
                            p.active_tab = ProfileTab::MutualGuilds;
                        }
                        cx.notify();
                    });
                })
                .child(format!("Servidores ({})", mutual_guilds_count)),
        )
        .child(
            div()
                .cursor_pointer()
                .text_size(px(12.0))
                .font_weight(FontWeight::BOLD)
                .text_color(if !is_guilds_tab {
                    white()
                } else {
                    legacy_949ba4()
                })
                .border_b_2()
                .border_color(if !is_guilds_tab {
                    blurple()
                } else {
                    transparent()
                })
                .pb(px(4.0))
                .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                    entity_tab_friends.update(cx, |s, cx| {
                        if let Some(ref mut p) = s.profile_popout {
                            p.active_tab = ProfileTab::MutualFriends;
                        }
                        cx.notify();
                    });
                })
                .child(format!("Amigos ({})", mutual_friends_count)),
        );

    let tab_content = if is_guilds_tab {
        render_mutual_guilds_list(mutual_guilds, guild_views, false)
    } else {
        render_mutual_friends_content(mutual_friends_count, false)
    };

    div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .child(tab_header)
        .child(tab_content)
        .into_any_element()
}

/// Renders quick direct message input field at the bottom of the popout (show_paste(false) per Parity)
fn render_popout_quick_input(
    popout: &ProfilePopoutState,
    quick_state: &Entity<InputState>,
    quick_send_in_flight: bool,
) -> AnyElement {
    let target_uname = popout
        .data
        .as_ref()
        .map(|data| data.user.username.trim().to_string())
        .filter(|username| !username.is_empty());
    let placeholder = target_uname
        .as_ref()
        .map(|username| t("profile.message_placeholder").replace("{username}", username))
        .unwrap_or_else(|| "Enviar mensagem...".to_string());
    let disabled = target_uname.is_none() || quick_send_in_flight;

    let mut container = div()
        .p(px(12.0))
        .border_t_1()
        .border_color(bg_server())
        .bg(bg_server())
        .flex()
        .flex_col()
        .gap(px(6.0));

    if let Some(ref err) = popout.error {
        container = container.child(
            div()
                .text_size(px(12.0))
                .text_color(status_danger())
                .child(err.clone()),
        );
    }

    container
        .child(
            TextInput::new("profile-quick-message-input", quick_state)
                .placeholder(placeholder)
                .show_paste(false)
                .show_clear(false)
                .disabled(disabled)
                .full_width(true),
        )
        .into_any_element()
}

// ============================================================================
// Component 2: Full User Profile Modal (1184x704px Dialog, 3 Tabs)
// ============================================================================

/// Builds the Dialog for the full User Profile Modal (opened exclusively from the popout)
/// UMA caixa só (1184x704 r12 com backdrop e sem moldura dupla) e UM botão de fechar (DialogClose do kit)
pub fn build_user_profile_modal_dialog(
    dialog: Dialog,
    entity: Entity<AppState>,
    vh: Pixels,
    cx: &mut App,
) -> Dialog {
    let entity_close = entity.clone();
    let state = entity.read(cx);

    let popout = match state.profile_popout {
        Some(ref p) => p.clone(),
        None => return dialog,
    };

    let catalog = state.profile_effects_catalog.clone();
    let guild_views = guild_profile_views(&state.guilds);
    let roles = profile_roles_for_popout(&popout);
    let presence = state.presences_by_user.get(&popout.user_id).cloned();
    let active_tab = state.profile_modal_tab;
    let reduce_motion = state.accessibility_settings.reduce_motion;

    let margin_top = (vh - px(704.0)).max(px(20.0)) / 2.0;

    let content = div()
        .size_full()
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(render_modal_banner(&popout, &catalog, reduce_motion))
        .child(render_modal_header(&popout, presence.as_ref()))
        .child(render_modal_tabs_bar(entity.clone(), &popout, active_tab))
        .child(render_modal_tab_content(
            &popout,
            active_tab,
            &guild_views,
            &roles,
        ));

    dialog
        .w(px(1184.0))
        .h(px(704.0))
        .margin_top(margin_top)
        .p_0()
        .bg(bg_user_bar())
        .rounded(px(12.0))
        .border_1()
        .border_color(bg_server())
        .overflow_hidden()
        .close_button(true)
        .on_close(move |_event, _window, cx| {
            entity_close.update(cx, |s, cx| {
                s.close_user_profile_modal();
                cx.notify();
            });
        })
        .child(content)
}

/// Renders the top modal banner with effect overlay (sem botão X duplicado; o X pertence ao Dialog)
fn render_modal_banner(
    popout: &ProfilePopoutState,
    catalog: &ProfileEffectCatalog,
    reduce_motion: bool,
) -> AnyElement {
    let banner_color_val = profile_banner_color(popout);
    let banner_image_url = profile_banner_image_url(popout, 600);

    let mut banner = div()
        .relative()
        .w_full()
        .h(px(210.0))
        .bg(rgb(banner_color_val))
        .flex_shrink_0();

    if let Some(ref img_url) = banner_image_url {
        banner = banner.child(
            img(img_url.clone())
                .w_full()
                .h_full()
                .object_fit(ObjectFit::Cover),
        );
    }

    let profile_effect_id = popout
        .data
        .as_ref()
        .and_then(|d| d.profile_effect_id.as_deref());
    let user_effect = profile_effect_id.map(UserProfileEffect::new);
    let effect_overlay = render_profile_effect_for_user_with_motion(
        user_effect.as_ref(),
        catalog,
        px(210.0),
        Some(px(1184.0)),
        reduce_motion,
    );
    banner = banner.child(effect_overlay);

    banner.into_any_element()
}

/// Renders the profile header row inside the modal
fn render_modal_header(popout: &ProfilePopoutState, presence: Option<&Presence>) -> AnyElement {
    let avatar_user = popout.data.as_ref().map(|d| &d.user);
    let avatar_url = avatar_user.and_then(|u| u.avatar_url());
    let display_name = avatar_user
        .map(|user| user.display_name().trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_default();
    let username = avatar_user
        .map(|user| user.username.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_default();
    let pronouns = popout
        .data
        .as_ref()
        .and_then(|data| data.pronouns.as_deref())
        .map(str::trim)
        .filter(|pronouns| !pronouns.is_empty());

    let presence_status = presence
        .map(|p| p.status)
        .or_else(|| avatar_user.map(|u| u.presence))
        .unwrap_or(PresenceStatus::Offline);

    let avatar_initial = display_name
        .chars()
        .next()
        .unwrap_or('?')
        .to_uppercase()
        .to_string();

    let avatar = Avatar::new(avatar_initial)
        .size(AvatarSize::Px80)
        .opt_src(avatar_url)
        .opt_status(Some(presence_status))
        .opt_decoration(avatar_user.and_then(|u| u.avatar_decoration_data.clone()))
        .cutout_color(bg_user_bar());

    let badges = popout
        .data
        .as_ref()
        .map(|data| data.badges.as_slice())
        .unwrap_or(&[]);

    div()
        .px(px(24.0))
        .mt(px(-40.0))
        .flex()
        .flex_row()
        .justify_between()
        .items_end()
        .child(
            div()
                .flex()
                .flex_row()
                .items_end()
                .gap(px(16.0))
                .child(avatar)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.0))
                        .child(
                            div()
                                .text_size(px(24.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(white())
                                .child(display_name),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(8.0))
                                .children((!username.is_empty()).then(|| {
                                    div()
                                        .text_size(px(14.0))
                                        .text_color(legacy_949ba4())
                                        .child(format!("@{}", username))
                                }))
                                .children(pronouns.map(|pronouns| {
                                    div()
                                        .px(px(6.0))
                                        .py(px(2.0))
                                        .rounded(px(4.0))
                                        .bg(opacity_white_8())
                                        .text_size(px(12.0))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(text_primary())
                                        .child(pronouns.to_string())
                                })),
                        ),
                ),
        )
        .child(render_profile_badges(badges, true))
        .into_any_element()
}

/// Renders the 3 canonical profile modal tabs bar
fn render_modal_tabs_bar(
    app_entity: Entity<AppState>,
    popout: &ProfilePopoutState,
    active_tab: ProfileModalTab,
) -> AnyElement {
    let mutual_guilds_count = popout
        .data
        .as_ref()
        .map(|d| d.mutual_guilds.len())
        .unwrap_or(0);
    let mutual_friends_count = popout
        .data
        .as_ref()
        .map(|d| d.mutual_friends_count)
        .unwrap_or(0);

    let e_tab1 = app_entity.clone();
    let e_tab2 = app_entity.clone();
    let e_tab3 = app_entity;

    let is_about = active_tab == ProfileModalTab::AboutMe;
    let is_guilds = active_tab == ProfileModalTab::MutualGuilds;
    let is_friends = active_tab == ProfileModalTab::MutualFriends;

    div()
        .mt(px(16.0))
        .px(px(24.0))
        .flex()
        .flex_row()
        .gap(px(24.0))
        .border_b_1()
        .border_color(channel_hover())
        .child(
            div()
                .cursor_pointer()
                .text_size(px(14.0))
                .font_weight(FontWeight::BOLD)
                .text_color(if is_about { white() } else { legacy_949ba4() })
                .border_b_2()
                .border_color(if is_about { blurple() } else { transparent() })
                .pb(px(8.0))
                .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                    e_tab1.update(cx, |s, cx| {
                        s.set_profile_modal_tab(ProfileModalTab::AboutMe);
                        cx.notify();
                    });
                })
                .child("Sobre mim"),
        )
        .child(
            div()
                .cursor_pointer()
                .text_size(px(14.0))
                .font_weight(FontWeight::BOLD)
                .text_color(if is_guilds { white() } else { legacy_949ba4() })
                .border_b_2()
                .border_color(if is_guilds { blurple() } else { transparent() })
                .pb(px(8.0))
                .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                    e_tab2.update(cx, |s, cx| {
                        s.set_profile_modal_tab(ProfileModalTab::MutualGuilds);
                        cx.notify();
                    });
                })
                .child(format!("Servidores em comum ({})", mutual_guilds_count)),
        )
        .child(
            div()
                .cursor_pointer()
                .text_size(px(14.0))
                .font_weight(FontWeight::BOLD)
                .text_color(if is_friends { white() } else { legacy_949ba4() })
                .border_b_2()
                .border_color(if is_friends { blurple() } else { transparent() })
                .pb(px(8.0))
                .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                    e_tab3.update(cx, |s, cx| {
                        s.set_profile_modal_tab(ProfileModalTab::MutualFriends);
                        cx.notify();
                    });
                })
                .child(format!("Amigos em comum ({})", mutual_friends_count)),
        )
        .into_any_element()
}

fn profile_roles_for_popout(popout: &ProfilePopoutState) -> Vec<crate::domain::guild::Role> {
    let Some(guild_id) = popout.guild_id.as_deref() else {
        return Vec::new();
    };
    let Some(role_ids) =
        crate::protocol::cache::lazy_guilds::get_cached_author_roles(guild_id, &popout.user_id)
    else {
        return Vec::new();
    };
    let guild_roles = crate::protocol::cache::lazy_guilds::get_cached_guild_roles(guild_id);
    let mut roles: Vec<_> = role_ids
        .iter()
        .filter_map(|role_id| guild_roles.iter().find(|role| role.id == *role_id))
        .filter(|role| role.id != guild_id && !role.name.trim().is_empty())
        .cloned()
        .collect();
    roles.sort_by(|left, right| {
        right
            .position
            .cmp(&left.position)
            .then_with(|| left.id.cmp(&right.id))
    });
    roles
}

fn render_profile_roles(roles: &[crate::domain::guild::Role]) -> Option<AnyElement> {
    if roles.is_empty() {
        return None;
    }

    let mut pills = div().flex().flex_row().flex_wrap().gap(space_4());
    for role in roles {
        let dot_color = if role.color == 0 {
            interactive_normal()
        } else {
            rgb(role.color & 0xffffff)
        };
        pills = pills.child(
            div()
                .px(space_4())
                .py(space_2())
                .rounded(radius_sm())
                .bg(bg_server())
                .border_1()
                .border_color(channel_hover())
                .flex()
                .flex_row()
                .items_center()
                .gap(space_3())
                .child(div().size(space_4()).rounded_full().bg(dot_color))
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(text_primary())
                        .child(role.name.clone()),
                ),
        );
    }

    Some(
        div()
            .flex()
            .flex_col()
            .gap(space_4())
            .child(
                div()
                    .text_size(px(12.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(text_secondary())
                    .child("CARGOS"),
            )
            .child(pills)
            .into_any_element(),
    )
}

/// Renders the modal content corresponding to the selected tab
fn render_modal_tab_content(
    popout: &ProfilePopoutState,
    active_tab: ProfileModalTab,
    guild_views: &GuildProfileViews,
    roles: &[crate::domain::guild::Role],
) -> AnyElement {
    let mut container = div()
        .id("modal-tab-content-container")
        .flex_1()
        .overflow_y_scroll()
        .p(px(24.0));

    match active_tab {
        ProfileModalTab::AboutMe => {
            container = container.child(render_modal_about_me(popout, roles));
        }
        ProfileModalTab::MutualGuilds => {
            let guilds = popout
                .data
                .as_ref()
                .map(|d| d.mutual_guilds.as_slice())
                .unwrap_or(&[]);
            container = container.child(render_mutual_guilds_list(guilds, guild_views, true));
        }
        ProfileModalTab::MutualFriends => {
            let count = popout
                .data
                .as_ref()
                .map(|d| d.mutual_friends_count)
                .unwrap_or(0);
            container = container.child(render_mutual_friends_content(count, true));
        }
    }

    container.into_any_element()
}

/// Renders "Sobre mim" tab content (Bio, Connected accounts, Member dates)
fn render_modal_about_me(
    popout: &ProfilePopoutState,
    roles: &[crate::domain::guild::Role],
) -> AnyElement {
    let mut col = div().flex().flex_col().gap(px(16.0));

    if let Some(ref bio) = popout.data.as_ref().and_then(|d| d.bio.as_ref()) {
        if !bio.trim().is_empty() {
            col = col.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(text_secondary())
                            .child("BIOGRAFIA"),
                    )
                    .child(render_bio_content(bio)),
            );
        }
    }

    if let Some(roles_el) = render_profile_roles(roles) {
        col = col.child(roles_el);
    }

    let created_at = popout
        .data
        .as_ref()
        .and_then(|d| d.created_at)
        .or_else(|| snowflake_to_datetime(&popout.user_id));
    let joined_server_at = popout.data.as_ref().and_then(|d| d.joined_server_at);

    if let Some(dates_el) = render_member_dates_content(created_at, joined_server_at) {
        col = col.child(dates_el);
    }

    if let Some(data) = popout.data.as_ref() {
        if !data.connected_accounts.is_empty() {
            let mut acc_col = div().flex().flex_col().gap(px(8.0)).child(
                div()
                    .text_size(px(12.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(text_secondary())
                    .child("CONTAS CONECTADAS"),
            );
            let mut acc_grid = div().flex().flex_row().flex_wrap().gap(px(8.0));
            for acc in &data.connected_accounts {
                acc_grid = acc_grid.child(
                    div()
                        .px(px(12.0))
                        .py(px(8.0))
                        .rounded(px(6.0))
                        .bg(bg_server())
                        .border_1()
                        .border_color(channel_hover())
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(text_primary())
                                .child(acc.name.clone()),
                        ),
                );
            }
            acc_col = acc_col.child(acc_grid);
            col = col.child(acc_col);
        }
    }

    col.into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::guild::Role;

    #[::core::prelude::v1::test]
    fn test_parse_hex_color_accepts_only_rgb() {
        assert_eq!(parse_hex_color("#5865f2"), Some(0x5865f2));
        assert_eq!(parse_hex_color(" 110fbf "), Some(0x110fbf));
        assert_eq!(parse_hex_color("#fff"), None);
        assert_eq!(parse_hex_color("#5865f288"), None);
        assert_eq!(parse_hex_color("#gggggg"), None);
    }

    #[::core::prelude::v1::test]
    fn test_popout_placement_uses_viewport_and_click_side() {
        let (position, corner) = resolve_profile_popout_anchor(
            Some(point(px(500.0), px(100.0))),
            size(px(800.0), px(600.0)),
        );
        assert_eq!(position, point(px(492.0), px(100.0)));
        assert_eq!(corner, Anchor::TopRight);

        let (position, corner) = resolve_profile_popout_anchor(
            Some(point(px(950.0), px(100.0))),
            size(px(1280.0), px(800.0)),
        );
        assert_eq!(position, point(px(958.0), px(100.0)));
        assert_eq!(corner, Anchor::TopLeft);

        let (position, corner) = resolve_profile_popout_anchor(
            Some(point(px(1200.0), px(100.0))),
            size(px(1920.0), px(1080.0)),
        );
        assert_eq!(position, point(px(1208.0), px(100.0)));
        assert_eq!(corner, Anchor::TopLeft);

        let (position, corner) = resolve_profile_popout_anchor(None, size(px(800.0), px(600.0)));
        assert_eq!(position, point(px(792.0), px(60.0)));
        assert_eq!(corner, Anchor::TopRight);
    }

    #[::core::prelude::v1::test]
    fn test_profile_roles_use_real_cache_and_position_order() {
        let guild_id = "synth-profile-roles-guild";
        let user_id = "synth-profile-roles-user";
        let roles = vec![
            Role::new(guild_id, "@everyone").with_position(100),
            Role::new("synth-role-mod", "Moderator")
                .with_color(0x2ecc71)
                .with_position(5),
            Role::new("synth-role-admin", "Admin")
                .with_color(0xe91e63)
                .with_position(10),
        ];
        crate::protocol::cache::lazy_guilds::cache_guild_roles(guild_id, roles);
        crate::protocol::cache::lazy_guilds::cache_author_roles(
            guild_id,
            user_id,
            vec![
                guild_id.into(),
                "synth-role-mod".into(),
                "synth-role-admin".into(),
            ],
        );

        let mut state = AppState::new();
        state.open_user_profile_in_guild(user_id, Some(guild_id.into()), None);
        let resolved = profile_roles_for_popout(state.profile_popout.as_ref().unwrap());
        assert_eq!(
            resolved
                .iter()
                .map(|role| role.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Admin", "Moderator"]
        );
        assert_eq!(resolved[0].color, 0xe91e63);
        assert_eq!(resolved[1].color, 0x2ecc71);
    }
}
