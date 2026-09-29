//! 焦点往下走（蓝图 `tui.md`「后台命令、子代理和侧边栏」第 2、6 条）：输入框 → 框下面那一行的按钮
//! （有几个时 `←` `→` 在它们之间换，现在只有后台按钮）→ 子代理状态行；`↑` 往回走。按钮、行没了就退回。

/// 焦点在哪。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Focus {
    /// 输入框。
    #[default]
    Input,
    /// 框下面那一行的一个按钮。
    Footer(Button),
    /// 子代理状态行的第几个子代理。
    Agent(usize),
}

/// 框下面那一行的按钮。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    /// 「N 个后台命令」。
    Background,
}

/// 这一刻能去的地方：框下面那一行的按钮（从左到右），子代理有几行。
#[derive(Debug, Clone, Copy)]
pub struct Stops<'a> {
    /// 按钮，从左到右。
    pub buttons: &'a [Button],
    /// 子代理状态行有几个子代理。
    pub agents: usize,
}

impl Focus {
    /// 按 `↓`：输入框到第一个按钮（没有按钮到第一个子代理），按钮到第一个子代理，子代理往下一个。
    pub fn down(self, stops: Stops) -> Self {
        let first_agent = (stops.agents > 0).then_some(Self::Agent(0));
        match self {
            Self::Input => stops
                .buttons
                .first()
                .map(|b| Self::Footer(*b))
                .or(first_agent)
                .unwrap_or(self),
            Self::Footer(_) => first_agent.unwrap_or(self),
            Self::Agent(i) => Self::Agent((i + 1).min(stops.agents.saturating_sub(1))),
        }
    }

    /// 按 `↑`：子代理往上一个，第一个子代理回到第一个按钮（没有按钮回输入框），按钮回输入框。
    pub fn up(self, stops: Stops) -> Self {
        match self {
            Self::Agent(i) if i > 0 => Self::Agent(i - 1),
            Self::Agent(_) => stops
                .buttons
                .first()
                .map_or(Self::Input, |b| Self::Footer(*b)),
            _ => Self::Input,
        }
    }

    /// 按 `←`、`→`：在按钮之间换，到头就停。
    pub fn side(self, stops: Stops, right: bool) -> Self {
        let Self::Footer(b) = self else {
            return self;
        };
        let at = stops.buttons.iter().position(|x| *x == b).unwrap_or(0);
        let next = if right {
            (at + 1).min(stops.buttons.len().saturating_sub(1))
        } else {
            at.saturating_sub(1)
        };
        stops.buttons.get(next).map_or(self, |b| Self::Footer(*b))
    }

    /// 按钮、行没了：退到还在的地方。
    pub fn settle(self, stops: Stops) -> Self {
        match self {
            Self::Footer(b) if !stops.buttons.contains(&b) => stops
                .buttons
                .first()
                .map_or(Self::Input, |b| Self::Footer(*b)),
            Self::Agent(_) if stops.agents == 0 => Self::Input,
            Self::Agent(i) => Self::Agent(i.min(stops.agents - 1)),
            _ => self,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Button, Focus, Stops};

    #[test]
    fn down_goes_input_then_the_button_then_agents_and_up_comes_back() {
        let bg = [Button::Background];
        let s = Stops {
            buttons: &bg,
            agents: 2,
        };
        let f = Focus::Input.down(s);
        assert_eq!(f, Focus::Footer(Button::Background));
        assert_eq!(f.side(s, true), f, "只有一个按钮：← → 不动");
        assert_eq!(f.side(s, false), f);
        let a = f.down(s);
        assert_eq!(a, Focus::Agent(0));
        assert_eq!(a.down(s).down(s), Focus::Agent(1), "到最后一个就停");
        assert_eq!(Focus::Agent(1).up(s), Focus::Agent(0));
        assert_eq!(a.up(s), Focus::Footer(Button::Background));
        assert_eq!(f.up(s), Focus::Input);
    }

    #[test]
    fn with_no_buttons_down_goes_straight_to_the_agents() {
        let s = Stops {
            buttons: &[],
            agents: 1,
        };
        assert_eq!(Focus::Input.down(s), Focus::Agent(0));
        assert_eq!(Focus::Agent(0).up(s), Focus::Input);
        let none = Stops {
            buttons: &[],
            agents: 0,
        };
        assert_eq!(
            Focus::Input.down(none),
            Focus::Input,
            "什么都没有：留在输入框"
        );
    }

    #[test]
    fn a_gone_button_or_row_falls_back() {
        let none = Stops {
            buttons: &[],
            agents: 0,
        };
        assert_eq!(
            Focus::Footer(Button::Background).settle(none),
            Focus::Input,
            "按钮没了"
        );
        assert_eq!(Focus::Agent(3).settle(none), Focus::Input);
        let two = Stops {
            buttons: &[],
            agents: 2,
        };
        assert_eq!(Focus::Agent(5).settle(two), Focus::Agent(1));
    }
}
