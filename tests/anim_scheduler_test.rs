use std::time::Duration;

use discord_rs::media::{
    AnimationId, AnimationScheduler, AnimationSpec, PlaybackState, Rect, ReducedMotion,
    SchedulerError,
};

fn viewport(y: f32, height: f32) -> Rect {
    Rect::new(0.0, y, 400.0, height)
}

fn active_scheduler() -> AnimationScheduler {
    AnimationScheduler::new(viewport(0.0, 400.0), ReducedMotion::Allow)
}

fn frame_rect(y: f32) -> Rect {
    Rect::new(8.0, y, 64.0, 64.0)
}

#[test]
fn spec_accessors_reflect_real_frame_timing() {
    let spec = AnimationSpec::from_millis(&[80, 120, 40]).expect("valid frames");
    assert_eq!(spec.frame_count(), 3);
    assert_eq!(spec.frame_duration(0), Some(Duration::from_millis(80)));
    assert_eq!(spec.frame_duration(1), Some(Duration::from_millis(120)));
    assert_eq!(spec.frame_duration(2), Some(Duration::from_millis(40)));
    assert_eq!(spec.frame_duration(3), None);
    assert_eq!(spec.total_duration(), Duration::from_millis(240));
    assert_eq!(spec.loop_count(), 0);
    assert!(spec.is_looping());

    let finite = spec.with_loop_count(3);
    assert_eq!(finite.loop_count(), 3);
    assert!(!finite.is_looping());
}

#[test]
fn spec_rejects_empty_and_zero_duration_frames() {
    assert_eq!(AnimationSpec::new(Vec::new()), Err(SchedulerError::NoFrames));
    assert_eq!(
        AnimationSpec::from_millis(&[]),
        Err(SchedulerError::NoFrames)
    );
    assert_eq!(
        AnimationSpec::new(vec![Duration::from_millis(50), Duration::ZERO]),
        Err(SchedulerError::ZeroFrameDuration { index: 1 })
    );
    assert_eq!(
        AnimationSpec::from_millis(&[0, 10]),
        Err(SchedulerError::ZeroFrameDuration { index: 0 })
    );
}

#[test]
fn rect_intersection_requires_positive_overlap() {
    let view = viewport(0.0, 100.0);
    assert!(frame_rect(0.0).intersects(&view));
    assert!(frame_rect(50.0).intersects(&view));
    assert!(!frame_rect(100.0).intersects(&view));
    assert!(!Rect::new(8.0, 0.0, 0.0, 64.0).intersects(&view));
    assert!(!Rect::new(0.0, 0.0, 400.0, 0.0).intersects(&view));
}

#[test]
fn frames_advance_only_after_their_real_duration() {
    let mut scheduler = active_scheduler();
    let id = scheduler.register(
        AnimationSpec::from_millis(&[100, 100, 100, 100]).expect("valid frames"),
        frame_rect(0.0),
    );

    let initial = scheduler.tick(Duration::ZERO);
    assert_eq!(initial.len(), 1);
    assert_eq!(initial[0].id, id);
    assert_eq!(initial[0].frame, 0);
    assert_eq!(initial[0].state, PlaybackState::Playing);
    assert!(scheduler.tick(Duration::ZERO).is_empty());

    assert!(scheduler.tick(Duration::from_millis(99)).is_empty());
    assert_eq!(scheduler.frame(id), Some(0));

    let second = scheduler.tick(Duration::from_millis(1));
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].frame, 1);

    scheduler.tick(Duration::from_millis(100));
    assert_eq!(scheduler.frame(id), Some(2));
    scheduler.tick(Duration::from_millis(100));
    assert_eq!(scheduler.frame(id), Some(3));
    scheduler.tick(Duration::from_millis(100));
    assert_eq!(scheduler.frame(id), Some(0));
    assert_eq!(scheduler.state(id), Some(PlaybackState::Playing));
}

#[test]
fn pauses_offscreen_and_resumes_from_same_frame() {
    let mut scheduler = AnimationScheduler::new(viewport(0.0, 100.0), ReducedMotion::Allow);
    let id = scheduler.register(
        AnimationSpec::from_millis(&[100, 100, 100, 100]).expect("valid frames"),
        frame_rect(200.0),
    );

    scheduler.tick(Duration::from_secs(1));
    assert_eq!(scheduler.frame(id), Some(0));
    assert_eq!(scheduler.state(id), Some(PlaybackState::PausedOffscreen));

    scheduler.set_viewport(viewport(150.0, 200.0));
    assert_eq!(scheduler.state(id), Some(PlaybackState::Playing));

    scheduler.tick(Duration::from_millis(250));
    assert_eq!(scheduler.frame(id), Some(2));

    scheduler.set_viewport(viewport(1000.0, 100.0));
    scheduler.tick(Duration::from_secs(1));
    assert_eq!(scheduler.frame(id), Some(2));
    assert_eq!(scheduler.state(id), Some(PlaybackState::PausedOffscreen));

    scheduler.set_viewport(viewport(150.0, 200.0));
    scheduler.tick(Duration::from_millis(50));
    assert_eq!(scheduler.frame(id), Some(3));
    scheduler.tick(Duration::from_millis(100));
    assert_eq!(scheduler.frame(id), Some(0));
}

