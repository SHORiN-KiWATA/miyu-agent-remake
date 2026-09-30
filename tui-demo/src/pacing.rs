//! 什么时候画下一帧（蓝图 `tui.md`「每一帧」）：核心推来的字、做好的图、鼠标移动攒到下一拍一起画（`frame_ms`）；
//! 按键、粘贴不等这一拍，只再等一小会儿（`key_burst_ms`）把一串一起到的收齐就画。

use std::time::{Duration, Instant};

use ratatui::crossterm::event::{Event, KeyEventKind};

/// 这一条终端事件要不要当场画：按下、按住的键和粘贴要；松开的键、鼠标、改窗口大小不要。
pub fn urgent(event: &Event) -> bool {
    match event {
        Event::Key(key) => key.kind != KeyEventKind::Release,
        Event::Paste(_) => true,
        _ => false,
    }
}

/// 画下一帧的时刻：上一帧画完的时刻加一拍；这一批里有按键的，最晚在 `now` 加 `burst`。
pub fn deadline(
    drawn_at: Instant,
    beat: Duration,
    now: Instant,
    burst: Duration,
    urgent: bool,
) -> Instant {
    let paced = drawn_at + beat;
    if urgent {
        paced.min(now + burst)
    } else {
        paced
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use ratatui::crossterm::event::{
        Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind,
    };

    use super::{deadline, urgent};

    #[test]
    fn keys_and_pastes_are_drawn_at_once_but_mouse_moves_wait_for_the_beat() {
        // 2026-10-01 性能体检：她在回答时打字，字要等下一拍（p50 约 20 毫秒）。
        let press = Event::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        assert!(urgent(&press));
        let mut release = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        release.kind = KeyEventKind::Release;
        assert!(!urgent(&Event::Key(release)));
        assert!(urgent(&Event::Paste("x".into())));
        let moved = Event::Mouse(MouseEvent {
            kind: MouseEventKind::Moved,
            column: 1,
            row: 1,
            modifiers: KeyModifiers::NONE,
        });
        assert!(!urgent(&moved));
        assert!(!urgent(&Event::Resize(80, 24)));
    }

    #[test]
    fn a_key_draws_after_the_short_burst_not_the_whole_beat() {
        let drawn = Instant::now();
        let beat = Duration::from_millis(33);
        let burst = Duration::from_millis(2);
        let now = drawn + Duration::from_millis(1);
        assert_eq!(
            deadline(drawn, beat, now, burst, false),
            drawn + beat,
            "推来的字等满一拍"
        );
        assert_eq!(
            deadline(drawn, beat, now, burst, true),
            now + burst,
            "按了键：收齐一串就画"
        );
        let late = drawn + Duration::from_millis(40);
        assert_eq!(
            deadline(drawn, beat, late, burst, true),
            drawn + beat,
            "一拍已经过了：照拍子马上画"
        );
    }
}
