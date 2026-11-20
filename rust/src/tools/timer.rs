use chrono::{Local, Timelike};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimerItem {
    pub id: usize,
    pub label: String,
    pub total_seconds: u64,
    #[serde(skip)]
    pub start_instant: Option<Instant>,
}

#[derive(Clone, Default)]
pub struct TimerManager {
    timers: Arc<Mutex<Vec<TimerItem>>>,
    next_id: Arc<Mutex<usize>>,
}

impl TimerManager {
    pub fn new() -> Self {
        Self {
            timers: Arc::new(Mutex::new(Vec::new())),
            next_id: Arc::new(Mutex::new(1)),
        }
    }

    pub fn add_timer(&self, minutes: f64, label: &str) -> String {
        let total_secs = (minutes * 60.0).round() as u64;
        let mut id_lock = self.next_id.lock().unwrap();
        let id = *id_lock;
        *id_lock += 1;

        let item = TimerItem {
            id,
            label: if label.is_empty() { "Hẹn giờ".to_string() } else { label.to_string() },
            total_seconds: total_secs,
            start_instant: Some(Instant::now()),
        };

        let mut timers = self.timers.lock().unwrap();
        timers.push(item);

        let display_time = if minutes >= 1.0 {
            format!("{:.0} phút", minutes)
        } else {
            format!("{} giây", total_secs)
        };

        if label.is_empty() {
            format!("Đã đặt hẹn giờ {} cho bạn.", display_time)
        } else {
            format!("Đã đặt hẹn giờ {} cho '{}'.", display_time, label)
        }
    }

    pub fn add_alarm(&self, hour: u32, minute: u32, label: &str) -> String {
        let now = Local::now();
        let current_h = now.hour();
        let current_m = now.minute();

        let mut diff_min = (hour as i32 - current_h as i32) * 60 + (minute as i32 - current_m as i32);
        if diff_min <= 0 {
            diff_min += 24 * 60; // Next day
        }

        let default_label = format!("Báo thức {} giờ {} phút", hour, minute);
        let lbl = if label.is_empty() {
            &default_label
        } else {
            label
        };

        self.add_timer(diff_min as f64, lbl);

        format!(
            "Đã cài báo thức lúc {} giờ {} phút cho bạn.",
            hour, minute
        )
    }

    pub fn cancel_all(&self) -> String {
        let mut timers = self.timers.lock().unwrap();
        let count = timers.len();
        timers.clear();
        if count == 0 {
            "Hiện tại bạn không có hẹn giờ hoặc báo thức nào đang chạy.".to_string()
        } else {
            format!("Đã hủy toàn bộ {} bộ hẹn giờ và báo thức cho bạn.", count)
        }
    }

    pub fn check_timers(&self) -> String {
        let timers = self.timers.lock().unwrap();
        if timers.is_empty() {
            return "Hiện tại bạn không có hẹn giờ hoặc báo thức nào đang đếm ngược.".to_string();
        }

        let mut reports = Vec::new();
        for t in timers.iter() {
            if let Some(start) = t.start_instant {
                let elapsed = start.elapsed().as_secs();
                if elapsed >= t.total_seconds {
                    reports.push(format!("'{}' đã hết thời gian!", t.label));
                } else {
                    let remain = t.total_seconds - elapsed;
                    let m = remain / 60;
                    let s = remain % 60;
                    reports.push(format!("'{}' còn {} phút {} giây", t.label, m, s));
                }
            }
        }

        reports.join("; ")
    }
}
