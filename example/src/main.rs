extern crate sdl3;

use rand::Rng;
use rand_distr::{Distribution, Normal};
use sdl3::event::Event;
use sdl3::keyboard::Keycode;
use sdl3::pixels::Color;
use sdl3::rect::Rect;
use std::{
    sync::LazyLock,
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct Position {
    x: f64,
    y: f64,
    remote_time: u64,
}

impl snapshot::Snapshot for Position {
    fn interpolate(t: f64, from: &Self, to: &Self) -> Self {
        Position {
            x: snapshot::lerp(from.x, to.x, t),
            y: snapshot::lerp(from.y, to.y, t),
            remote_time: 0,
        }
    }

    fn remote_time(&self) -> f64 {
        self.remote_time as f64
    }
}

pub static SETTINGS: LazyLock<snapshot::Settings> = LazyLock::new(|| snapshot::Settings {
    // playback_clamp_periods: 3.0,
    // playback_fast_speed: 1.0 + 0.1,
    // playback_slow_speed: 1.0 - 0.1,
    // dynamic_playback_time: false,
    // playback_offset_periods: 0.2,
    ..Default::default()
});

pub fn main() {
    let mut buf = snapshot::Buffer::<Position>::new(&*SETTINGS);
    let mut play = snapshot::Playback::new(&buf);

    let sdl_context = sdl3::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    let window = video_subsystem
        .window("rust-sdl3 demo", 800, 600)
        .position_centered()
        .build()
        .unwrap();

    let mut canvas = window.into_canvas();

    canvas.set_draw_color(Color::RGB(0, 255, 255));
    canvas.clear();
    canvas.present();

    let mut event_pump = sdl_context.event_pump().unwrap();

    const CUBE_SIZE: i32 = 50;
    const CUBE_SPEED: i32 = 3;

    let mut green_rect = Rect::new(0, 0, CUBE_SIZE as u32, CUBE_SIZE as u32);
    let mut red_rect = Rect::new(
        (800 - CUBE_SIZE) / 2,
        (600 - CUBE_SIZE) / 2,
        CUBE_SIZE as u32,
        CUBE_SIZE as u32,
    );
    let mut dx: i32 = 0;
    let mut dy: i32 = 0;

    let start = Instant::now();
    let mut last_snapshot_send = Instant::now();
    let mut delta_time = Instant::now();

    let mut pipeline_snapshots = Vec::new();

    let mut rng = rand::rng();
    let mut none_ping = Normal::new(0.0, 0.0).unwrap();
    let mut best_ping = Normal::new(80.0, 0.0).unwrap();
    let mut good_ping = Normal::new(80.0, 40.0).unwrap();
    let mut bad_ping = Normal::new(200.0, 170.0).unwrap();

    'running: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                Event::KeyDown {
                    keycode: Some(keycode),
                    ..
                } => match keycode {
                    Keycode::Left => {
                        dx = -CUBE_SPEED;
                        // dy = 0;
                    }
                    Keycode::Right => {
                        dx = CUBE_SPEED;
                        // dy = 0;
                    }
                    Keycode::Up => {
                        dy = -CUBE_SPEED;
                        // dx = 0;
                    }
                    Keycode::Down => {
                        dy = CUBE_SPEED;
                        // dx = 0;
                    }
                    _ => {}
                },

                _ => {}
            }
        }

        // Update cube position
        red_rect.x += dx;
        red_rect.y += dy;

        if let Some(pos) = play.step(delta_time.elapsed().as_millis() as f64, &buf) {
            green_rect.x = pos.x.round() as i32;
            green_rect.y = pos.y.round() as i32;
        }
        delta_time = Instant::now();

        // Implement bouncing behavior for the red cube
        if red_rect.x < 0 {
            red_rect.x = 0;
            dx = -dx;
        } else if red_rect.x > 800 - CUBE_SIZE {
            red_rect.x = 800 - CUBE_SIZE;
            dx = -dx;
        }

        if red_rect.y < 0 {
            red_rect.y = 0;
            dy = -dy;
        } else if red_rect.y > 600 - CUBE_SIZE {
            red_rect.y = 600 - CUBE_SIZE;
            dy = -dy;
        }

        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();

        canvas.set_draw_color(Color::RGB(0, 255, 0));
        canvas.fill_rect(green_rect).unwrap();

        canvas.set_draw_color(Color::RGB(255, 0, 0));
        canvas.fill_rect(red_rect).unwrap();

        canvas.present();

        for _ in 0..4 {
            // Send a snapshot of the cube's position
            if last_snapshot_send.elapsed().as_millis() >= SETTINGS.period as u128 {
                last_snapshot_send = Instant::now();
                // Simulate the remote sending a snapshot
                pipeline_snapshots.push((
                    Position {
                        x: red_rect.x as f64,
                        y: red_rect.y as f64,
                        remote_time: start.elapsed().as_millis() as u64,
                    },
                    Instant::now(),
                    Duration::from_millis(bad_ping.sample(&mut rng) as u64),
                ));
            }

            pipeline_snapshots.retain(|snapshot| {
                if snapshot.1.elapsed() >= snapshot.2 {
                    // Simulate the local client receiving this snapshot

                    if rng.random_bool(0.95) {
                        //drop rate
                        buf.insert_snapshot(snapshot.0.clone());
                    }

                    return false;
                }

                true
            });

            ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 500));
        }

        println!(
            "db_extrapolating {} (0 is healthy)",
            play.db_extrapolating_ema.value.unwrap_or_default()
        );
        println!(
            "db_clamping      {} (0 is healthy)",
            play.db_clamping_ema.value.unwrap_or_default()
        );
        println!(
            "db_scaling       {} (0 is healthy)",
            play.db_scaling_ema.value.unwrap_or_default()
        );
        println!(
            "catchup time  ({} <=) {} (<= {}) - targets 0 ms (time scaling: {} - {})",
            -SETTINGS.playback_clamp().round(),
            play.catchup_time.value.unwrap_or_default().round(),
            SETTINGS.playback_clamp().round(),
            SETTINGS.slow_threshold().round(),
            SETTINGS.fast_threshold().round(),
        );
        println!("playback time   {}", buf.dynamic_playback_offset().round());
        println!(
            "remote delt  {} - targets 200ms + latency (+ frame time)",
            buf.remote_delta_time.value.unwrap_or_default().round()
        );
        println!("timescale     {}", play.timescale,);

        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 120));
    }
}
