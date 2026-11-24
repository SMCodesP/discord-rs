use std::collections::BTreeMap;
use std::fmt;
use std::time::Duration;

/// Rectangle in content coordinates. The scheduler compares animation bounds
/// against the visible viewport window to decide whether frames advance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.width > 0.0
            && self.height > 0.0
            && other.width > 0.0
            && other.height > 0.0
            && self.x < other.x + other.width
            && other.x < self.x + self.width
            && self.y < other.y + other.height
            && other.y < self.y + self.height
    }
}

/// How animated media must behave under the accessibility reduced-motion flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReducedMotion {
    /// Frames advance while the animation is inside the viewport.
    Allow,
    /// Animation is held on its first frame and never advances.
    Pause,
}

impl ReducedMotion {
    pub const fn from_reduce_motion(reduce_motion: bool) -> Self {
        if reduce_motion {
            Self::Pause
        } else {
            Self::Allow
        }
    }

    pub const fn is_paused(self) -> bool {
        matches!(self, Self::Pause)
    }
}

/// Current playback state of a registered animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    /// Frames advance normally.
    Playing,
    /// Outside the viewport window; playback frozen at the current frame.
    PausedOffscreen,
    /// Accessibility reduced motion is active; held on the first frame.
    PausedReducedMotion,
    /// All configured loop repetitions completed; last frame is held.
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AnimationId(u64);

impl AnimationId {
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for AnimationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A frame change or playback-state transition emitted by `tick`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameUpdate {
    pub id: AnimationId,
    pub frame: usize,
    pub state: PlaybackState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchedulerError {
    NoFrames,
    ZeroFrameDuration { index: usize },
    UnknownAnimation(AnimationId),
}

impl fmt::Display for SchedulerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoFrames => write!(f, "animation has no frames"),
            Self::ZeroFrameDuration { index } => {
                write!(f, "animation frame {index} has a zero duration")
            }
            Self::UnknownAnimation(id) => write!(f, "unknown animation {id}"),
        }
    }
}

impl std::error::Error for SchedulerError {}

/// Validated frame timing for an animated asset (GIF, APNG, animated avatar,
/// emoji or sticker). `loop_count == 0` means repeat forever, matching the
/// APNG `num_plays` and GIF Netscape extension convention.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimationSpec {
    frame_durations: Vec<Duration>,
    loop_count: u32,
}

impl AnimationSpec {
    pub fn new(frame_durations: Vec<Duration>) -> Result<Self, SchedulerError> {
        if frame_durations.is_empty() {
            return Err(SchedulerError::NoFrames);
        }
        for (index, duration) in frame_durations.iter().enumerate() {
            if duration.is_zero() {
                return Err(SchedulerError::ZeroFrameDuration { index });
            }
        }
        Ok(Self {
            frame_durations,
            loop_count: 0,
        })
    }

    pub fn from_millis(frame_delays_ms: &[u64]) -> Result<Self, SchedulerError> {
        Self::new(
            frame_delays_ms
                .iter()
                .map(|ms| Duration::from_millis(*ms))
                .collect(),
        )
    }

    pub fn with_loop_count(mut self, loop_count: u32) -> Self {
        self.loop_count = loop_count;
        self
    }

    pub fn frame_count(&self) -> usize {
        self.frame_durations.len()
    }

    pub fn frame_duration(&self, index: usize) -> Option<Duration> {
        self.frame_durations.get(index).copied()
    }

    pub fn total_duration(&self) -> Duration {
        self.frame_durations.iter().copied().sum()
    }

    pub fn loop_count(&self) -> u32 {
        self.loop_count
    }

    pub const fn is_looping(&self) -> bool {
        self.loop_count == 0
    }
}

struct Entry {
    spec: AnimationSpec,
    rect: Rect,
    frame_index: usize,
    frame_elapsed: Duration,
    loops_completed: u32,
    finished: bool,
    last_reported: Option<(usize, PlaybackState)>,
}

impl Entry {
    fn new(spec: AnimationSpec, rect: Rect) -> Self {
        Self {
            spec,
            rect,
            frame_index: 0,
            frame_elapsed: Duration::ZERO,
            loops_completed: 0,
            finished: false,
            last_reported: None,
        }
    }

    fn reset_playback(&mut self) {
        self.frame_index = 0;
        self.frame_elapsed = Duration::ZERO;
        self.loops_completed = 0;
        self.finished = false;
        self.last_reported = None;
    }

    fn effective_state(&self, viewport: &Rect, reduced: ReducedMotion) -> PlaybackState {
        if reduced.is_paused() {
            PlaybackState::PausedReducedMotion
        } else if !self.rect.intersects(viewport) {
            PlaybackState::PausedOffscreen
        } else if self.finished {
            PlaybackState::Finished
        } else {
            PlaybackState::Playing
        }
    }

    fn advance(&mut self, delta: Duration) {
        if self.finished || delta.is_zero() {
            return;
        }

        let count = self.spec.frame_durations.len();
        let mut remaining = delta;

        while remaining > Duration::ZERO && !self.finished {
            let frame_duration = self.spec.frame_durations[self.frame_index];
            let left = frame_duration.saturating_sub(self.frame_elapsed);

            if remaining < left {
                self.frame_elapsed += remaining;
                remaining = Duration::ZERO;
            } else {
                remaining = remaining.saturating_sub(left);
                self.frame_elapsed = Duration::ZERO;

                if self.frame_index + 1 < count {
                    self.frame_index += 1;
                } else {
                    self.loops_completed += 1;
                    if self.spec.loop_count != 0
                        && self.loops_completed >= self.spec.loop_count
                    {
                        self.finished = true;
                    } else {
                        self.frame_index = 0;
                    }
                }
            }
        }
    }
}

