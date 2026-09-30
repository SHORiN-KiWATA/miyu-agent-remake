//! 终端界面的演示程序：全屏，底下一个圆角输入框，连上重制版的核心跟她说话。
//!
//! 设计见仓库的 `docs/designs/13-终端界面.md`；连核心的走法照 `miyu ask`（`docs/blueprint/cli/ask.md`）。

mod app;
mod body_view;
mod caret;
mod clipboard;
mod commands;
mod config;
mod core;
mod crash;
mod diff;
mod drawer;
mod editor;
mod figures;
mod focus;
mod history;
mod input;
mod jobs;
mod language;
mod linebreak;
mod local;
mod markdown;
mod mascot;
mod mention;
mod menu;
mod meter;
mod notify;
mod open;
mod pointer;
mod pulse;
mod reader;
mod rng;
mod side_select;
mod theme;
mod tips;
mod transcript;
mod ui;

use std::io::{self, Write, stdout};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{
    DisableBracketedPaste, DisableFocusChange, DisableMouseCapture, EnableBracketedPaste,
    EnableFocusChange, EnableMouseCapture, Event, KeyboardEnhancementFlags,
    PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use ratatui::crossterm::{cursor, execute, terminal};

use miyu_store::env::Env;

use app::App;
use config::Config;
use core::Update;

fn main() -> io::Result<()> {
    // 配置先读：读不懂就别进全屏，错误照原样打在终端里。
    // 界面语言照系统语言（蓝图「界面语言」）。
    let table = language::LanguageTable::builtin().map_err(io::Error::other)?;
    let language = table.detect(|name| std::env::var(name).ok());
    let config = Config::load(&language).map_err(io::Error::other)?;
    let mut terminal = ratatui::init();
    let keyboard = match enter() {
        Ok(keyboard) => keyboard,
        Err(e) => {
            ratatui::restore();
            return Err(e);
        }
    };
    // ratatui::init 装的崩溃处理只收拾原始模式和备用屏；鼠标、粘贴、键盘协议也要收，
    // 不然崩了以后终端里一动鼠标就是一串乱码。
    let previous = std::panic::take_hook();
    let saved = config.text.crash_saved.clone();
    std::panic::set_hook(Box::new(move |info| {
        // 已经在崩了，收拾失败也只能说一声，接着把崩溃信息交给原来的处理。
        if let Err(e) = leave(keyboard) {
            eprintln!("终端没收拾干净：{e}");
        }
        previous(info);
        // 调用栈记进文件，下一次复现不出来的也留得下位置（蓝图「崩了」）。
        if let Some(path) = crash::record(info) {
            eprintln!("{}", saved.replace("{path}", &path.display().to_string()));
        }
    }));
    // 问终端能不能显示图：进了全屏、还没开始读按键的时候问（蓝图「图片、公式和 mermaid 图」第 1 条）。
    let graphics = figures::terminal::probe();
    let result = run(&mut terminal, config, graphics, keyboard);
    leave(keyboard)?;
    ratatui::restore();
    result
}

/// 打开鼠标、括号粘贴；终端认得 kitty 键盘协议就打开，Shift+Enter 才分得出来。
/// 返回有没有打开键盘协议，退出时照着关。光标的样子不碰，跟终端自己的设置（`tui.md`「输入框」第 4 条）。
fn enter() -> io::Result<bool> {
    let keyboard = terminal::supports_keyboard_enhancement().unwrap_or(false);
    modes_on(keyboard)?;
    Ok(keyboard)
}

/// 打开鼠标、括号粘贴，`keyboard` 为真时再打开 kitty 键盘协议。
fn modes_on(keyboard: bool) -> io::Result<()> {
    // 焦点上报：终端报在不在前台，系统通知照它（蓝图「系统通知」第 2 条）。
    execute!(
        stdout(),
        EnableMouseCapture,
        EnableBracketedPaste,
        EnableFocusChange
    )?;
    if keyboard {
        execute!(
            stdout(),
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        )?;
    }
    Ok(())
}

fn leave(keyboard: bool) -> io::Result<()> {
    pointer::reset(&mut stdout())?;
    if keyboard {
        execute!(stdout(), PopKeyboardEnhancementFlags)?;
    }
    execute!(
        stdout(),
        DisableMouseCapture,
        DisableBracketedPaste,
        DisableFocusChange
    )
}

/// 主循环等的东西：终端的事件，或者核心的消息。
enum Incoming {
    Terminal(Event),
    Core(Update),
    Figure(figures::Done),
}

/// 主循环：画一帧，等事件；一次把攒着的都处理完再画，空闲时不重画。
///
/// 终端的事件在一个线程里读，核心在另一个线程里连，都送进同一个通道，主循环只等这一个口子。
fn run(
    terminal: &mut ratatui::DefaultTerminal,
    config: Config,
    graphics: Option<figures::Graphics>,
    keyboard: bool,
) -> io::Result<()> {
    let (sender, incoming) = mpsc::channel();
    let to_core = sender.clone();
    let reconnect = config.layout.reconnect_ms;
    let core = core::spawn(reconnect, move |update| {
        to_core.send(Incoming::Core(update)).is_ok()
    });
    let to_main = sender.clone();
    // 点开看的 mermaid 大图放在机器共用的缓存目录下（蓝图「图片、公式和 mermaid 图」第 4 条）。
    let zoom_dir = miyu_store::root::cache_root(&Env::current())
        .ok()
        .map(|root| root.join("tui").join("diagrams"));
    let figures = figures::Figures::start(graphics, &config.figures, zoom_dir, move |done| {
        to_main.send(Incoming::Figure(done)).is_ok()
    });
    // 读按键的线程：让出终端给编辑器时停下（`reader.rs`）。
    let reader = reader::Reader::spawn(move |event| sender.send(Incoming::Terminal(event)).is_ok());
    let human = config.language.human();
    let mut app = App::new(config, core, human, figures);
    let mut pointer = pointer::Pointer::default();
    // 终端显示得了几种颜色，启动时看一次（蓝图「主题」第 5 条）。
    let depth = theme::Depth::detect(|name| std::env::var(name).ok());
    while !app.quit {
        frame(terminal, &mut app, &mut pointer, depth)?;
        let drawn_at = Instant::now();
        let wait = app.deadline().map_or(Duration::from_secs(3600), |d| {
            d.saturating_duration_since(Instant::now())
        });
        let mut next = match incoming.recv_timeout(wait) {
            Ok(message) => Some(message),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        while let Some(message) = next {
            dispatch(&mut app, message);
            next = incoming.try_recv().ok();
        }
        // 离上一帧还不到 `frame_ms`：接着收，到点一起画（蓝图「每一帧」：一段段推来的字每段画一帧，
        // 一秒上百帧，输入法的预编辑跟着光标重画，打字时狂闪）。
        let gap = Duration::from_millis(app.config.layout.frame_ms);
        while let Some(left) = gap.checked_sub(drawn_at.elapsed()).filter(|d| !d.is_zero()) {
            match incoming.recv_timeout(left) {
                Ok(message) => dispatch(&mut app, message),
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return Ok(()),
            }
        }
        app.tick();
        if std::mem::take(&mut app.suspend) {
            suspend(terminal, keyboard)?;
            app.figures.borrow_mut().forget();
            pointer = pointer::Pointer::default();
        }
        // 点了本机的文本文件：让出终端给编辑器，退出了再回来（蓝图「她的回答：Markdown」第 10 条）。
        if let Some((editor, file)) = app.edit.take() {
            let edited = edit(terminal, keyboard, &reader, &editor, &file);
            app.edited(edited);
            pointer = pointer::Pointer::default();
        }
    }
    Ok(())
}

/// 收一条：终端的事件、核心的消息、做好的图。
fn dispatch(app: &mut App, message: Incoming) {
    match message {
        Incoming::Terminal(event) => app.handle(event),
        Incoming::Core(update) => app.core(update),
        Incoming::Figure(done) => app.figures.borrow_mut().done(done),
    }
}

/// Ctrl+Z：还原终端、挂起自己；`fg` 回来以后重新进全屏，下一帧整屏重画（蓝图 `tui.md`「按键」）。
/// 只停自己：核心在自己的进程组里，照常在后台跑。
#[cfg(unix)]
fn suspend(screen: &mut ratatui::DefaultTerminal, keyboard: bool) -> io::Result<()> {
    leave(keyboard)?;
    ratatui::restore();
    rustix::process::kill_process(rustix::process::getpid(), rustix::process::Signal::TSTP)?;
    // 到这里已经 `fg` 回来了。
    reenter(screen, keyboard)
}

/// 回全屏：不能调 ratatui 的 `clear`、不能重新探测键盘协议：它们都要读终端的回话，而读按键的线程一直占着读的锁，
/// 读不到就超时报错。所以自己清屏、换一块新画布（下一帧整屏重画），键盘协议照启动时探到的开回去。
fn reenter(screen: &mut ratatui::DefaultTerminal, keyboard: bool) -> io::Result<()> {
    terminal::enable_raw_mode()?;
    execute!(
        stdout(),
        terminal::EnterAlternateScreen,
        terminal::Clear(terminal::ClearType::All)
    )?;
    modes_on(keyboard)?;
    *screen = ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(stdout()))?;
    Ok(())
}

/// 让出终端给编辑器：关掉鼠标、粘贴、键盘协议和原始模式，读按键的停下；编辑器退出了回全屏（它自己离开了备用屏的
/// 马上进回去）、接着读（蓝图「她的回答：Markdown」第 10 条）。
/// 核心在别的线程，推来的攒在通道里，回来再画。
fn edit(
    screen: &mut ratatui::DefaultTerminal,
    keyboard: bool,
    reader: &reader::Reader,
    editor: &str,
    file: &std::path::Path,
) -> io::Result<()> {
    reader.pause();
    leave(keyboard)?;
    // 不离开备用屏：离开会露出底下 shell 的内容，光标先飞到左下角再飞进编辑器（2026-09-30 项目主人）。这一帧留着，
    // 光标藏起来，等编辑器自己画上来。
    execute!(stdout(), cursor::Hide)?;
    terminal::disable_raw_mode()?;
    let ran = editor::run(editor, file);
    let back = reenter(screen, keyboard);
    reader.resume();
    ran.and(back)
}

#[cfg(not(unix))]
fn suspend(_screen: &mut ratatui::DefaultTerminal, _keyboard: bool) -> io::Result<()> {
    Ok(())
}

/// 画一帧，用同步输出包起来（蓝图 `tui.md`「每一帧」）：终端收齐了再画，传图的那一帧不会闪、光标不乱跳。
/// 不认的终端当没有。画失败了也要把结尾发出去，不然认得的终端会一直等。
/// 顺手照这一帧悬停的是不是链接，换鼠标指针的样子。画完、交给终端之前把宽字后面那格清空，再照色深统一换色
/// （蓝图「每一帧」、「主题」第 6 条）。
fn frame(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    pointer: &mut pointer::Pointer,
    depth: theme::Depth,
) -> io::Result<()> {
    execute!(stdout(), terminal::BeginSynchronizedUpdate)?;
    let drawn = terminal.draw(|frame| {
        ui::draw(frame, app);
        ui::wide::tidy(frame.buffer_mut());
        theme::degrade(frame.buffer_mut(), depth);
    });
    // 光标最后挪：先挪到插入点、要显示时再显示，不显示也停在那里（蓝图「每一帧」）。
    caret::place(app.caret, &mut stdout())?;
    pointer.set(app.pointing(), &mut stdout())?;
    execute!(stdout(), terminal::EndSynchronizedUpdate)?;
    // 系统通知的转义序列（kitty 的 OSC 99、OSC 9）：画完一帧再写，一条一次写完，不被别的输出劈开（「系统通知」第 4 条）。
    let mut out = stdout();
    for sequence in app.notifier.outbox() {
        out.write_all(sequence.as_bytes())?;
    }
    out.flush()?;
    drawn.map(|_| ())
}