#[test]
fn partially_visible_animation_keeps_playing() {
    let mut scheduler = AnimationScheduler::new(viewport(0.0, 100.0), ReducedMotion::Allow);
    let id = scheduler.register(
        AnimationSpec::from_millis(&[100, 100]).expect("valid frames"),
        frame_rect(90.0),
    );

    assert_eq!(scheduler.state(id), Some(PlaybackState::Playing));
    scheduler.tick(Duration::from_millis(100));
    assert_eq!(scheduler.frame(id), Some(1));
}

#[test]
fn reduced_motion_holds_first_frame_until_disabled() {
    let mut scheduler = AnimationScheduler::new(viewport(0.0, 400.0), ReducedMotion::Pause);
    let id = scheduler.register(
        AnimationSpec::from_millis(&[100, 100, 100]).expect("valid frames"),
        frame_rect(0.0),
    );

    let first = scheduler.tick(Duration::from_secs(1));
    assert_eq!(scheduler.frame(id), Some(0));
    assert_eq!(
        scheduler.state(id),
        Some(PlaybackState::PausedReducedMotion)
    );
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].state, PlaybackState::PausedReducedMotion);
    assert!(scheduler.tick(Duration::from_secs(5)).is_empty());
    assert!(!scheduler.has_active_animations());

    scheduler.set_reduced_motion(ReducedMotion::Allow);
    assert_eq!(scheduler.state(id), Some(PlaybackState::Playing));
    scheduler.tick(Duration::from_millis(100));
    assert_eq!(scheduler.frame(id), Some(1));

    scheduler.set_reduced_motion(ReducedMotion::Pause);
    assert_eq!(scheduler.frame(id), Some(0));
    scheduler.tick(Duration::from_secs(1));
    assert_eq!(scheduler.frame(id), Some(0));
}

#[test]
fn reduced_motion_takes_precedence_over_offscreen_state() {
    let mut scheduler = AnimationScheduler::new(viewport(0.0, 100.0), ReducedMotion::Pause);
    let id = scheduler.register(
        AnimationSpec::from_millis(&[100]).expect("valid frame"),
        frame_rect(500.0),
    );

    assert_eq!(
        scheduler.state(id),
        Some(PlaybackState::PausedReducedMotion)
    );
    assert_eq!(
        ReducedMotion::from_reduce_motion(true),
        ReducedMotion::Pause
    );
    assert_eq!(
        ReducedMotion::from_reduce_motion(false),
        ReducedMotion::Allow
    );
}

#[test]
fn finite_loop_count_finishes_on_last_frame() {
    let mut scheduler = active_scheduler();
    let id = scheduler.register(
        AnimationSpec::from_millis(&[100, 100])
            .expect("valid frames")
            .with_loop_count(2),
        frame_rect(0.0),
    );

    scheduler.tick(Duration::from_millis(399));
    assert_eq!(scheduler.frame(id), Some(1));
    assert_eq!(scheduler.state(id), Some(PlaybackState::Playing));

    scheduler.tick(Duration::from_millis(1));
    assert_eq!(scheduler.frame(id), Some(1));
    assert_eq!(scheduler.state(id), Some(PlaybackState::Finished));
    assert!(!scheduler.has_active_animations());

    assert!(scheduler.tick(Duration::from_secs(10)).is_empty());
    assert_eq!(scheduler.frame(id), Some(1));
}

#[test]
fn single_play_finishes_after_one_cycle() {
    let mut scheduler = active_scheduler();
    let id = scheduler.register(
        AnimationSpec::from_millis(&[50, 50])
            .expect("valid frames")
            .with_loop_count(1),
        frame_rect(0.0),
    );

    scheduler.tick(Duration::from_millis(50));
    assert_eq!(scheduler.frame(id), Some(1));
    assert_eq!(scheduler.state(id), Some(PlaybackState::Playing));
    scheduler.tick(Duration::from_millis(50));
    assert_eq!(scheduler.state(id), Some(PlaybackState::Finished));
}

#[test]
fn large_delta_spans_multiple_cycles() {
    let mut scheduler = active_scheduler();
    let id = scheduler.register(
        AnimationSpec::from_millis(&[10, 10, 10, 10]).expect("valid frames"),
        frame_rect(0.0),
    );

    scheduler.tick(Duration::from_millis(125));
    assert_eq!(scheduler.frame(id), Some(0));
    scheduler.tick(Duration::from_millis(5));
    assert_eq!(scheduler.frame(id), Some(1));
}