/// Schedules frame advancement for animated media.
///
/// Animations receive content-space bounds and only advance while those bounds
/// intersect the current viewport window. Offscreen animations freeze in place
/// and resume from the same frame when they become visible again. When reduced
/// motion is active every animation is held on its first frame.
pub struct AnimationScheduler {
    viewport: Rect,
    reduced_motion: ReducedMotion,
    entries: BTreeMap<AnimationId, Entry>,
    next_id: u64,
}

impl AnimationScheduler {
    pub fn new(viewport: Rect, reduced_motion: ReducedMotion) -> Self {
        Self {
            viewport,
            reduced_motion,
            entries: BTreeMap::new(),
            next_id: 0,
        }
    }

    pub fn viewport(&self) -> Rect {
        self.viewport
    }

    pub fn reduced_motion(&self) -> ReducedMotion {
        self.reduced_motion
    }

    pub fn set_viewport(&mut self, viewport: Rect) {
        self.viewport = viewport;
    }

    pub fn set_reduced_motion(&mut self, reduced_motion: ReducedMotion) {
        if self.reduced_motion == reduced_motion {
            return;
        }
        self.reduced_motion = reduced_motion;
        if reduced_motion.is_paused() {
            for entry in self.entries.values_mut() {
                entry.reset_playback();
            }
        }
    }

    pub fn register(&mut self, spec: AnimationSpec, rect: Rect) -> AnimationId {
        let id = AnimationId(self.next_id);
        self.next_id += 1;
        self.entries.insert(id, Entry::new(spec, rect));
        id
    }

    pub fn unregister(&mut self, id: AnimationId) -> bool {
        self.entries.remove(&id).is_some()
    }

    pub fn update_rect(&mut self, id: AnimationId, rect: Rect) -> Result<(), SchedulerError> {
        let entry = self
            .entries
            .get_mut(&id)
            .ok_or(SchedulerError::UnknownAnimation(id))?;
        entry.rect = rect;
        Ok(())
    }

    pub fn frame(&self, id: AnimationId) -> Option<usize> {
        self.entries.get(&id).map(|entry| entry.frame_index)
    }

    pub fn state(&self, id: AnimationId) -> Option<PlaybackState> {
        self.entries
            .get(&id)
            .map(|entry| entry.effective_state(&self.viewport, self.reduced_motion))
    }

    pub fn spec(&self, id: AnimationId) -> Option<&AnimationSpec> {
        self.entries.get(&id).map(|entry| &entry.spec)
    }

    pub fn ids(&self) -> impl Iterator<Item = AnimationId> + '_ {
        self.entries.keys().copied()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn has_active_animations(&self) -> bool {
        self.entries.values().any(|entry| {
            entry.effective_state(&self.viewport, self.reduced_motion) == PlaybackState::Playing
        })
    }

    pub fn tick(&mut self, delta: Duration) -> Vec<FrameUpdate> {
        let viewport = self.viewport;
        let reduced = self.reduced_motion;
        let mut updates = Vec::new();

        for (id, entry) in self.entries.iter_mut() {
            if entry.effective_state(&viewport, reduced) == PlaybackState::Playing {
                entry.advance(delta);
            }

            let state = entry.effective_state(&viewport, reduced);
            let report = (entry.frame_index, state);
            if entry.last_reported != Some(report) {
                entry.last_reported = Some(report);
                updates.push(FrameUpdate {
                    id: *id,
                    frame: entry.frame_index,
                    state,
                });
            }
        }

        updates
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn visible_scheduler() -> AnimationScheduler {
        AnimationScheduler::new(Rect::new(0.0, 0.0, 300.0, 300.0), ReducedMotion::Allow)
    }

    #[test]
    fn spec_rejects_empty_and_zero_frames() {
        assert_eq!(AnimationSpec::new(Vec::new()), Err(SchedulerError::NoFrames));
        assert_eq!(
            AnimationSpec::from_millis(&[50, 0, 20]),
            Err(SchedulerError::ZeroFrameDuration { index: 1 })
        );
    }

    #[test]
    fn rect_requires_positive_area_and_overlap() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert!(a.intersects(&Rect::new(9.0, 9.0, 5.0, 5.0)));
        assert!(!a.intersects(&Rect::new(10.0, 0.0, 5.0, 5.0)));
        assert!(!a.intersects(&Rect::new(0.0, 0.0, 0.0, 10.0)));
    }

    #[test]
    fn advances_and_wraps_using_frame_durations() {
        let mut scheduler = visible_scheduler();
        let spec = AnimationSpec::from_millis(&[100, 100, 100]).unwrap();
        let id = scheduler.register(spec, Rect::new(0.0, 0.0, 50.0, 50.0));

        assert_eq!(scheduler.frame(id), Some(0));
        scheduler.tick(Duration::from_millis(99));
        assert_eq!(scheduler.frame(id), Some(0));
        scheduler.tick(Duration::from_millis(1));
        assert_eq!(scheduler.frame(id), Some(1));
        scheduler.tick(Duration::from_millis(200));
        assert_eq!(scheduler.frame(id), Some(0));
        assert_eq!(scheduler.state(id), Some(PlaybackState::Playing));
    }
}
