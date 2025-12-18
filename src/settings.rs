use std::sync::LazyLock;

#[derive(Clone)]
pub struct Settings {
    /// The number of seconds worth of packets to store in the
    /// buffer
    pub buf_duration: f32,

    /// The time period (seconds) between the server sending
    /// any two snapshots.
    pub period: f64,

    /// Dynamically adjust the playback offset to adjust for measured
    /// network jitter. i.e. When the network becomes jittery slow the
    /// playback such that jitter is more easily accounted for. When
    /// the network jitter eases, speed back up.
    pub dynamic_playback_time: bool,

    /// When dynamic playback is enabled, this configures the window
    /// in seconds of packets from which to measure network jitter.
    /// i.e. When set to 2, the network jitter is calculated from the
    /// last 2 seconds of received packets.
    pub dynamic_playback_jitter_duration: f32,

    /// How far behind should the playback be? In multiples of the period
    pub playback_offset_periods: f32,

    /// Clamp the playback time this many periods about the
    /// target time
    pub playback_clamp_periods: f32,

    /// Begin slowing the playback when the playback time is
    /// this many periods ahead of the target time (positive)
    pub playback_slow_periods: f32,
    pub playback_slow_speed: f32,

    /// Begin hastening the playback when the playback time is
    /// this many periods behind of the target time (negative)
    pub playback_fast_periods: f32,
    pub playback_fast_speed: f32,
}

pub static SNAPSHOT_SETTINGS_DEFAULT: LazyLock<Settings> = LazyLock::new(|| Settings::default());

impl Default for Settings {
    fn default() -> Self {
        Settings {
            buf_duration: 2.0,
            period: 200.0 / 1000.0, // T = 200ms

            dynamic_playback_time: true,
            dynamic_playback_jitter_duration: 2.0,

            playback_clamp_periods: 1.0,
            playback_fast_periods: 0.5,
            playback_fast_speed: 1.0 + 0.02,
            playback_slow_periods: -0.5,
            playback_slow_speed: 1.0 - 0.04,

            playback_offset_periods: 1.0,
        }
    }
}

impl Settings {
    pub fn playback_offset(&self) -> f32 {
        self.period as f32 * self.playback_offset_periods
    }

    pub fn playback_clamp(&self) -> f32 {
        self.period as f32 * self.playback_clamp_periods
    }

    pub fn fast_threshold(&self) -> f32 {
        self.period as f32 * self.playback_fast_periods
    }

    pub fn slow_threshold(&self) -> f32 {
        self.period as f32 * self.playback_slow_periods
    }

    /// Packets per Second (dispatched by the remote)
    pub fn send_rate(&self) -> f64 {
        1.0 / (self.period as f64)
    }
}
