use crate::core::state::{AssistantState, StateManager};
use crate::speech::{play_chime, ChimeType, PiperTts};
use chrono::{Local, Timelike};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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
        let total_secs = (minutes * 60.0).round().max(1.0) as u64;
        let mut id_lock = self.next_id.lock().unwrap();
        let id = *id_lock;
        *id_lock += 1;

        let item = TimerItem {
            id,
            label: if label.is_empty() {
                "Hẹn giờ".to_string()
            } else {
                label.to_string()
            },
            total_seconds: total_secs,
            start_instant: Some(Instant::now()),
        };

        let mut timers = self.timers.lock().unwrap();
        timers.push(item);

        let mins = total_secs / 60;
        let secs = total_secs % 60;
        let display_time = match (mins, secs) {
            (0, s) => format!("{} giây", s),
            (m, 0) => format!("{} phút", m),
            (m, s) => format!("{} phút {} giây", m, s),
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

        let mut diff_min =
            (hour as i32 - current_h as i32) * 60 + (minute as i32 - current_m as i32);
        if diff_min <= 0 {
            diff_min += 24 * 60; // Ngày hôm sau
        }

        let default_label = format!("Báo thức {} giờ {} phút", hour, minute);
        let lbl = if label.is_empty() {
            &default_label
        } else {
            label
        };

        self.add_timer(diff_min as f64, lbl);

        format!("Đã cài báo thức lúc {} giờ {} phút cho bạn.", hour, minute)
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

    /// Lấy và xóa toàn bộ các bộ hẹn giờ đã hết thời gian
    pub fn pop_expired(&self) -> Vec<TimerItem> {
        let mut timers = self.timers.lock().unwrap();
        let mut expired = Vec::new();
        let mut active = Vec::with_capacity(timers.len());

        for item in timers.drain(..) {
            let is_done = item
                .start_instant
                .map(|s| s.elapsed().as_secs() >= item.total_seconds)
                .unwrap_or(false);
            if is_done {
                expired.push(item);
            } else {
                active.push(item);
            }
        }

        *timers = active;
        expired
    }

    /// Tác vụ nền kiểm tra mỗi giây để tự động đổ chuông và đọc thông báo khi hết giờ
    pub fn spawn_watcher(&'static self, tts: Arc<PiperTts>, state_mgr: StateManager) {
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(1));
            loop {
                ticker.tick().await;
                let expired = self.pop_expired();
                for item in expired {
                    let msg = format!(
                        "Hết giờ rồi bạn ơi! '{}' đã hoàn thành.",
                        item.label
                    );
                    println!("⏰ [Timer]: {}", msg);

                    state_mgr
                        .set_state_full(
                            AssistantState::Speaking,
                            Some(msg.clone()),
                            Some(item.label.clone()),
                        )
                        .await;

                    let tts_clone = tts.clone();
                    let mgr_clone = state_mgr.clone();
                    let msg_clone = msg.clone();
                    let _ = tokio::task::spawn_blocking(move || {
                        play_chime(ChimeType::Success);
                        std::thread::sleep(Duration::from_millis(120));
                        play_chime(ChimeType::Success);
                        if tts_clone.is_available() {
                            tts_clone.speak(&msg_clone, Some(&mgr_clone));
                        }
                    })
                    .await;

                    state_mgr.set_state(AssistantState::Idle, None).await;
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timer_pop_expired() {
        let mgr = TimerManager::new();
        let msg = mgr.add_timer(1.0 / 60.0, "Luộc trứng");
        assert!(msg.contains("1 giây"));
        assert!(mgr.pop_expired().is_empty());

        std::thread::sleep(Duration::from_millis(1050));
        let done = mgr.pop_expired();
        assert_eq!(done.len(), 1);
        assert_eq!(done[0].label, "Luộc trứng");
        assert!(mgr.pop_expired().is_empty());
    }
}