#[test]
fn animations_track_viewport_independently() {
    let mut scheduler = AnimationScheduler::new(viewport(0.0, 100.0), ReducedMotion::Allow);
    let visible = scheduler.register(
        AnimationSpec::from_millis(&[100, 100, 100, 100]).expect("valid frames"),
        frame_rect(0.0),
    );
    let hidden = scheduler.register(
        AnimationSpec::from_millis(&[100, 100, 100, 100]).expect("valid frames"),
        frame_rect(500.0),
    );

    scheduler.tick(Duration::from_secs(1));
    assert_eq!(scheduler.frame(visible), Some(2));
    assert_eq!(scheduler.frame(hidden), Some(0));
    assert_eq!(
        scheduler.state(hidden),
        Some(PlaybackState::PausedOffscreen)
    );
}

#[test]
fn tick_reports_frame_and_state_changes_once() {
    let mut scheduler = active_scheduler();
    let id = scheduler.register(
        AnimationSpec::from_millis(&[100, 100]).expect("valid frames"),
        frame_rect(0.0),
    );

    let initial = scheduler.tick(Duration::ZERO);
    assert_eq!(initial.len(), 1);
    assert_eq!(initial[0].id, id);
    assert!(scheduler.tick(Duration::ZERO).is_empty());

    let advanced = scheduler.tick(Duration::from_millis(100));
    assert_eq!(advanced.len(), 1);
    assert_eq!(advanced[0].frame, 1);

    scheduler.set_viewport(viewport(1000.0, 100.0));
    let hidden = scheduler.tick(Duration::ZERO);
    assert_eq!(hidden.len(), 1);
    assert_eq!(hidden[0].state, PlaybackState::PausedOffscreen);
    assert_eq!(hidden[0].frame, 1);
    assert!(scheduler.tick(Duration::from_secs(1)).is_empty());

    scheduler.unregister(id);
    assert!(scheduler.tick(Duration::ZERO).is_empty());
}

#[test]
fn update_rect_moves_animation_between_offscreen_and_visible() {
    let mut scheduler = AnimationScheduler::new(viewport(0.0, 100.0), ReducedMotion::Allow);
    let id = scheduler.register(
        AnimationSpec::from_millis(&[100, 100]).expect("valid frames"),
        frame_rect(500.0),
    );

    scheduler.tick(Duration::from_secs(1));
    assert_eq!(scheduler.frame(id), Some(0));

    scheduler
        .update_rect(id, frame_rect(10.0))
        .expect("known animation");
    assert_eq!(scheduler.state(id), Some(PlaybackState::Playing));
    scheduler.tick(Duration::from_millis(100));
    assert_eq!(scheduler.frame(id), Some(1));

    scheduler.update_rect(id, frame_rect(500.0)).expect("known animation");
    scheduler.tick(Duration::from_secs(1));
    assert_eq!(scheduler.frame(id), Some(1));
}

#[test]
fn unknown_ids_return_none_and_errors() {
    let mut scheduler = active_scheduler();
    let id = scheduler.register(
        AnimationSpec::from_millis(&[100]).expect("valid frame"),
        frame_rect(0.0),
    );
    assert_eq!(scheduler.len(), 1);
    assert_eq!(scheduler.ids().collect::<Vec<_>>(), vec![id]);
    assert_eq!(scheduler.spec(id).map(AnimationSpec::frame_count), Some(1));

    assert!(scheduler.unregister(id));
    assert_eq!(scheduler.frame(id), None);
    assert_eq!(scheduler.state(id), None);
    assert_eq!(scheduler.spec(id), None);
    assert!(!scheduler.unregister(id));
    assert_eq!(
        scheduler.update_rect(id, frame_rect(0.0)),
        Err(SchedulerError::UnknownAnimation(id))
    );

    scheduler.clear();
    assert!(scheduler.is_empty());
}

#[test]
fn zero_area_target_is_treated_as_offscreen() {
    let mut scheduler = AnimationScheduler::new(viewport(0.0, 100.0), ReducedMotion::Allow);
    let id = scheduler.register(
        AnimationSpec::from_millis(&[100]).expect("valid frame"),
        Rect::new(0.0, 0.0, 0.0, 64.0),
    );

    assert_eq!(scheduler.state(id), Some(PlaybackState::PausedOffscreen));
    scheduler.tick(Duration::from_secs(1));
    assert_eq!(scheduler.frame(id), Some(0));
}

#[test]
fn animation_id_displays_its_numeric_value() {
    let mut scheduler = active_scheduler();
    let id: AnimationId = scheduler.register(
        AnimationSpec::from_millis(&[100]).expect("valid frame"),
        frame_rect(0.0),
    );
    assert_eq!(id.to_string(), id.get().to_string());
}
