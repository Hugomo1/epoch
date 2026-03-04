use crossterm::event::{KeyEvent, MouseEvent};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{Duration, interval};

use crate::types::{SystemMetrics, TrainingMetrics};

pub const EVENT_CHANNEL_CAPACITY: usize = 64;
pub const METRICS_CHANNEL_CAPACITY: usize = 256;
pub const SYSTEM_CHANNEL_CAPACITY: usize = 64;

#[derive(Debug)]
pub enum Event {
    Tick,
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
    Metrics(TrainingMetrics),
    System(SystemMetrics),
}

pub struct EventHandler {
    rx: mpsc::Receiver<Event>,
    _event_task: JoinHandle<()>,
    _tick_task: JoinHandle<()>,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> Self {
        let (tx, rx) = mpsc::channel(EVENT_CHANNEL_CAPACITY);
        let event_task = spawn_event_reader(tx.clone());
        let tick_task = spawn_tick(tx, tick_rate);

        Self {
            rx,
            _event_task: event_task,
            _tick_task: tick_task,
        }
    }

    pub async fn next(&mut self) -> color_eyre::Result<Event> {
        self.rx
            .recv()
            .await
            .ok_or_else(|| color_eyre::eyre::eyre!("event channel closed"))
    }
}

pub fn spawn_event_reader(tx: mpsc::Sender<Event>) -> JoinHandle<()> {
    tokio::spawn(async move {
        use crossterm::event::EventStream;
        use futures::StreamExt;

        let mut reader = EventStream::new();

        while let Some(Ok(event)) = reader.next().await {
            let mapped = match event {
                crossterm::event::Event::Key(key) => Event::Key(key),
                crossterm::event::Event::Resize(width, height) => Event::Resize(width, height),
                _ => continue,
            };

            if tx.send(mapped).await.is_err() {
                break;
            }
        }
    })
}

pub fn spawn_tick(tx: mpsc::Sender<Event>, tick_rate: Duration) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = interval(tick_rate);

        loop {
            ticker.tick().await;

            if tx.send(Event::Tick).await.is_err() {
                break;
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{
        KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    use tokio::sync::mpsc;
    use tokio::time::{Duration, timeout};

    #[test]
    fn test_event_channel_constants() {
        assert_eq!(EVENT_CHANNEL_CAPACITY, 64);
        assert_eq!(METRICS_CHANNEL_CAPACITY, 256);
        assert_eq!(SYSTEM_CHANNEL_CAPACITY, 64);
    }

    #[tokio::test]
    async fn test_event_tick_received() {
        let (tx, mut rx) = mpsc::channel(EVENT_CHANNEL_CAPACITY);
        let _tick_task = spawn_tick(tx, Duration::from_millis(50));

        let received = timeout(Duration::from_millis(250), rx.recv())
            .await
            .expect("timed out waiting for tick");
        assert!(matches!(received, Some(Event::Tick)));
    }

    #[test]
    fn test_event_enum_variants() {
        let tick = Event::Tick;
        assert!(matches!(tick, Event::Tick));

        let key = Event::Key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
        assert!(matches!(key, Event::Key(_)));

        let mouse = Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 10,
            row: 5,
            modifiers: KeyModifiers::NONE,
        });
        assert!(matches!(mouse, Event::Mouse(_)));

        let resize = Event::Resize(120, 40);
        assert!(matches!(resize, Event::Resize(120, 40)));

        let metrics = Event::Metrics(TrainingMetrics::default());
        assert!(matches!(metrics, Event::Metrics(_)));

        let system = Event::System(SystemMetrics::default());
        assert!(matches!(system, Event::System(_)));
    }

    #[tokio::test]
    async fn test_event_handler_creation() {
        let mut handler = EventHandler::new(Duration::from_millis(50));
        let received = timeout(Duration::from_millis(250), handler.next())
            .await
            .expect("timed out waiting for handler event")
            .expect("event handler channel unexpectedly closed");

        assert!(matches!(received, Event::Tick));
    }
}
